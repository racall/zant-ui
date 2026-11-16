//! What a statement or expression of an async body contains (`await`,
//! `return`, assigned names), and the in-place edits the lowering makes.

// AST statement lists hold boxed statements (`ast::Body::Block`)
#![allow(clippy::vec_box)]

use crate::ast::{Decl, Expr, ExprData, Pattern, Stmt, StmtData};
use crate::ast_walk::{self, StmtChild};

use super::build::{call, ident, member};

pub(super) fn is_await(e: &Expr) -> bool {
    matches!(&e.data, ExprData::Unary(u) if u.op == b"await")
}

/// The promise of `await promise`.
pub(super) fn operand(e: Expr) -> Box<Expr> {
    match e.data {
        ExprData::Unary(u) => u.operand,
        _ => unreachable!("an await"),
    }
}

/// An `await` in `e`, outside nested functions.
pub(super) fn expr_awaits(e: &Expr) -> bool {
    if is_await(e) {
        return true;
    }
    let mut found = false;
    ast_walk::expr_children(e, &mut |child| found = found || expr_awaits(child));
    found
}

/// An `await` in `s`, outside nested functions.
pub(super) fn stmt_awaits(s: &Stmt) -> bool {
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

/// Names `s` assigns (`x = …`, `x += …`, `x++`) and declares, outside
/// nested functions.
pub(super) fn names(s: &Stmt, assigned: &mut Vec<Vec<u8>>, declared: &mut Vec<Vec<u8>>) {
    fn expr(e: &Expr, assigned: &mut Vec<Vec<u8>>) {
        let target = match &e.data {
            ExprData::Assign(a) => Some(&a.target),
            ExprData::Update(u) => Some(&u.target),
            _ => None,
        };
        if let Some(target) = target
            && let ExprData::Ident(name) = &target.data
            && !assigned.contains(name)
        {
            assigned.push(name.clone());
        }
        ast_walk::expr_children(e, &mut |child| expr(child, assigned));
    }
    match &s.data {
        StmtData::Decl(d) => match &d.pattern {
            Pattern::Ident(name) => declared.push(name.name.clone()),
            Pattern::Array(items) => {
                declared.extend(items.iter().flatten().map(|n| n.name.clone()))
            }
            Pattern::Object(bindings) => {
                declared.extend(bindings.iter().map(|b| b.local.name.clone()))
            }
        },
        StmtData::Func(f) => declared.push(f.name.clone()),
        _ => {}
    }
    ast_walk::stmt_children(s, &mut |child| match child {
        StmtChild::Expr(e) => expr(e, assigned),
        StmtChild::Stmt(s) => names(s, assigned, declared),
    });
}

/// `s` is `let name = …`.
pub(super) fn declares(s: &Stmt, name: &[u8]) -> bool {
    matches!(&s.data, StmtData::Decl(Decl { pattern: Pattern::Ident(n), .. }) if n.name == name)
}

/// A `return` in `s`, outside nested functions.
pub(super) fn contains_return(s: &Stmt) -> bool {
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
pub(super) fn wrap_returns(s: &mut Stmt) {
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
pub(super) fn take_await(e: &mut Expr, name: &[u8]) -> Result<Option<Box<Expr>>, u32> {
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
        ExprData::Object(properties) => first(
            &mut properties
                .iter_mut()
                .map(|p| &mut p.value)
                .collect::<Vec<_>>(),
            name,
        ),
        ExprData::Array(items) => first(&mut items.iter_mut().collect::<Vec<_>>(), name),
        ExprData::Spread(x) | ExprData::NonNull(x) => take_await(x, name),
        ExprData::Index(i) => first(&mut [&mut i.object, &mut i.index], name),
        ExprData::As(a) => take_await(&mut a.expr, name),
        ExprData::Assign(a) => first(&mut [&mut a.target, &mut a.value], name),
        ExprData::Update(u) => take_await(&mut u.target, name),
        _ => Ok(None),
    }
}

pub(super) fn first(slots: &mut [&mut Box<Expr>], name: &[u8]) -> Result<Option<Box<Expr>>, u32> {
    for slot in slots.iter_mut() {
        if let Some(found) = take_await(slot, name)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}
