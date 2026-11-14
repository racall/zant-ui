//! After inlining: a custom hook still named somewhere was called where
//! hooks can't be (inside a condition, a loop, a handler, a plain
//! function); and a hook's body may only return at its end.
//!
//! Direct translation of `tools/tsx/hooks/leftover.zig`.

use crate::ast::{self, Loc};

pub struct Found {
    pub name: Vec<u8>,
    pub loc: Loc,
}

struct Walker<'a> {
    units: &'a [super::resolve::Unit],
    unit: usize,
    found: Option<Found>,
}

impl Walker<'_> {
    fn check(&mut self, name: &[u8], loc: Loc) {
        if self.found.is_some() {
            return;
        }
        if super::resolve::find_hook_in(self.units, self.unit, name).is_none() {
            return;
        }
        let at = name.iter().position(|&b| b == b'@').unwrap_or(name.len());
        self.found = Some(Found {
            name: name[..at].to_vec(),
            loc,
        });
    }

    fn function(&mut self, f: &ast::Function) {
        for p in &f.params {
            if let Some(d) = &p.default {
                self.expr(d);
            }
        }
        match &f.body {
            ast::Body::Block(ss) => self.stmts(ss),
            ast::Body::Expr(e) => self.expr(e),
        }
    }

    fn stmts(&mut self, ss: &[Box<ast::Stmt>]) {
        for s in ss {
            self.stmt(s);
        }
    }

    fn opt(&mut self, e: &Option<Box<ast::Expr>>) {
        if let Some(v) = e {
            self.expr(v);
        }
    }

    fn stmt(&mut self, s: &ast::Stmt) {
        match &s.data {
            ast::StmtData::Decl(d) => self.opt(&d.init),
            ast::StmtData::Func(f) => self.function(f),
            ast::StmtData::Ret(e) => self.opt(e),
            ast::StmtData::If(v) => {
                self.expr(&v.cond);
                self.stmt(&v.then);
                if let Some(el) = &v.else_ {
                    self.stmt(el);
                }
            }
            ast::StmtData::For(v) => {
                if let Some(i) = &v.init {
                    self.stmt(i);
                }
                self.opt(&v.cond);
                self.opt(&v.update);
                self.stmt(&v.body);
            }
            ast::StmtData::ForOf(v) => {
                self.expr(&v.iterable);
                self.stmt(&v.body);
            }
            ast::StmtData::While(v) => {
                self.expr(&v.cond);
                self.stmt(&v.body);
            }
            ast::StmtData::Switch(v) => {
                self.expr(&v.value);
                for case in &v.cases {
                    self.opt(&case.value);
                    self.stmts(&case.body);
                }
            }
            ast::StmtData::Break | ast::StmtData::Continue => {}
            ast::StmtData::Expr(e) => self.expr(e),
            ast::StmtData::Block(ss) => self.stmts(ss),
        }
    }

    fn expr(&mut self, e: &ast::Expr) {
        match &e.data {
            ast::ExprData::Number(_)
            | ast::ExprData::String(_)
            | ast::ExprData::Boolean(_)
            | ast::ExprData::NullLit => {}
            ast::ExprData::Ident(n) => self.check(n, e.loc),
            ast::ExprData::Template(t) => {
                for x in &t.exprs {
                    self.expr(x);
                }
            }
            ast::ExprData::Unary(u) => self.expr(&u.operand),
            ast::ExprData::Binary(b) => {
                self.expr(&b.lhs);
                self.expr(&b.rhs);
            }
            ast::ExprData::Cond(v) => {
                self.expr(&v.cond);
                self.expr(&v.then);
                self.expr(&v.else_);
            }
            ast::ExprData::Call(v) => {
                self.expr(&v.callee);
                for a in &v.args {
                    self.expr(a);
                }
            }
            ast::ExprData::Member(m) => self.expr(&m.object),
            ast::ExprData::Arrow(f) => self.function(f),
            ast::ExprData::Jsx(j) => self.jsx(j),
            ast::ExprData::Object(ps) => {
                for p in ps {
                    self.expr(&p.value);
                }
            }
            ast::ExprData::Array(items) => {
                for x in items {
                    self.expr(x);
                }
            }
            ast::ExprData::Spread(inner) | ast::ExprData::NonNull(inner) => self.expr(inner),
            ast::ExprData::Index(ix) => {
                self.expr(&ix.object);
                self.expr(&ix.index);
            }
            ast::ExprData::As(cast) => self.expr(&cast.expr),
            ast::ExprData::Assign(v) => {
                self.expr(&v.target);
                self.expr(&v.value);
            }
            ast::ExprData::Update(v) => self.expr(&v.target),
        }
    }

    fn jsx(&mut self, j: &ast::Jsx) {
        for a in &j.attrs {
            self.opt(&a.value);
        }
        for child in &j.children {
            match child {
                ast::Child::Text(_) => {}
                ast::Child::Expr(e) => self.opt(e),
                ast::Child::Element(el) => self.jsx(el),
            }
        }
    }
}

/// The first custom hook named in unit `unit`'s code (hooks themselves excluded).
pub fn find(units: &[super::resolve::Unit], unit: usize) -> Option<Found> {
    let mut w = Walker {
        units,
        unit,
        found: None,
    };
    let m = &units[unit].module;
    for top in &m.functions {
        if !super::resolve::is_hook_name(&top.func.name) {
            w.function(&top.func);
        }
    }
    for c in &m.consts {
        let hook_arrow = super::resolve::is_hook_name(&c.name.name)
            && matches!(c.value.data, ast::ExprData::Arrow(_));
        if !hook_arrow {
            w.expr(&c.value);
        }
    }
    w.found
}

/// A `return` inside `s` outside nested functions.
pub fn returns(s: &ast::Stmt) -> Option<Loc> {
    match &s.data {
        ast::StmtData::Ret(_) => Some(s.loc),
        ast::StmtData::If(v) => {
            returns(&v.then).or_else(|| v.else_.as_ref().and_then(|el| returns(el)))
        }
        ast::StmtData::For(v) => returns(&v.body),
        ast::StmtData::ForOf(v) => returns(&v.body),
        ast::StmtData::While(v) => returns(&v.body),
        ast::StmtData::Switch(v) => {
            for case in &v.cases {
                if let Some(loc) = first(&case.body) {
                    return Some(loc);
                }
            }
            None
        }
        ast::StmtData::Block(ss) => first(ss),
        _ => None,
    }
}

fn first(ss: &[Box<ast::Stmt>]) -> Option<Loc> {
    for b in ss {
        if let Some(loc) = returns(b) {
            return Some(loc);
        }
    }
    None
}
