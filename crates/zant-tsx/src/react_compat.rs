//! Lower qualified React hooks and Fragment to the existing Zant syntax.
//! Runs after parsing all imports, before stateful analysis or generation.
//!
//! Also lowers the memoization-only hooks the way their React semantics
//! allow: `useCallback(fn, deps)` only keeps `fn`'s identity stable between
//! renders, so it is `fn` (Zant rebuilds handlers every render anyway), and a
//! `useMemo` whose factory returns a function (`useMemo(() => () => …, deps)`)
//! is that function.
//!
//! The walker mutates the tree in place (`&mut Module`).

use crate::ast::{
    Body, Child, Expr, ExprData, Function, Import, Jsx, Module, Pattern, Stmt, StmtData,
};

/// Whether a name is a default or namespace binding imported from React.
pub fn namespace(imports: &[Import], name: &[u8]) -> bool {
    for imp in imports {
        if imp.source.as_slice() != b"react".as_slice() {
            continue;
        }
        for n in [&imp.default_name, &imp.namespace_name]
            .into_iter()
            .flatten()
        {
            if n.name.as_slice() == name {
                return true;
            }
        }
    }
    false
}

/// A qualified element type through a React binding (including renamed namespaces).
pub fn node_type(imports: &[Import], name: &[u8]) -> bool {
    let Some(dot) = name.iter().position(|&b| b == b'.') else {
        return false;
    };
    namespace(imports, &name[..dot])
        && (&name[dot + 1..] == b"ReactNode".as_slice()
            || &name[dot + 1..] == b"ReactElement".as_slice())
}

/// Normalize only the four supported React hooks and React's Fragment tag.
pub fn normalize(module: &mut Module) {
    // The bindings are collected first so the module can be borrowed mutably
    // afterwards.
    let mut bindings: Vec<Vec<u8>> = Vec::new();
    for imp in &module.imports {
        if imp.source.as_slice() != b"react".as_slice() || imp.type_only {
            continue;
        }
        for n in [&imp.default_name, &imp.namespace_name]
            .into_iter()
            .flatten()
        {
            bindings.push(n.name.clone());
        }
    }
    for name in bindings {
        let mut walker = Walker {
            name,
            mode: Mode::Namespace,
        };
        for top in &mut module.functions {
            walker.function(&mut top.func);
        }
        for decl in &mut module.consts {
            walker.expr(&mut decl.value);
        }
    }
    // `useCallback` / `useMemo` under the local names they are imported as
    // (React's qualified `React.useCallback` was renamed to the plain hook above)
    for (hook, mode) in [
        (b"useCallback".as_slice(), Mode::Callback),
        (b"useMemo".as_slice(), Mode::FunctionMemo),
    ] {
        let mut names: Vec<Vec<u8>> = vec![hook.to_vec()];
        for imp in &module.imports {
            if imp.type_only
                || (imp.source.as_slice() != b"react" && imp.source.as_slice() != b"zant")
            {
                continue;
            }
            for n in &imp.names {
                if n.imported.name.as_slice() == hook && n.local.name.as_slice() != hook {
                    names.push(n.local.name.clone());
                }
            }
        }
        for name in names {
            let mut walker = Walker { name, mode };
            for top in &mut module.functions {
                walker.function(&mut top.func);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `name` is React's default / namespace binding: `React.useState` → `useState`.
    Namespace,
    /// `name` is `useCallback`: `useCallback(fn, deps)` → `fn`.
    Callback,
    /// `name` is `useMemo`: `useMemo(() => fn, deps)` with an arrow `fn` → `fn`.
    FunctionMemo,
}

struct Walker {
    name: Vec<u8>,
    mode: Mode,
}

impl Walker {
    fn matches(&self, name: &[u8]) -> bool {
        self.name.as_slice() == name
    }

    fn binds(&self, pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Ident(n) => self.matches(&n.name),
            Pattern::Array(ns) => ns.iter().flatten().any(|v| self.matches(&v.name)),
            Pattern::Object(bs) => bs.iter().any(|b| self.matches(&b.local.name)),
        }
    }

    fn function(&mut self, f: &mut Function) {
        if self.matches(&f.name) {
            return;
        }
        for param in &f.params {
            if self.matches(&param.name.name) {
                return;
            }
            for n in &param.destructure {
                if self.matches(&n.name) {
                    return;
                }
            }
        }
        self.body(&mut f.body);
    }

    fn block(&mut self, ss: &mut [Box<Stmt>]) {
        // Lexical declarations shadow the import throughout their block (including the TDZ).
        for s in ss.iter() {
            match &s.data {
                StmtData::Decl(d) => {
                    if self.binds(&d.pattern) {
                        return;
                    }
                }
                StmtData::Func(f) => {
                    if self.matches(&f.name) {
                        return;
                    }
                }
                _ => {}
            }
        }
        for s in ss.iter_mut() {
            self.stmt(s);
        }
    }

    fn body(&mut self, b: &mut Body) {
        match b {
            Body::Block(ss) => self.block(ss),
            Body::Expr(e) => self.expr(e),
        }
    }

    fn stmt(&mut self, s: &mut Stmt) {
        match &mut s.data {
            StmtData::Decl(d) => {
                if let Some(e) = &mut d.init {
                    self.expr(e);
                }
            }
            StmtData::Func(f) => self.function(f),
            StmtData::Ret(e) => {
                if let Some(v) = e {
                    self.expr(v);
                }
            }
            StmtData::Expr(e) => self.expr(e),
            StmtData::Block(ss) => self.block(ss),
            StmtData::If(v) => {
                self.expr(&mut v.cond);
                self.stmt(&mut v.then);
                if let Some(el) = &mut v.else_ {
                    self.stmt(el);
                }
            }
            StmtData::For(v) => {
                if let Some(init) = &v.init
                    && let StmtData::Decl(d) = &init.data
                    && self.binds(&d.pattern)
                {
                    return;
                }
                if let Some(init) = &mut v.init {
                    self.stmt(init);
                }
                if let Some(cond) = &mut v.cond {
                    self.expr(cond);
                }
                if let Some(update) = &mut v.update {
                    self.expr(update);
                }
                self.stmt(&mut v.body);
            }
            StmtData::ForOf(v) => {
                self.expr(&mut v.iterable);
                if !self.matches(&v.name.name) {
                    self.stmt(&mut v.body);
                }
            }
            StmtData::While(v) => {
                self.expr(&mut v.cond);
                self.stmt(&mut v.body);
            }
            StmtData::Switch(v) => {
                self.expr(&mut v.value);
                for case in &mut v.cases {
                    if let Some(value) = &mut case.value {
                        self.expr(value);
                    }
                    for child in &mut case.body {
                        self.stmt(child);
                    }
                }
            }
            StmtData::Break | StmtData::Continue => {}
        }
    }

    fn expr(&mut self, e: &mut Expr) {
        match &mut e.data {
            ExprData::Number(_)
            | ExprData::String(_)
            | ExprData::Boolean(_)
            | ExprData::NullLit
            | ExprData::Ident(_) => {}
            ExprData::Call(c) if self.mode != Mode::Namespace => {
                let lowered = match &c.callee.data {
                    ExprData::Ident(callee)
                        if self.matches(callee) && !c.args.is_empty() && c.args.len() <= 2 =>
                    {
                        match self.mode {
                            Mode::Callback => Some((*c.args[0]).clone()),
                            _ => match &c.args[0].data {
                                ExprData::Arrow(factory) => match &factory.body {
                                    Body::Expr(inner)
                                        if matches!(inner.data, ExprData::Arrow(_)) =>
                                    {
                                        Some((**inner).clone())
                                    }
                                    _ => None,
                                },
                                _ => None,
                            },
                        }
                    }
                    _ => None,
                };
                if let Some(function) = lowered {
                    *e = function;
                    self.expr(e);
                    return;
                }
                self.expr(&mut c.callee);
                for a in &mut c.args {
                    self.expr(a);
                }
            }
            ExprData::Call(c) => {
                let mut replacement: Option<&'static str> = None;
                if let ExprData::Member(m) = &c.callee.data
                    && !m.optional
                    && let ExprData::Ident(object) = &m.object.data
                    && self.matches(object)
                {
                    for hook in ["useState", "useEffect", "useMemo", "useRef", "useCallback"] {
                        if m.name.name.as_slice() == hook.as_bytes() {
                            replacement = Some(hook);
                        }
                    }
                }
                if let Some(hook) = replacement {
                    c.callee.data = ExprData::Ident(hook.as_bytes().to_vec());
                }
                self.expr(&mut c.callee);
                for a in &mut c.args {
                    self.expr(a);
                }
            }
            ExprData::Template(t) => {
                for x in &mut t.exprs {
                    self.expr(x);
                }
            }
            ExprData::Unary(u) => self.expr(&mut u.operand),
            ExprData::Binary(b) => {
                self.expr(&mut b.lhs);
                self.expr(&mut b.rhs);
            }
            ExprData::Cond(c) => {
                self.expr(&mut c.cond);
                self.expr(&mut c.then);
                self.expr(&mut c.else_);
            }
            ExprData::Member(m) => self.expr(&mut m.object),
            ExprData::Arrow(f) => self.function(f),
            ExprData::Jsx(j) => self.jsx(j),
            ExprData::Object(props) => {
                for p in props {
                    self.expr(&mut p.value);
                }
            }
            ExprData::Array(items) => {
                for x in items {
                    self.expr(x);
                }
            }
            ExprData::Spread(x) | ExprData::NonNull(x) => self.expr(x),
            ExprData::Index(i) => {
                self.expr(&mut i.object);
                self.expr(&mut i.index);
            }
            ExprData::As(a) => self.expr(&mut a.expr),
            ExprData::Assign(a) => {
                self.expr(&mut a.target);
                self.expr(&mut a.value);
            }
            ExprData::Update(u) => self.expr(&mut u.target),
        }
    }

    fn jsx(&mut self, j: &mut Jsx) {
        if self.mode == Mode::Namespace
            && let Some(dot) = j.name.iter().position(|&b| b == b'.')
            && self.matches(&j.name[..dot])
            && &j.name[dot + 1..] == b"Fragment".as_slice()
        {
            j.name = Vec::new();
        }
        for a in &mut j.attrs {
            if let Some(v) = &mut a.value {
                self.expr(v);
            }
        }
        for child in &mut j.children {
            match child {
                Child::Text(_) => {}
                Child::Expr(x) => {
                    if let Some(e) = x {
                        self.expr(e);
                    }
                }
                Child::Element(el) => self.jsx(el),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callee_is_member(e: &Expr) -> bool {
        match &e.data {
            ExprData::Call(c) => matches!(&c.callee.data, ExprData::Member(_)),
            _ => false,
        }
    }

    fn callee_is_ident(e: &Expr) -> bool {
        match &e.data {
            ExprData::Call(c) => matches!(&c.callee.data, ExprData::Ident(_)),
            _ => false,
        }
    }

    fn ret_expr(f: &Function, index: usize) -> &Expr {
        match &f.body {
            Body::Block(stmts) => match &stmts[index].data {
                StmtData::Ret(Some(e)) => e,
                _ => panic!("expected a return with a value"),
            },
            Body::Expr(_) => panic!("expected a block body"),
        }
    }

    #[test]
    fn react_compatibility_respects_shadowed_namespace_bindings() {
        let m = crate::parser::parse_ok(
            r#"import R from "react";
function f(R: number) { return R.useState(0); }
function g() { const R = 0; return R.useState(0); }
function h() { return R.useState(0); }"#,
        );
        assert!(callee_is_member(ret_expr(&m.functions[0].func, 0)));
        assert!(callee_is_member(ret_expr(&m.functions[1].func, 1)));
        assert!(callee_is_ident(ret_expr(&m.functions[2].func, 0)));
    }
}
