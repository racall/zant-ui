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

use crate::ast::{
    Body, Call, Decl, Expr, ExprData, Function, Member, Name, Param, Pattern, Stmt, StmtData, Try,
};
use crate::ast_walk::{self, StmtChild};
use crate::error::ParseError as Error;
use crate::parser::Parser;

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
    f.body = Body::Block(lowering.segment(statements, if keep { Tail::Return } else { Tail::Run })?);
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
                body.extend(self.segment(rest, Tail::Return)?);
                Ok(then(promise, Some(param), body, loc))
            }
            // `await save();`
            StmtData::Expr(e) if is_await(&e) => {
                let body = self.segment(rest, Tail::Return)?;
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
                body.extend(self.segment(rest, Tail::Return)?);
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
                return Err(self.fail(at.loc, "`await` inside `catch` / `finally` is not supported yet"));
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
        let body = self.segment(t.body, Tail::Return)?;
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
            let body = self.segment(rest, Tail::Return)?;
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

fn is_await(e: &Expr) -> bool {
    matches!(&e.data, ExprData::Unary(u) if u.op == b"await")
}

/// The promise of `await promise`.
fn operand(e: Expr) -> Box<Expr> {
    match e.data {
        ExprData::Unary(u) => u.operand,
        _ => unreachable!("an await"),
    }
}

/// An `await` in `e`, outside nested functions.
fn expr_awaits(e: &Expr) -> bool {
    if is_await(e) {
        return true;
    }
    let mut found = false;
    ast_walk::expr_children(e, &mut |child| found = found || expr_awaits(child));
    found
}

/// An `await` in `s`, outside nested functions.
fn stmt_awaits(s: &Stmt) -> bool {
    let mut found = false;
    ast_walk::stmt_children(s, &mut |child| {
        found = found
            || match child {
                StmtChild::Expr(e) => expr_awaits(e),
                StmtChild::Stmt(s) => stmt_awaits(s),
            }
    });
    found
}

/// A `return` in `s`, outside nested functions.
fn contains_return(s: &Stmt) -> bool {
    if matches!(s.data, StmtData::Ret(_)) {
        return true;
    }
    let mut found = false;
    ast_walk::stmt_children(s, &mut |child| {
        if let StmtChild::Stmt(s) = child {
            found = found || contains_return(s);
        }
    });
    found
}

/// `return;` / `return v;` before the first `await` of a segment whose value
/// is a promise: `return Promise.resolve(v)`.
fn wrap_returns(s: &mut Stmt) {
    let loc = s.loc;
    match &mut s.data {
        StmtData::Ret(value) => {
            let args = value.take().into_iter().collect();
            *value = Some(call(
                member(ident(loc, b"Promise"), b"resolve", loc),
                args,
                loc,
            ));
        }
        StmtData::If(i) => {
            wrap_returns(&mut i.then);
            if let Some(e) = &mut i.else_ {
                wrap_returns(e);
            }
        }
        StmtData::Block(statements) => statements.iter_mut().for_each(|s| wrap_returns(s)),
        StmtData::For(f) => wrap_returns(&mut f.body),
        StmtData::ForOf(f) => wrap_returns(&mut f.body),
        StmtData::While(w) => wrap_returns(&mut w.body),
        StmtData::Switch(sw) => sw
            .cases
            .iter_mut()
            .flat_map(|c| c.body.iter_mut())
            .for_each(|s| wrap_returns(s)),
        StmtData::Try(t) => {
            t.body.iter_mut().for_each(|s| wrap_returns(s));
            if let Some(c) = &mut t.catch {
                c.body.iter_mut().for_each(|s| wrap_returns(s));
            }
            if let Some(f) = &mut t.finally {
                f.iter_mut().for_each(|s| wrap_returns(s));
            }
        }
        _ => {}
    }
}

/// Replaces the first `await` evaluated in `e` (outside nested functions)
/// with `name` and returns it. `Err(loc)` for an `await` that runs only
/// conditionally (`a && await b`).
fn take_await(e: &mut Expr, name: &[u8]) -> Result<Option<Box<Expr>>, u32> {
    if is_await(e) {
        let loc = e.loc;
        let awaited = std::mem::replace(e, *ident(loc, name));
        return Ok(Some(Box::new(awaited)));
    }
    let conditional = |e: &Expr| if expr_awaits(e) { Err(e.loc) } else { Ok(None) };
    match &mut e.data {
        ExprData::Binary(b) if matches!(b.op.as_slice(), b"&&" | b"||" | b"??") => {
            match take_await(&mut b.lhs, name)? {
                Some(found) => Ok(Some(found)),
                None => conditional(&b.rhs),
            }
        }
        ExprData::Cond(c) => match take_await(&mut c.cond, name)? {
            Some(found) => Ok(Some(found)),
            None => conditional(&c.then).and_then(|_| conditional(&c.else_)),
        },
        ExprData::Binary(b) => first(&mut [&mut b.lhs, &mut b.rhs], name),
        ExprData::Unary(u) => take_await(&mut u.operand, name),
        ExprData::Template(t) => first(&mut t.exprs.iter_mut().collect::<Vec<_>>(), name),
        ExprData::Call(c) => {
            let mut slots: Vec<&mut Box<Expr>> = vec![&mut c.callee];
            slots.extend(c.args.iter_mut());
            first(&mut slots, name)
        }
        ExprData::Member(m) => take_await(&mut m.object, name),
        ExprData::Object(properties) => {
            first(&mut properties.iter_mut().map(|p| &mut p.value).collect::<Vec<_>>(), name)
        }
        ExprData::Array(items) => first(&mut items.iter_mut().collect::<Vec<_>>(), name),
        ExprData::Spread(x) | ExprData::NonNull(x) => take_await(x, name),
        ExprData::Index(i) => first(&mut [&mut i.object, &mut i.index], name),
        ExprData::As(a) => take_await(&mut a.expr, name),
        ExprData::Assign(a) => first(&mut [&mut a.target, &mut a.value], name),
        ExprData::Update(u) => take_await(&mut u.target, name),
        _ => Ok(None),
    }
}

fn first(slots: &mut [&mut Box<Expr>], name: &[u8]) -> Result<Option<Box<Expr>>, u32> {
    for slot in slots.iter_mut() {
        if let Some(found) = take_await(slot, name)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

// ── building the chain ───────────────────────────────────────────────

fn ident(loc: u32, name: &[u8]) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Ident(name.to_vec()),
    })
}

fn member(object: Box<Expr>, name: &[u8], loc: u32) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Member(Member {
            object,
            name: Name {
                name: name.to_vec(),
                loc,
            },
            optional: false,
        }),
    })
}

fn call(callee: Box<Expr>, args: Vec<Box<Expr>>, loc: u32) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Call(Call {
            callee,
            type_args: Vec::new(),
            args,
            optional: false,
            is_new: false,
        }),
    })
}

fn param(name: Name, ty: Option<crate::ast::TypeRef>) -> Param {
    Param {
        name,
        ty,
        optional: false,
        destructure: Vec::new(),
        destructure_defaults: Vec::new(),
        rest: false,
        default: None,
    }
}

/// `promise.then((param) => { body })`.
fn then(promise: Box<Expr>, param: Option<Param>, body: Vec<Box<Stmt>>, loc: u32) -> Box<Expr> {
    reaction(promise, b"then", param, body, loc)
}

fn reaction(
    promise: Box<Expr>,
    method: &[u8],
    param: Option<Param>,
    body: Vec<Box<Stmt>>,
    loc: u32,
) -> Box<Expr> {
    let arrow = Box::new(Expr {
        loc,
        data: ExprData::Arrow(Box::new(Function {
            type_params: Vec::new(),
            loc,
            name: Vec::new(),
            params: param.into_iter().collect(),
            ret: None,
            body: Body::Block(body),
        })),
    });
    call(member(promise, method, loc), vec![arrow], loc)
}
