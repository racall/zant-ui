//! `async` / `await` lowered to promise reactions, right after parsing.
//!
//! ```ts
//! async function load() {
//!   setLoading(true);
//!   try {
//!     const res = await fetch(url);
//!     const items: Item[] = await res.json();
//!     setItems(items);
//!   } catch (e) {
//!     setError(String(e));
//!   } finally {
//!     setLoading(false);
//!   }
//! }
//! ```
//! becomes
//! ```ts
//! function load() {
//!   setLoading(true);
//!   Promise.resolve()
//!     .then(() => fetch(url).then((res) => res.json().then((items: Item[]) => { setItems(items); })))
//!     .catch((e) => { setError(String(e)); })
//!     .finally(() => { setLoading(false); });
//! }
//! ```
//! The statements after an `await` become the reaction to its promise
//! (continuation-passing, as Regenerator and TypeScript's ES5 output do with
//! a state machine). A continuation is created when the code before it has
//! run, so it captures the locals' current values; each later statement
//! sees what the earlier ones assigned. `try` becomes `.catch` /
//! `.finally`, so a rejected `await` lands in the catch block.
//!
//! The function itself returns its promise when it declares
//! `Promise<T>`; otherwise it starts the work and returns nothing (an event
//! handler, `load()` in an effect).
//!
//! Supported: `await` in the statements of the body and of a `try` block,
//! anywhere in an expression evaluated unconditionally (it is hoisted into a
//! temporary). Not yet: `await` inside `if` / loops / `switch`, on the right
//! of `&&` / `||` / `??` / `?:`, or in `catch` / `finally` blocks; those
//! report an error with the place.

// AST statement lists hold boxed statements (`ast::Body::Block`)
#![allow(clippy::vec_box)]

mod analysis;
mod build;

use crate::ast::{Body, Decl, Expr, ExprData, Function, Name, Pattern, Stmt, StmtData, Try};
use crate::error::ParseError as Error;
use crate::parser::Parser;
use analysis::{
    contains_return, declares, is_await, names, operand, stmt_awaits, take_await, wrap_returns,
};
use build::{call, ident, member, param, reaction, then};

/// Lowers the body of async function `f` in place.
pub fn lower(p: &mut Parser, f: &mut Function) -> Result<(), Error> {
    let keep = f.ret.is_some();
    let statements = match std::mem::replace(&mut f.body, Body::Block(Vec::new())) {
        Body::Block(statements) => statements,
        // `async () => await load()`
        Body::Expr(e) => {
            let loc = e.loc;
            let data = if keep {
                StmtData::Ret(Some(e))
            } else {
                StmtData::Expr(e)
            };
            vec![Box::new(Stmt { loc, data })]
        }
    };
    let mut lowering = Lowering { p, counter: 0 };
    f.body =
        Body::Block(lowering.segment(statements, if keep { Tail::Return } else { Tail::Run })?);
    Ok(())
}

/// What a segment does with the promise of its first `await`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tail {
    /// The function's own body without a declared type: start it.
    Run,
    /// Return it (continuations, and functions declaring `Promise<T>`).
    Return,
}

struct Lowering<'p, 'a> {
    p: &'p mut Parser<'a>,
    counter: usize,
}

impl Lowering<'_, '_> {
    fn temporary(&mut self) -> Vec<u8> {
        self.counter += 1;
        format!("_await{}", self.counter).into_bytes()
    }

    fn fail(&mut self, loc: u32, msg: &str) -> Error {
        self.p.fail(loc, msg)
    }

    /// A continuation's body: `rest` lowered, after `let x = x;` for each
    /// outer local it assigns (the continuation captured a copy; the
    /// rebinding is the variable from here on, and later continuations
    /// capture its current value).
    fn continuation(&mut self, rest: Vec<Box<Stmt>>, loc: u32) -> Result<Vec<Box<Stmt>>, Error> {
        let mut assigned: Vec<Vec<u8>> = Vec::new();
        let mut declared: Vec<Vec<u8>> = Vec::new();
        for s in &rest {
            names(s, &mut assigned, &mut declared);
        }
        let mut body: Vec<Box<Stmt>> = Vec::new();
        for name in assigned {
            if declared.contains(&name) || body.iter().any(|s| declares(s, &name)) {
                continue;
            }
            body.push(Box::new(Stmt {
                loc,
                data: StmtData::Decl(Decl {
                    is_const: false,
                    pattern: Pattern::Ident(Name {
                        name: name.clone(),
                        loc,
                    }),
                    ty: None,
                    init: Some(ident(loc, &name)),
                }),
            }));
        }
        body.extend(self.segment(rest, Tail::Return)?);
        Ok(body)
    }

    /// Lowers `statements`: unchanged up to the first `await`, which turns
    /// the rest into its reaction.
    fn segment(&mut self, statements: Vec<Box<Stmt>>, tail: Tail) -> Result<Vec<Box<Stmt>>, Error> {
        let statements = self.hoist(statements)?;
        let Some(at) = statements.iter().position(|s| stmt_awaits(s)) else {
            return Ok(statements);
        };
        let mut statements = statements;
        let rest: Vec<Box<Stmt>> = statements.split_off(at + 1);
        let current = statements.pop().expect("the awaiting statement");
        let mut prefix = statements;
        if tail == Tail::Return {
            // the segment's value is a promise on every path
            for s in &mut prefix {
                wrap_returns(s);
            }
        }
        let loc = current.loc;
        let chain = self.chain(*current, rest)?;
        let data = match tail {
            Tail::Run => StmtData::Expr(chain),
            Tail::Return => StmtData::Ret(Some(chain)),
        };
        prefix.push(Box::new(Stmt { loc, data }));
        Ok(prefix)
    }

    /// The promise chain of awaiting statement `current` followed by `rest`.
    fn chain(&mut self, current: Stmt, rest: Vec<Box<Stmt>>) -> Result<Box<Expr>, Error> {
        let loc = current.loc;
        match current.data {
            // `const items: Item[] = await res.json();`
            StmtData::Decl(Decl {
                is_const,
                pattern,
                ty,
                init: Some(init),
            }) if is_await(&init) => {
                let promise = operand(*init);
                let (param, mut body) = match pattern {
                    Pattern::Ident(name) => (param(name, ty), Vec::new()),
                    pattern => {
                        let name = self.temporary();
                        let unpack = Box::new(Stmt {
                            loc,
                            data: StmtData::Decl(Decl {
                                is_const,
                                pattern,
                                ty: None,
                                init: Some(ident(loc, &name)),
                            }),
                        });
                        (param(Name { name, loc }, ty), vec![unpack])
                    }
                };
                body.extend(self.continuation(rest, loc)?);
                Ok(then(promise, Some(param), body, loc))
            }
            // `await save();`
            StmtData::Expr(e) if is_await(&e) => {
                let body = self.continuation(rest, loc)?;
                Ok(then(operand(*e), None, body, loc))
            }
            // `text = await res.text();`: the continuation's own `text`
            StmtData::Expr(e) if matches!(&e.data, ExprData::Assign(a) if a.op == b"=" && is_await(&a.value)) => {
                let ExprData::Assign(assign) = e.data else {
                    unreachable!("an assignment")
                };
                let ExprData::Ident(target) = assign.target.data else {
                    return Err(self.fail(
                        assign.target.loc,
                        "assign the awaited value to a local: `const value = await …`",
                    ));
                };
                let name = self.temporary();
                let mut body = vec![Box::new(Stmt {
                    loc,
                    data: StmtData::Decl(Decl {
                        is_const: false,
                        pattern: Pattern::Ident(Name { name: target, loc }),
                        ty: None,
                        init: Some(ident(loc, &name)),
                    }),
                })];
                body.extend(self.continuation(rest, loc)?);
                Ok(then(
                    operand(*assign.value),
                    Some(param(Name { name, loc }, None)),
                    body,
                    loc,
                ))
            }
            // `return await load();`: the promise is the value
            StmtData::Ret(Some(e)) if is_await(&e) => Ok(operand(*e)),
            StmtData::Try(t) => self.try_chain(loc, t, rest),
            _ => Err(self.fail(
                loc,
                "`await` is supported in the statements of an async function and of its `try` blocks, not inside `if`, loops or `switch` yet",
            )),
        }
    }

    /// `try { … await … } catch (e) { … } finally { … }` and what follows.
    fn try_chain(&mut self, loc: u32, t: Try, rest: Vec<Box<Stmt>>) -> Result<Box<Expr>, Error> {
        for block in t.catch.iter().map(|c| &c.body).chain(t.finally.iter()) {
            if let Some(at) = block.iter().find(|s| stmt_awaits(s)) {
                return Err(self.fail(
                    at.loc,
                    "`await` inside `catch` / `finally` is not supported yet",
                ));
            }
        }
        if !rest.is_empty() && t.body.iter().any(|s| contains_return(s)) {
            return Err(self.fail(
                loc,
                "a `return` inside this `try` would also skip the statements after it; move them into the `try`",
            ));
        }
        // the block runs as a reaction so a throw before its first `await`
        // is caught too
        let start = call(
            member(ident(loc, b"Promise"), b"resolve", loc),
            Vec::new(),
            loc,
        );
        let body = self.continuation(t.body, loc)?;
        let mut chain = then(start, None, body, loc);
        if let Some(catch) = t.catch {
            let name = match catch.name {
                Some(name) => name,
                None => Name {
                    name: self.temporary(),
                    loc,
                },
            };
            chain = reaction(chain, b"catch", Some(param(name, None)), catch.body, loc);
        }
        if let Some(finally) = t.finally {
            chain = reaction(chain, b"finally", None, finally, loc);
        }
        if !rest.is_empty() {
            let body = self.continuation(rest, loc)?;
            chain = then(chain, None, body, loc);
        }
        Ok(chain)
    }

    /// Moves an `await` nested in a statement's expression into a
    /// `const _awaitN = await …;` before it, until every `await` is one of
    /// the forms [`chain`](Self::chain) lowers.
    fn hoist(&mut self, statements: Vec<Box<Stmt>>) -> Result<Vec<Box<Stmt>>, Error> {
        let mut out = Vec::with_capacity(statements.len());
        for mut s in statements {
            loop {
                let slot = match &mut s.data {
                    StmtData::Decl(Decl { init: Some(e), .. }) if !is_await(e) => e,
                    StmtData::Expr(e)
                        if !is_await(e)
                            && !matches!(&e.data, ExprData::Assign(a) if a.op == b"=" && is_await(&a.value)) =>
                    {
                        e
                    }
                    StmtData::Ret(Some(e)) if !is_await(e) => e,
                    StmtData::Throw(e) => e,
                    _ => break,
                };
                let name = self.temporary();
                match take_await(slot, &name) {
                    Ok(Some(awaited)) => {
                        let loc = awaited.loc;
                        out.push(Box::new(Stmt {
                            loc,
                            data: StmtData::Decl(Decl {
                                is_const: true,
                                pattern: Pattern::Ident(Name { name, loc }),
                                ty: None,
                                init: Some(awaited),
                            }),
                        }));
                    }
                    Ok(None) => break,
                    Err(loc) => {
                        return Err(self.fail(
                            loc,
                            "`await` on the right of `&&` / `||` / `??` / `?:` is not supported yet; await into a `const` first",
                        ));
                    }
                }
            }
            out.push(s);
        }
        Ok(out)
    }
}
