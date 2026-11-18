//! Where a hook call's result goes: arguments bound to parameters, the returned value given to the call site, aliases.

use super::*;

pub(super) fn new_stmt(loc: Loc, data: StmtData) -> Box<Stmt> {
    Box::new(Stmt { loc, data })
}

pub(super) fn decl_stmt(
    name: &[u8],
    loc: Loc,
    ty: Option<ast::TypeRef>,
    value: Box<Expr>,
) -> Box<Stmt> {
    new_stmt(
        loc,
        StmtData::Decl(ast::Decl {
            is_const: true,
            pattern: ast::Pattern::Ident(ast::Name {
                name: name.to_vec(),
                loc,
            }),
            ty,
            init: Some(value),
        }),
    )
}

pub(super) fn undefined_expr(loc: Loc) -> Box<Expr> {
    Box::new(Expr {
        loc,
        data: ExprData::Ident(b"undefined".to_vec()),
    })
}

/// Parameters become the arguments: names and literals are used as
/// they are, functions and other values become constants.
#[allow(clippy::vec_box)] // AST statement lists hold boxed statements
pub(super) fn arguments(
    rw: &mut Rewriter<'_>,
    at: &Site,
    out: &mut Vec<Box<Stmt>>,
    locals: &Locals,
    h: &ast::Function,
) -> Result<(), Error> {
    if at.call.args.len() > h.params.len() {
        return Err(Error {
            loc: at.loc,
            msg: format!("`{}` takes {} arguments", text(&h.name), h.params.len()),
        });
    }
    if !at.call.type_args.is_empty() {
        return Err(Error {
            loc: at.loc,
            msg: format!("`{}` takes no type arguments", text(&h.name)),
        });
    }
    let mut bindings: Vec<Entry> = Vec::new();
    for (i, p) in h.params.iter().enumerate() {
        if !p.destructure.is_empty() {
            return Err(Error {
                loc: p.name.loc | rw.loc_tag,
                msg: "a custom hook's parameters are plain names".to_string(),
            });
        }
        let given = i < at.call.args.len();
        if given && matches!(at.call.args[i].data, ExprData::Spread(_)) {
            return Err(Error {
                loc: at.call.args[i].loc,
                msg: "spread arguments are not supported".to_string(),
            });
        }
        // defaults are the hook's code (its file's names)
        let value: Box<Expr> = if given {
            at.call.args[i].clone()
        } else if let Some(d) = &p.default {
            rw.expr(d)?
        } else if p.optional {
            undefined_expr(at.loc)
        } else {
            return Err(Error {
                loc: at.loc,
                msg: format!("`{}` takes {} arguments", text(&h.name), h.params.len()),
            });
        };
        if simple(&value) {
            bindings.push(Entry {
                name: p.name.name.clone(),
                binding: Binding::Expr(value),
            });
            continue;
        }
        let mut name = rw
            .prefix
            .clone()
            .expect("a rewriter inlining a hook has a prefix");
        name.extend_from_slice(&p.name.name);
        let ty = match &p.ty {
            Some(t) => Some(rw.type_ref(t)?),
            None => None,
        };
        let ty = if matches!(value.data, ExprData::Arrow(_)) {
            None
        } else {
            ty
        };
        out.push(decl_stmt(&name, value.loc, ty, value));
        locals.borrow_mut().insert(name.clone());
        bindings.push(Entry {
            name: p.name.name.clone(),
            binding: Binding::Rename(name),
        });
    }
    rw.env.extend(bindings);
    Ok(())
}

/// The caller's names for what the hook returned.
#[allow(clippy::vec_box)] // AST statement lists hold boxed statements
pub(super) fn result(
    aliases: &mut Rewriter<'_>,
    at: &Site,
    value: Option<Box<Expr>>,
    out: &mut Vec<Box<Stmt>>,
) -> Result<(), Error> {
    let h = &at.hook.func;
    match &at.target {
        Target::None => {
            if let Some(v) = value
                && matches!(v.data, ExprData::Call(_))
            {
                out.push(new_stmt(at.loc, StmtData::Expr(v)));
            }
        }
        Target::Ident(n) => {
            let Some(v) = value else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!("`{}` returns nothing", text(&h.name)),
                });
            };
            if let ExprData::Object(props) = &v.data {
                alias(aliases, &n.name, Binding::Object(props.clone()))?;
            } else {
                give(aliases, n, v, at.ty.clone(), out)?;
            }
        }
        Target::Array(names) => {
            let Some(v) = value else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!("`{}` returns nothing", text(&h.name)),
                });
            };
            let ExprData::Array(items) = &v.data else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!(
                        "`{}` must return an array literal (`return [a, b]`) to destructure it",
                        text(&h.name)
                    ),
                });
            };
            for (i, n) in names.iter().enumerate() {
                if let Some(name) = n {
                    if i >= items.len() {
                        return Err(Error {
                            loc: name.loc,
                            msg: format!("`{}` returns {} values", text(&h.name), items.len()),
                        });
                    }
                    give(aliases, name, items[i].clone(), None, out)?;
                }
            }
        }
        Target::Object(bindings) => {
            let Some(v) = value else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!("`{}` returns nothing", text(&h.name)),
                });
            };
            let ExprData::Object(props) = &v.data else {
                return Err(Error {
                    loc: at.loc,
                    msg: format!(
                        "`{}` must return an object literal (`return {{ a, b }}`) to destructure it",
                        text(&h.name)
                    ),
                });
            };
            for b in bindings {
                let field = props
                    .iter()
                    .find(|p| !p.spread && p.key.name == b.key.name)
                    .map(|p| p.value.clone());
                let Some(field) = field else {
                    return Err(Error {
                        loc: b.key.loc,
                        msg: format!("`{}` returns no `{}`", text(&h.name), text(&b.key.name)),
                    });
                };
                give(aliases, &b.local, field, None, out)?;
            }
        }
    }
    Ok(())
}

/// `name` stands for `value` in the rest of the component: replaced
/// when it is a name or a literal, else a constant.
#[allow(clippy::vec_box)] // AST statement lists hold boxed statements
pub(super) fn give(
    aliases: &mut Rewriter<'_>,
    name: &ast::Name,
    value: Box<Expr>,
    ty: Option<ast::TypeRef>,
    out: &mut Vec<Box<Stmt>>,
) -> Result<(), Error> {
    if simple(&value) && ty.is_none() {
        return alias(aliases, &name.name, Binding::Expr(value));
    }
    out.push(decl_stmt(&name.name, name.loc, ty, value));
    Ok(())
}

/// The rest of the component reads `name` as `binding`. Earlier aliases
/// may name it (a hook returned a value of a hook it called): they are
/// updated, so every alias is final.
pub(super) fn alias(
    aliases: &mut Rewriter<'_>,
    name: &[u8],
    binding: Binding,
) -> Result<(), Error> {
    let mut one = Rewriter::empty();
    one.env.push(Entry {
        name: name.to_vec(),
        binding: binding.clone(),
    });
    for entry in aliases.env.iter_mut() {
        match &mut entry.binding {
            Binding::Rename(_) => {}
            Binding::Expr(e) => {
                *e = one.expr(e)?;
            }
            Binding::Object(props) => {
                let mut copy = Vec::with_capacity(props.len());
                for p in props.iter() {
                    copy.push(ast::Property {
                        key: p.key.clone(),
                        value: one.expr(&p.value)?,
                        spread: p.spread,
                    });
                }
                *props = copy;
            }
        }
    }
    aliases.env.push(Entry {
        name: name.to_vec(),
        binding,
    });
    Ok(())
}

/// Names, literals and member paths: substituting them changes nothing.
pub(super) fn simple(e: &Expr) -> bool {
    match &e.data {
        ExprData::Ident(_)
        | ExprData::Number(_)
        | ExprData::String(_)
        | ExprData::Boolean(_)
        | ExprData::NullLit => true,
        ExprData::Member(m) => simple(&m.object),
        ExprData::NonNull(inner) => simple(inner),
        ExprData::Template(t) => t.exprs.is_empty(),
        _ => false,
    }
}

pub(super) fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}
