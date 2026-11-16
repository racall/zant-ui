//! Copies statements and expressions while renaming the bindings they
//! declare and replacing the names they read: how a custom hook's body is
//! inlined into a component (`expand.rs`), and how names a hook returned
//! are replaced in the rest of the component.
//!
//! The only error is [`Failure`]; copies own their nodes.

use crate::ast::{self, Loc};

/// Why inlining failed, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub loc: Loc,
    pub msg: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// Another name (a renamed local, an import alias).
    Rename(Vec<u8>),
    /// An expression (a hook's argument, a value it returned).
    Expr(Box<ast::Expr>),
    /// An object a hook returned: `v.key` is the property's value.
    Object(Vec<ast::Property>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: Vec<u8>,
    pub binding: Binding,
}

/// What a name the copied code reads without declaring it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Value,
    TypeName,
    Tag,
}

/// How a name the copied code reads without declaring it is resolved in the
/// copy (callers pass a trait object).
pub trait Free {
    fn resolve(&mut self, name: &[u8], loc: Loc, kind: Kind) -> Result<Vec<u8>, Failure>;
}

pub struct Rewriter<'a> {
    pub env: Vec<Entry>,
    /// Lexically bound type names, separate from value bindings.
    pub type_names: Vec<Vec<u8>>,
    /// Bindings declared in the copy get this prefix (null: they keep their names).
    pub prefix: Option<Vec<u8>>,
    /// Or-ed into every location (the file of inlined code, `expand.rs`).
    pub loc_tag: Loc,
    pub free: Option<&'a mut dyn Free>,
}

impl Rewriter<'static> {
    /// A rewriter that only replaces names from its environment (no inlining).
    pub fn empty() -> Self {
        Rewriter {
            env: Vec::new(),
            type_names: Vec::new(),
            prefix: None,
            loc_tag: 0,
            free: None,
        }
    }
}

fn fail(at: Loc, msg: impl Into<String>) -> Failure {
    Failure {
        loc: at,
        msg: msg.into(),
    }
}

fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

impl Rewriter<'_> {
    fn lookup(&self, name: &[u8]) -> Option<Binding> {
        self.env
            .iter()
            .rev()
            .find(|entry| entry.name == name)
            .map(|entry| entry.binding.clone())
    }

    /// Declares `name` in the copy; returns its new name.
    pub fn bind(&mut self, name: &[u8]) -> Vec<u8> {
        let new = match &self.prefix {
            Some(prefix) => {
                let mut new = prefix.clone();
                new.extend_from_slice(name);
                new
            }
            None => name.to_vec(),
        };
        self.env.push(Entry {
            name: name.to_vec(),
            binding: Binding::Rename(new.clone()),
        });
        new
    }

    fn renamed(&mut self, name: &ast::Name) -> Result<ast::Name, Failure> {
        let new = match self.lookup(&name.name) {
            Some(Binding::Rename(new)) => new,
            _ => self.bind(&name.name),
        };
        Ok(ast::Name {
            name: new,
            loc: self.loc(name.loc),
        })
    }

    fn loc(&self, l: Loc) -> Loc {
        l | self.loc_tag
    }

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

    fn opt(&mut self, e: &Option<Box<ast::Expr>>) -> Result<Option<Box<ast::Expr>>, Failure> {
        match e {
            Some(v) => Ok(Some(self.expr(v)?)),
            None => Ok(None),
        }
    }

    fn new_stmt(&self, at: Loc, data: ast::StmtData) -> Box<ast::Stmt> {
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
        }
    }

    /// A statement in its own scope (the body of an `if` or a loop).
    fn scoped(&mut self, s: &ast::Stmt) -> Result<Box<ast::Stmt>, Failure> {
        if matches!(s.data, ast::StmtData::Block(_)) {
            return self.stmt(s);
        }
        let mark = self.env.len();
        let result = self.stmt(s);
        self.env.truncate(mark);
        result
    }

    fn pattern(&mut self, p: &ast::Pattern) -> Result<ast::Pattern, Failure> {
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

    fn new_expr(&self, at: Loc, data: ast::ExprData) -> Box<ast::Expr> {
        Box::new(ast::Expr {
            loc: self.loc(at),
            data,
        })
    }

    #[allow(clippy::vec_box)] // AST expression lists hold boxed expressions
    fn exprs(&mut self, es: &[Box<ast::Expr>]) -> Result<Vec<Box<ast::Expr>>, Failure> {
        let mut out = Vec::with_capacity(es.len());
        for e in es {
            out.push(self.expr(e)?);
        }
        Ok(out)
    }

    pub fn expr(&mut self, e: &ast::Expr) -> Result<Box<ast::Expr>, Failure> {
        match &e.data {
            ast::ExprData::Number(_)
            | ast::ExprData::String(_)
            | ast::ExprData::Boolean(_)
            | ast::ExprData::NullLit => Ok(self.new_expr(e.loc, e.data.clone())),
            ast::ExprData::Ident(n) => self.ident(e.loc, n),
            ast::ExprData::Template(t) => {
                let quasis = t.quasis.clone();
                let exprs = self.exprs(&t.exprs)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Template(ast::Template { quasis, exprs }),
                ))
            }
            ast::ExprData::Unary(u) => {
                let operand = self.expr(&u.operand)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Unary(ast::Unary {
                        op: u.op.clone(),
                        operand,
                    }),
                ))
            }
            ast::ExprData::Binary(b) => {
                let lhs = self.expr(&b.lhs)?;
                let rhs = self.expr(&b.rhs)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Binary(ast::Binary {
                        op: b.op.clone(),
                        lhs,
                        rhs,
                    }),
                ))
            }
            ast::ExprData::Cond(v) => {
                let cond = self.expr(&v.cond)?;
                let then = self.expr(&v.then)?;
                let else_ = self.expr(&v.else_)?;
                Ok(self.new_expr(e.loc, ast::ExprData::Cond(ast::Cond { cond, then, else_ })))
            }
            ast::ExprData::Call(v) => {
                let mut type_args = Vec::with_capacity(v.type_args.len());
                for t in &v.type_args {
                    type_args.push(self.type_ref(t)?);
                }
                let callee = self.expr(&v.callee)?;
                let args = self.exprs(&v.args)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Call(ast::Call {
                        callee,
                        type_args,
                        args,
                        optional: false,
                    }),
                ))
            }
            ast::ExprData::Member(m) => self.member(e, m),
            ast::ExprData::Arrow(f) => {
                let f = self.function(f)?;
                Ok(self.new_expr(e.loc, ast::ExprData::Arrow(f)))
            }
            ast::ExprData::Jsx(j) => {
                let j = self.jsx(j)?;
                Ok(self.new_expr(e.loc, ast::ExprData::Jsx(j)))
            }
            ast::ExprData::Object(props) => {
                let mut out = Vec::with_capacity(props.len());
                for p in props {
                    out.push(ast::Property {
                        key: p.key.clone(),
                        value: self.expr(&p.value)?,
                        spread: p.spread,
                    });
                }
                Ok(self.new_expr(e.loc, ast::ExprData::Object(out)))
            }
            ast::ExprData::Array(items) => {
                let items = self.exprs(items)?;
                Ok(self.new_expr(e.loc, ast::ExprData::Array(items)))
            }
            ast::ExprData::Spread(inner) => {
                let inner = self.expr(inner)?;
                Ok(self.new_expr(e.loc, ast::ExprData::Spread(inner)))
            }
            ast::ExprData::NonNull(inner) => {
                let inner = self.expr(inner)?;
                Ok(self.new_expr(e.loc, ast::ExprData::NonNull(inner)))
            }
            ast::ExprData::Index(ix) => {
                let object = self.expr(&ix.object)?;
                let index = self.expr(&ix.index)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Index(ast::Index {
                        object,
                        index,
                        optional: ix.optional,
                    }),
                ))
            }
            ast::ExprData::As(cast) => {
                let expr = self.expr(&cast.expr)?;
                let ty = self.type_ref(&cast.ty)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::As(ast::As {
                        expr,
                        ty,
                        satisfies: cast.satisfies,
                    }),
                ))
            }
            ast::ExprData::Assign(v) => {
                let target = self.expr(&v.target)?;
                let value = self.expr(&v.value)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Assign(ast::Assign {
                        op: v.op.clone(),
                        target,
                        value,
                    }),
                ))
            }
            ast::ExprData::Update(v) => {
                let target = self.expr(&v.target)?;
                Ok(self.new_expr(
                    e.loc,
                    ast::ExprData::Update(ast::Update {
                        op: v.op.clone(),
                        target,
                    }),
                ))
            }
        }
    }

    fn ident(&mut self, at: Loc, n: &[u8]) -> Result<Box<ast::Expr>, Failure> {
        if let Some(binding) = self.lookup(n) {
            return match binding {
                Binding::Rename(new) => Ok(self.new_expr(at, ast::ExprData::Ident(new))),
                Binding::Expr(value) => Ok(value),
                Binding::Object(_) => Err(fail(
                    at | self.loc_tag,
                    format!(
                        "`{}` is the object a hook returned: read its fields (`{}.name`)",
                        text(n),
                        text(n)
                    ),
                )),
            };
        }
        let name = match self.free.as_mut() {
            Some(free) => free.resolve(n, at | self.loc_tag, Kind::Value)?,
            None => n.to_vec(),
        };
        Ok(self.new_expr(at, ast::ExprData::Ident(name)))
    }

    fn member(&mut self, e: &ast::Expr, m: &ast::Member) -> Result<Box<ast::Expr>, Failure> {
        if let ast::ExprData::Ident(n) = &m.object.data
            && let Some(Binding::Object(props)) = self.lookup(n)
        {
            for p in &props {
                if !p.spread && p.key.name == m.name.name {
                    return Ok(p.value.clone());
                }
            }
            return Err(fail(
                e.loc | self.loc_tag,
                format!("the hook's result has no field `{}`", text(&m.name.name)),
            ));
        }
        let object = self.expr(&m.object)?;
        Ok(self.new_expr(
            e.loc,
            ast::ExprData::Member(ast::Member {
                object,
                name: m.name.clone(),
                optional: m.optional,
            }),
        ))
    }

    fn jsx(&mut self, j: &ast::Jsx) -> Result<Box<ast::Jsx>, Failure> {
        let mut attrs = Vec::with_capacity(j.attrs.len());
        for a in &j.attrs {
            let value = self.opt(&a.value)?;
            attrs.push(ast::Attr {
                name: a.name.clone(),
                value,
                spread: a.spread,
            });
        }
        let mut children = Vec::with_capacity(j.children.len());
        for child in &j.children {
            children.push(match child {
                ast::Child::Text(t) => ast::Child::Text(t.clone()),
                ast::Child::Expr(v) => ast::Child::Expr(self.opt(v)?),
                ast::Child::Element(el) => ast::Child::Element(self.jsx(el)?),
            });
        }
        let name = self.tag_name(j)?;
        Ok(Box::new(ast::Jsx {
            type_args: j
                .type_args
                .iter()
                .map(|t| self.type_ref(t))
                .collect::<Result<_, _>>()?,
            loc: self.loc(j.loc),
            name,
            attrs,
            children,
        }))
    }

    /// `<Row>` names a component (or a tag) the hook's file imported.
    fn tag_name(&mut self, j: &ast::Jsx) -> Result<Vec<u8>, Failure> {
        let loc_tag = self.loc_tag;
        let Some(free) = self.free.as_mut() else {
            return Ok(j.name.clone());
        };
        if j.name.is_empty() {
            return Ok(j.name.clone());
        }
        let dot = j
            .name
            .iter()
            .position(|&b| b == b'.')
            .unwrap_or(j.name.len());
        let head = free.resolve(&j.name[..dot], j.loc | loc_tag, Kind::Tag)?;
        if head == j.name[..dot] {
            return Ok(j.name.clone());
        }
        let mut name = head;
        name.extend_from_slice(&j.name[dot..]);
        Ok(name)
    }

    fn type_params(&mut self, params: &[ast::TypeParam]) -> Result<Vec<ast::TypeParam>, Failure> {
        self.type_names
            .extend(params.iter().map(|p| p.name.name.clone()));
        params
            .iter()
            .map(|p| {
                Ok(ast::TypeParam {
                    name: ast::Name {
                        name: p.name.name.clone(),
                        loc: self.loc(p.name.loc),
                    },
                    constraint: p
                        .constraint
                        .as_ref()
                        .map(|t| self.type_ref(t))
                        .transpose()?,
                    default: p.default.as_ref().map(|t| self.type_ref(t)).transpose()?,
                    is_const: p.is_const,
                })
            })
            .collect()
    }

    pub fn type_ref(&mut self, t: &ast::TypeRef) -> Result<ast::TypeRef, Failure> {
        let loc_tag = self.loc_tag;
        let mut out = t.clone();
        out.loc = self.loc(t.loc);
        out.args = t
            .args
            .iter()
            .map(|t| self.type_ref(t))
            .collect::<Result<_, _>>()?;
        if !t.name.is_empty()
            && !t.name.contains(&b'.')
            && !self.type_names.contains(&t.name)
            && let Some(free) = self.free.as_mut()
        {
            out.name = free.resolve(&t.name, t.loc | loc_tag, Kind::TypeName)?;
        }
        if let Some(d) = &t.detail {
            let detail = match &**d {
                ast::TypeDetail::Array(item) => ast::TypeDetail::Array(self.type_ref(item)?),
                ast::TypeDetail::Union(members) => {
                    let mut out = Vec::with_capacity(members.len());
                    for m in members {
                        out.push(self.type_ref(m)?);
                    }
                    ast::TypeDetail::Union(out)
                }
                ast::TypeDetail::Intersection(members) => {
                    let mut out = Vec::with_capacity(members.len());
                    for m in members {
                        out.push(self.type_ref(m)?);
                    }
                    ast::TypeDetail::Intersection(out)
                }
                ast::TypeDetail::Tuple(members) => {
                    let mut out = Vec::with_capacity(members.len());
                    for m in members {
                        out.push(self.type_ref(m)?);
                    }
                    ast::TypeDetail::Tuple(out)
                }
                ast::TypeDetail::StringLit(value) => ast::TypeDetail::StringLit(value.clone()),
                ast::TypeDetail::Null => ast::TypeDetail::Null,
                ast::TypeDetail::Object(fields) => {
                    let mut out = Vec::with_capacity(fields.len());
                    for f in fields {
                        out.push(ast::TypeField {
                            name: f.name.clone(),
                            ty: self.type_ref(&f.ty)?,
                            optional: f.optional,
                        });
                    }
                    ast::TypeDetail::Object(out)
                }
                ast::TypeDetail::Func(f) => {
                    let mark = self.type_names.len();
                    let result = (|| {
                        let type_params = self.type_params(&f.type_params)?;
                        let mut params = Vec::with_capacity(f.params.len());
                        for p in &f.params {
                            let mut o = p.clone();
                            o.ty = p.ty.as_ref().map(|t| self.type_ref(t)).transpose()?;
                            params.push(o);
                        }
                        let ret = self.type_ref(&f.ret)?;
                        Ok(ast::TypeDetail::Func(ast::FuncType {
                            type_params,
                            params,
                            ret,
                        }))
                    })();
                    self.type_names.truncate(mark);
                    result?
                }
            };
            out.detail = Some(Box::new(detail));
        }
        Ok(out)
    }
}
