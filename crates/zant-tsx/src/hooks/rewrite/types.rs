//! Rewriting type parameters and type references of an inlined hook.

use super::*;

impl Rewriter<'_> {
    pub(super) fn type_params(
        &mut self,
        params: &[ast::TypeParam],
    ) -> Result<Vec<ast::TypeParam>, Failure> {
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
                ast::TypeDetail::Keyof(inner) => ast::TypeDetail::Keyof(self.type_ref(inner)?),
                ast::TypeDetail::Indexed(inner, key) => {
                    ast::TypeDetail::Indexed(self.type_ref(inner)?, key.clone())
                }
                ast::TypeDetail::NumberLit(value) => ast::TypeDetail::NumberLit(value.clone()),
                ast::TypeDetail::Typeof(name) => {
                    // a value name, resolved like any other free name
                    let resolved = match self.free.as_mut() {
                        Some(free) => free.resolve(&name.name, name.loc | loc_tag, Kind::Value)?,
                        None => name.name.clone(),
                    };
                    ast::TypeDetail::Typeof(ast::Name {
                        name: resolved,
                        loc: self.loc(name.loc),
                    })
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
