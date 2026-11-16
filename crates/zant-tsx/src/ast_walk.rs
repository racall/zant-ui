//! The direct children of an AST node, for analyses that only care about a
//! few node kinds (they match those and recurse through the rest here).
//!
//! Function bodies (`Arrow`, `Func`) are *not* children: an analysis decides
//! whether creating a function counts as running it.

use crate::ast::{Body, Child, Expr, ExprData, Jsx, Stmt, StmtData};

/// Calls `f` on each direct child expression of `e`.
pub fn expr_children<'a>(e: &'a Expr, f: &mut dyn FnMut(&'a Expr)) {
    match &e.data {
        ExprData::Number(_)
        | ExprData::String(_)
        | ExprData::Boolean(_)
        | ExprData::NullLit
        | ExprData::Ident(_)
        | ExprData::Arrow(_) => {}
        ExprData::Template(t) => t.exprs.iter().for_each(|x| f(x)),
        ExprData::Unary(u) => f(&u.operand),
        ExprData::Binary(b) => {
            f(&b.lhs);
            f(&b.rhs);
        }
        ExprData::Cond(c) => {
            f(&c.cond);
            f(&c.then);
            f(&c.else_);
        }
        ExprData::Call(c) => {
            f(&c.callee);
            c.args.iter().for_each(|x| f(x));
        }
        ExprData::Member(m) => f(&m.object),
        ExprData::Jsx(jsx) => jsx_children(jsx, f),
        ExprData::Object(properties) => properties.iter().for_each(|p| f(&p.value)),
        ExprData::Array(items) => items.iter().for_each(|x| f(x)),
        ExprData::Spread(x) | ExprData::NonNull(x) => f(x),
        ExprData::Index(i) => {
            f(&i.object);
            f(&i.index);
        }
        ExprData::As(a) => f(&a.expr),
        ExprData::Assign(a) => {
            f(&a.target);
            f(&a.value);
        }
        ExprData::Update(u) => f(&u.target),
    }
}

fn jsx_children<'a>(jsx: &'a Jsx, f: &mut dyn FnMut(&'a Expr)) {
    for attr in &jsx.attrs {
        if let Some(value) = attr.value.as_deref() {
            f(value);
        }
    }
    for child in &jsx.children {
        match child {
            Child::Text(_) | Child::Expr(None) => {}
            Child::Expr(Some(x)) => f(x),
            Child::Element(element) => jsx_children(element, f),
        }
    }
}

/// A child of a statement.
pub enum StmtChild<'a> {
    Expr(&'a Expr),
    Stmt(&'a Stmt),
}

/// Calls `f` on each direct child of `s`, in source order. A `try`'s blocks
/// are listed body, catch, finally (analyses that care about the regions
/// match `StmtData::Try` themselves).
pub fn stmt_children<'a>(s: &'a Stmt, f: &mut dyn FnMut(StmtChild<'a>)) {
    let stmts = |list: &'a [Box<Stmt>], f: &mut dyn FnMut(StmtChild<'a>)| {
        list.iter().for_each(|x| f(StmtChild::Stmt(x)));
    };
    match &s.data {
        StmtData::Decl(d) => {
            if let Some(init) = d.init.as_deref() {
                f(StmtChild::Expr(init));
            }
        }
        StmtData::Func(_) | StmtData::Break | StmtData::Continue => {}
        StmtData::Ret(value) => {
            if let Some(value) = value.as_deref() {
                f(StmtChild::Expr(value));
            }
        }
        StmtData::If(i) => {
            f(StmtChild::Expr(&i.cond));
            f(StmtChild::Stmt(&i.then));
            if let Some(otherwise) = i.else_.as_deref() {
                f(StmtChild::Stmt(otherwise));
            }
        }
        StmtData::For(l) => {
            if let Some(init) = l.init.as_deref() {
                f(StmtChild::Stmt(init));
            }
            if let Some(cond) = l.cond.as_deref() {
                f(StmtChild::Expr(cond));
            }
            if let Some(update) = l.update.as_deref() {
                f(StmtChild::Expr(update));
            }
            f(StmtChild::Stmt(&l.body));
        }
        StmtData::ForOf(l) => {
            f(StmtChild::Expr(&l.iterable));
            f(StmtChild::Stmt(&l.body));
        }
        StmtData::While(l) => {
            f(StmtChild::Expr(&l.cond));
            f(StmtChild::Stmt(&l.body));
        }
        StmtData::Switch(sw) => {
            f(StmtChild::Expr(&sw.value));
            for case in &sw.cases {
                if let Some(value) = case.value.as_deref() {
                    f(StmtChild::Expr(value));
                }
                stmts(&case.body, f);
            }
        }
        StmtData::Expr(e) | StmtData::Throw(e) => f(StmtChild::Expr(e)),
        StmtData::Block(list) => stmts(list, f),
        StmtData::Try(t) => {
            stmts(&t.body, f);
            if let Some(catch) = &t.catch {
                stmts(&catch.body, f);
            }
            if let Some(finally) = &t.finally {
                stmts(finally, f);
            }
        }
    }
}

/// The statements or expression of a function body.
pub fn body_children<'a>(body: &'a Body, f: &mut dyn FnMut(StmtChild<'a>)) {
    match body {
        Body::Block(list) => list.iter().for_each(|x| f(StmtChild::Stmt(x))),
        Body::Expr(e) => f(StmtChild::Expr(e)),
    }
}
