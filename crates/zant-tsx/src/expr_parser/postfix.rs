//! Postfix chains: members, optional chaining, calls with type arguments, indexes and arguments.

use super::*;

pub fn parse_postfix(p: &mut Parser) -> Result<Box<Expr>, Error> {
    let e = parse_primary(p)?;
    parse_postfix_on(p, e)
}

/// Members, calls, indexes, `!`, `++` and `as` after `e`.
pub(super) fn parse_postfix_on(p: &mut Parser, mut e: Box<Expr>) -> Result<Box<Expr>, Error> {
    loop {
        if p.is_p(".") {
            p.advance();
            let name = p.expect_ident("a property name")?;
            let loc = e.loc;
            e = p.expr(
                loc,
                ExprData::Member(Member {
                    object: e,
                    name,
                    optional: false,
                }),
            );
        } else if p.is_p("(") {
            let args = parse_args(p, &e)?;
            let loc = e.loc;
            e = p.expr(
                loc,
                ExprData::Call(Call {
                    is_new: false,
                    callee: e,
                    type_args: Vec::new(),
                    args,
                    optional: false,
                }),
            );
        } else if p.is_p("<") {
            let Some(type_args) = try_type_args(p) else {
                break;
            };
            let args = parse_args(p, &e)?;
            let loc = e.loc;
            e = p.expr(
                loc,
                ExprData::Call(Call {
                    is_new: false,
                    callee: e,
                    type_args,
                    args,
                    optional: false,
                }),
            );
        } else if p.is_p("?.") {
            p.advance();
            if p.is_p("[") {
                e = parse_index(p, e, true)?;
            } else if p.is_p("(") || p.is_p("<") {
                // `onSave?.(id)`: the codegen checks the callee is optional.
                let type_args = if p.is_p("<") {
                    crate::type_params::parse_args(p)?
                } else {
                    Vec::new()
                };
                let args = parse_args(p, &e)?;
                let loc = e.loc;
                e = p.expr(
                    loc,
                    ExprData::Call(Call {
                        is_new: false,
                        callee: e,
                        type_args,
                        args,
                        optional: true,
                    }),
                );
            } else {
                let name = p.expect_ident("a property name")?;
                let loc = e.loc;
                e = p.expr(
                    loc,
                    ExprData::Member(Member {
                        object: e,
                        name,
                        optional: true,
                    }),
                );
            }
        } else if p.is_p("[") {
            e = parse_index(p, e, false)?;
        } else if p.is_p("!") && !p.cur.nl_before {
            // `x!`: TypeScript's non-null assertion
            p.advance();
            let loc = e.loc;
            e = p.expr(loc, ExprData::NonNull(e));
        } else if (p.is_p("++") || p.is_p("--")) && !p.cur.nl_before {
            let op = p.text().as_bytes().to_vec();
            p.advance();
            let loc = e.loc;
            e = p.expr(loc, ExprData::Update(Update { op, target: e }));
        } else if p.is_kw("as") && !p.cur.nl_before {
            p.advance();
            if p.is_kw("const") {
                // Preserve literal inference; this subset does not model const types.
                p.advance();
                continue;
            }
            let ty = p.parse_type()?;
            let loc = e.loc;
            e = p.expr(
                loc,
                ExprData::As(As {
                    expr: e,
                    ty,
                    satisfies: false,
                }),
            );
        } else if p.is_kw("satisfies") && !p.cur.nl_before {
            // `expr satisfies T`: TypeScript 4.9's checked contextual type
            p.advance();
            let ty = p.parse_type()?;
            let loc = e.loc;
            e = p.expr(
                loc,
                ExprData::As(As {
                    expr: e,
                    ty,
                    satisfies: true,
                }),
            );
        } else {
            break;
        }
    }
    Ok(e)
}

/// `<Todo[]>(` after a callee: a type argument list, or null (nothing
/// consumed, no error kept) when `<` is a comparison.
pub fn try_type_args(p: &mut Parser) -> Option<Vec<TypeRef>> {
    let s = p.save();
    let saved_diag = p.diag.clone();
    if let Ok(list) = crate::type_params::parse_args(p)
        && p.is_p("(")
    {
        return Some(list);
    }
    p.restore(s);
    p.diag = saved_diag;
    None
}

/// `a[i]` (`a?.[i]` when `optional`); `cur` is the `[`.
pub(super) fn parse_index(
    p: &mut Parser,
    object: Box<Expr>,
    optional: bool,
) -> Result<Box<Expr>, Error> {
    p.advance(); // [
    let index = parse_expr(p)?;
    p.expect_p("]")?;
    let loc = object.loc;
    Ok(p.expr(
        loc,
        ExprData::Index(Index {
            object,
            index,
            optional,
        }),
    ))
}

/// Parse arguments, retaining method context for unsupported regex overload diagnostics.
pub fn parse_args(p: &mut Parser, callee: &Expr) -> Result<Vec<Box<Expr>>, Error> {
    p.expect_p("(")?;
    let mut args: Vec<Box<Expr>> = Vec::new();
    while !p.is_p(")") {
        if p.is_p("...") {
            // `f(...items)`: only a rest parameter accepts it (codegen checks)
            let loc = p.cur.start;
            p.advance();
            let inner = parse_assign(p)?;
            args.push(p.expr(loc, ExprData::Spread(inner)));
            if !p.eat_p(",") {
                break;
            }
            continue;
        }
        let arg = match parse_assign(p) {
            Ok(arg) => arg,
            Err(err) => {
                if err == Error::Syntax
                    && p.is_p("/")
                    && matches!(&callee.data, ExprData::Member(_))
                {
                    let member_name = match &callee.data {
                        ExprData::Member(m) => bytes_lossy(&m.name.name),
                        _ => String::new(),
                    };
                    p.diag = None;
                    return Err(p.fail(
                        p.cur.start,
                        format!(
                            "unknown string method `{member_name}` for a regex argument; supported (string patterns only): {SUPPORTED}"
                        ),
                    ));
                }
                return Err(err);
            }
        };
        args.push(arg);
        if !p.eat_p(",") {
            break;
        }
    }
    p.expect_p(")")?;
    Ok(args)
}
