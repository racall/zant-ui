//! Rewriting expressions, identifiers, members and JSX of an inlined hook.

use super::*;

impl Rewriter<'_> {
    pub(super) fn new_expr(&self, at: Loc, data: ast::ExprData) -> Box<ast::Expr> {
        Box::new(ast::Expr {
            loc: self.loc(at),
            data,
        })
    }

    #[allow(clippy::vec_box)] // AST expression lists hold boxed expressions
    pub(super) fn exprs(&mut self, es: &[Box<ast::Expr>]) -> Result<Vec<Box<ast::Expr>>, Failure> {
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
                        is_new: false,
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

    pub(super) fn ident(&mut self, at: Loc, n: &[u8]) -> Result<Box<ast::Expr>, Failure> {
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

    pub(super) fn member(
        &mut self,
        e: &ast::Expr,
        m: &ast::Member,
    ) -> Result<Box<ast::Expr>, Failure> {
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

    pub(super) fn jsx(&mut self, j: &ast::Jsx) -> Result<Box<ast::Jsx>, Failure> {
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
    pub(super) fn tag_name(&mut self, j: &ast::Jsx) -> Result<Vec<u8>, Failure> {
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
}
