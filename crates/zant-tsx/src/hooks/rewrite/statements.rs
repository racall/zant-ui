//! Rewriting statements, patterns and functions of an inlined hook.

use super::*;

impl Rewriter<'_> {
    /// Declares the names a block declares (functions are hoisted, so all of them first).
    pub fn predeclare(&mut self, ss: &[Box<ast::Stmt>]) -> Result<(), Failure> {
        for s in ss {
            match &s.data {
                ast::StmtData::Decl(d) => match &d.pattern {
                    ast::Pattern::Ident(n) => {
                        self.bind(&n.name);
                    }
                    ast::Pattern::Array(names) => {
                        for n in names.iter().flatten() {
                            self.bind(&n.name);
                        }
                    }
                    ast::Pattern::Object(bindings) => {
                        for b in bindings {
                            self.bind(&b.local.name);
                        }
                    }
                },
                ast::StmtData::Func(f) => {
                    self.bind(&f.name);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn statements(&mut self, ss: &[Box<ast::Stmt>]) -> Result<Vec<Box<ast::Stmt>>, Failure> {
        let mark = self.env.len();
        let result = (|| {
            self.predeclare(ss)?;
            let mut out = Vec::with_capacity(ss.len());
            for s in ss {
                out.push(self.stmt(s)?);
            }
            Ok(out)
        })();
        self.env.truncate(mark);
        result
    }

    pub(super) fn new_stmt(&self, at: Loc, data: ast::StmtData) -> Box<ast::Stmt> {
        Box::new(ast::Stmt {
            loc: self.loc(at),
            data,
        })
    }

    pub fn stmt(&mut self, s: &ast::Stmt) -> Result<Box<ast::Stmt>, Failure> {
        match &s.data {
            ast::StmtData::Decl(d) => {
                // the initializer reads the names before this declaration
                let init = self.opt(&d.init)?;
                let ty = match &d.ty {
                    Some(t) => Some(self.type_ref(t)?),
                    None => None,
                };
                let pattern = self.pattern(&d.pattern)?;
                Ok(self.new_stmt(
                    s.loc,
                    ast::StmtData::Decl(ast::Decl {
                        is_const: d.is_const,
                        pattern,
                        ty,
                        init,
                    }),
                ))
            }
            ast::StmtData::Func(f) => {
                let name = self.renamed(&ast::Name {
                    name: f.name.clone(),
                    loc: f.loc,
                })?;
                let mut copy = self.function(f)?;
                copy.name = name.name;
                Ok(self.new_stmt(s.loc, ast::StmtData::Func(copy)))
            }
            ast::StmtData::Ret(e) => {
                let e = self.opt(e)?;
                Ok(self.new_stmt(s.loc, ast::StmtData::Ret(e)))
            }
            ast::StmtData::If(v) => {
                let cond = self.expr(&v.cond)?;
                let then = self.scoped(&v.then)?;
                let else_ = match &v.else_ {
                    Some(el) => Some(self.scoped(el)?),
                    None => None,
                };
                Ok(self.new_stmt(s.loc, ast::StmtData::If(ast::If { cond, then, else_ })))
            }
            ast::StmtData::For(v) => {
                let mark = self.env.len();
                let result = (|| {
                    let init = match &v.init {
                        Some(i) => Some(self.stmt(i)?),
                        None => None,
                    };
                    let cond = self.opt(&v.cond)?;
                    let update = self.opt(&v.update)?;
                    let body = self.scoped(&v.body)?;
                    Ok(self.new_stmt(
                        s.loc,
                        ast::StmtData::For(ast::For {
                            init,
                            cond,
                            update,
                            body,
                        }),
                    ))
                })();
                self.env.truncate(mark);
                result
            }
            ast::StmtData::ForOf(v) => {
                let iterable = self.expr(&v.iterable)?;
                let mark = self.env.len();
                let result = (|| {
                    let name = self.bind(&v.name.name);
                    let body = self.scoped(&v.body)?;
                    Ok(self.new_stmt(
                        s.loc,
                        ast::StmtData::ForOf(ast::ForOf {
                            name: ast::Name {
                                name,
                                loc: self.loc(v.name.loc),
                            },
                            is_const: v.is_const,
                            iterable,
                            body,
                        }),
                    ))
                })();
                self.env.truncate(mark);
                result
            }
            ast::StmtData::While(v) => {
                let cond = self.expr(&v.cond)?;
                let body = self.scoped(&v.body)?;
                Ok(self.new_stmt(
                    s.loc,
                    ast::StmtData::While(ast::While {
                        cond,
                        body,
                        do_first: v.do_first,
                    }),
                ))
            }
            ast::StmtData::Switch(v) => {
                let value = self.expr(&v.value)?;
                let mark = self.env.len();
                let result = (|| {
                    for case in &v.cases {
                        self.predeclare(&case.body)?;
                    }
                    let mut cases = Vec::with_capacity(v.cases.len());
                    for case in &v.cases {
                        let mut body = Vec::with_capacity(case.body.len());
                        for b in &case.body {
                            body.push(self.stmt(b)?);
                        }
                        let value = self.opt(&case.value)?;
                        cases.push(ast::Case {
                            loc: self.loc(case.loc),
                            value,
                            body,
                        });
                    }
                    Ok(self.new_stmt(s.loc, ast::StmtData::Switch(ast::Switch { value, cases })))
                })();
                self.env.truncate(mark);
                result
            }
            ast::StmtData::Break => Ok(self.new_stmt(s.loc, ast::StmtData::Break)),
            ast::StmtData::Continue => Ok(self.new_stmt(s.loc, ast::StmtData::Continue)),
            ast::StmtData::Expr(e) => {
                let e = self.expr(e)?;
                Ok(self.new_stmt(s.loc, ast::StmtData::Expr(e)))
            }
            ast::StmtData::Block(ss) => {
                let ss = self.statements(ss)?;
                Ok(self.new_stmt(s.loc, ast::StmtData::Block(ss)))
            }
            ast::StmtData::Throw(e) => {
                let e = self.expr(e)?;
                Ok(self.new_stmt(s.loc, ast::StmtData::Throw(e)))
            }
            ast::StmtData::Try(t) => {
                let body = self.statements(&t.body)?;
                let catch = match &t.catch {
                    Some(c) => {
                        let mark = self.env.len();
                        let result = (|| {
                            let name = c.name.as_ref().map(|name| ast::Name {
                                name: self.bind(&name.name),
                                loc: self.loc(name.loc),
                            });
                            let body = self.statements(&c.body)?;
                            Ok(ast::Catch { name, body })
                        })();
                        self.env.truncate(mark);
                        Some(result?)
                    }
                    None => None,
                };
                let finally = match &t.finally {
                    Some(f) => Some(self.statements(f)?),
                    None => None,
                };
                Ok(self.new_stmt(
                    s.loc,
                    ast::StmtData::Try(ast::Try {
                        body,
                        catch,
                        finally,
                    }),
                ))
            }
        }
    }

    /// A statement in its own scope (the body of an `if` or a loop).
    pub(super) fn scoped(&mut self, s: &ast::Stmt) -> Result<Box<ast::Stmt>, Failure> {
        if matches!(s.data, ast::StmtData::Block(_)) {
            return self.stmt(s);
        }
        let mark = self.env.len();
        let result = self.stmt(s);
        self.env.truncate(mark);
        result
    }

    pub(super) fn pattern(&mut self, p: &ast::Pattern) -> Result<ast::Pattern, Failure> {
        match p {
            ast::Pattern::Ident(n) => Ok(ast::Pattern::Ident(self.renamed(n)?)),
            ast::Pattern::Array(names) => {
                let mut out = Vec::with_capacity(names.len());
                for n in names {
                    out.push(match n {
                        Some(v) => Some(self.renamed(v)?),
                        None => None,
                    });
                }
                Ok(ast::Pattern::Array(out))
            }
            ast::Pattern::Object(bindings) => {
                let mut out = Vec::with_capacity(bindings.len());
                for b in bindings {
                    out.push(ast::ObjectBinding {
                        key: b.key.clone(),
                        local: self.renamed(&b.local)?,
                    });
                }
                Ok(ast::Pattern::Object(out))
            }
        }
    }

    pub fn function(&mut self, f: &ast::Function) -> Result<Box<ast::Function>, Failure> {
        let mark = self.env.len();
        let type_mark = self.type_names.len();
        let result = (|| {
            let type_params = self.type_params(&f.type_params)?;
            let mut params = Vec::with_capacity(f.params.len());
            for p in &f.params {
                let mut o = p.clone();
                o.ty = match &p.ty {
                    Some(t) => Some(self.type_ref(t)?),
                    None => None,
                };
                o.default = self.opt(&p.default)?;
                if !p.destructure.is_empty() {
                    // `{ todo }: Props` reads props by these names: they keep them
                    let mut defaults = Vec::with_capacity(p.destructure_defaults.len());
                    for d in &p.destructure_defaults {
                        defaults.push(self.opt(d)?);
                    }
                    o.destructure_defaults = defaults;
                    for n in &p.destructure {
                        self.env.push(Entry {
                            name: n.local.name.clone(),
                            binding: Binding::Rename(n.local.name.clone()),
                        });
                    }
                    params.push(o);
                    continue;
                }
                o.name = self.renamed(&p.name)?;
                params.push(o);
            }
            let ret = match &f.ret {
                Some(r) => Some(self.type_ref(r)?),
                None => None,
            };
            let body = match &f.body {
                ast::Body::Block(ss) => ast::Body::Block(self.statements(ss)?),
                ast::Body::Expr(e) => ast::Body::Expr(self.expr(e)?),
            };
            Ok(Box::new(ast::Function {
                type_params,
                loc: self.loc(f.loc),
                name: f.name.clone(),
                params,
                ret,
                body,
            }))
        })();
        self.env.truncate(mark);
        self.type_names.truncate(type_mark);
        result
    }
}
