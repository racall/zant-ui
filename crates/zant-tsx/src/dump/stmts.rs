//! Dumping statements, switch cases and binding patterns.

use super::*;

impl Dumper {
    pub(super) fn stmt(&mut self, s: &Stmt) {
        match &s.data {
            StmtData::Decl(decl) => {
                self.line_fmt(format!("stmt loc={} decl const={}", s.loc, decl.is_const));
                self.depth += 1;
                self.pattern(&decl.pattern);
                if let Some(ty) = &decl.ty {
                    self.wrapped("ty", |d| d.type_ref(ty));
                }
                if let Some(e) = &decl.init {
                    self.wrapped("init", |d| d.expr(e));
                }
                self.depth -= 1;
            }
            StmtData::Func(f) => {
                self.line_fmt(format!("stmt loc={} func", s.loc));
                self.depth += 1;
                self.function(f);
                self.depth -= 1;
            }
            StmtData::Ret(e) => {
                self.line_fmt(format!("stmt loc={} ret", s.loc));
                self.depth += 1;
                if let Some(e) = e {
                    self.expr(e);
                }
                self.depth -= 1;
            }
            StmtData::If(v) => {
                self.line_fmt(format!("stmt loc={} if", s.loc));
                self.depth += 1;
                self.wrapped("cond", |d| d.expr(&v.cond));
                self.wrapped("then", |d| d.stmt(&v.then));
                if let Some(el) = &v.else_ {
                    self.wrapped("else", |d| d.stmt(el));
                }
                self.depth -= 1;
            }
            StmtData::For(v) => {
                self.line_fmt(format!("stmt loc={} for", s.loc));
                self.depth += 1;
                if let Some(init) = &v.init {
                    self.wrapped("init", |d| d.stmt(init));
                }
                if let Some(cond) = &v.cond {
                    self.wrapped("cond", |d| d.expr(cond));
                }
                if let Some(update) = &v.update {
                    self.wrapped("update", |d| d.expr(update));
                }
                self.wrapped("body", |d| d.stmt(&v.body));
                self.depth -= 1;
            }
            StmtData::ForOf(v) => {
                self.line_fmt(format!(
                    "stmt loc={} for-of name={} const={}",
                    s.loc,
                    quote(&v.name.name),
                    v.is_const
                ));
                self.depth += 1;
                self.wrapped("iterable", |d| d.expr(&v.iterable));
                self.wrapped("body", |d| d.stmt(&v.body));
                self.depth -= 1;
            }
            StmtData::While(v) => {
                self.line_fmt(format!("stmt loc={} while do-first={}", s.loc, v.do_first));
                self.depth += 1;
                self.wrapped("cond", |d| d.expr(&v.cond));
                self.wrapped("body", |d| d.stmt(&v.body));
                self.depth -= 1;
            }
            StmtData::Switch(v) => {
                self.line_fmt(format!("stmt loc={} switch", s.loc));
                self.depth += 1;
                self.wrapped("value", |d| d.expr(&v.value));
                for case in &v.cases {
                    self.case(case);
                }
                self.depth -= 1;
            }
            StmtData::Break => self.line_fmt(format!("stmt loc={} break", s.loc)),
            StmtData::Continue => self.line_fmt(format!("stmt loc={} continue", s.loc)),
            StmtData::Expr(e) => {
                self.line_fmt(format!("stmt loc={} expr", s.loc));
                self.depth += 1;
                self.expr(e);
                self.depth -= 1;
            }
            StmtData::Block(ss) => {
                self.line_fmt(format!("stmt loc={} block", s.loc));
                self.depth += 1;
                for s in ss {
                    self.stmt(s);
                }
                self.depth -= 1;
            }
            StmtData::Throw(e) => {
                self.line_fmt(format!("stmt loc={} throw", s.loc));
                self.depth += 1;
                self.expr(e);
                self.depth -= 1;
            }
            StmtData::Try(t) => {
                self.line_fmt(format!("stmt loc={} try", s.loc));
                self.depth += 1;
                for s in &t.body {
                    self.stmt(s);
                }
                if let Some(c) = &t.catch {
                    let name = c.name.as_ref().map(|n| quote(&n.name)).unwrap_or_default();
                    self.line_fmt(format!("catch {name}"));
                    for s in &c.body {
                        self.stmt(s);
                    }
                }
                if let Some(f) = &t.finally {
                    self.line("finally");
                    for s in f {
                        self.stmt(s);
                    }
                }
                self.depth -= 1;
            }
        }
    }

    pub(super) fn case(&mut self, c: &Case) {
        self.line_fmt(format!("case loc={}", c.loc));
        self.depth += 1;
        if let Some(value) = &c.value {
            self.wrapped("test", |d| d.expr(value));
        }
        self.section("body", |d| {
            for s in &c.body {
                d.stmt(s);
            }
        });
        self.depth -= 1;
    }

    pub(super) fn pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Ident(n) => {
                self.line_fmt(format!("pattern ident {} loc={}", quote(&n.name), n.loc));
            }
            Pattern::Array(names) => {
                self.line("pattern array");
                self.depth += 1;
                for n in names {
                    match n {
                        Some(n) => self.line_fmt(format!("binding {}", quote(&n.name))),
                        None => self.line("hole"),
                    }
                }
                self.depth -= 1;
            }
            Pattern::Object(bindings) => {
                self.line("pattern object");
                self.depth += 1;
                for b in bindings {
                    self.object_binding(b);
                }
                self.depth -= 1;
            }
        }
    }

    pub(super) fn object_binding(&mut self, b: &ObjectBinding) {
        self.line_fmt(format!(
            "object-binding key={} local={} loc={}",
            quote(&b.key.name),
            quote(&b.local.name),
            b.key.loc
        ));
    }
}
