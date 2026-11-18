//! Arrow functions: telling an arrow from a parenthesized expression and parsing its body.

use super::*;

/// `x => ...`, `() => ...`, `(a: T, b) => ...`, `(t: Todo): string => ...`;
/// null when the tokens are not an arrow function (nothing consumed).
/// Arrows are recognised before parsing them (tsc's approach), so errors
/// inside their parameters are reported as such.
pub fn try_arrow(p: &mut Parser) -> Result<Option<Box<Expr>>, Error> {
    // `async x => …`, `async (x) => …`; otherwise `async` is a name
    if p.is_kw("async") {
        let s = p.save();
        p.advance();
        if !p.cur.nl_before
            && (p.cur.kind == Kind::Identifier || p.is_p("(") || p.is_p("<"))
            && let Some(arrow) = try_arrow_with(p, s.cur.start, true)?
        {
            return Ok(Some(arrow));
        }
        p.restore(s);
    }
    let loc = p.cur.start;
    try_arrow_with(p, loc, false)
}

pub(super) fn try_arrow_with(
    p: &mut Parser,
    loc: u32,
    is_async: bool,
) -> Result<Option<Box<Expr>>, Error> {
    if p.cur.kind == Kind::Identifier {
        let s = p.save();
        let name = Name {
            name: p.text().as_bytes().to_vec(),
            loc: p.cur.start,
        };
        p.advance();
        if !p.is_p("=>") || p.cur.nl_before {
            p.restore(s);
            return Ok(None);
        }
        let params = vec![Param {
            name,
            ty: None,
            optional: false,
            destructure: Vec::new(),
            destructure_defaults: Vec::new(),
            rest: false,
            default: None,
        }];
        let func = arrow_body(p, loc, params, None, is_async)?;
        return Ok(Some(p.expr(loc, ExprData::Arrow(func))));
    }
    let generic = p.is_p("<") && crate::type_params::starts_arrow(p);
    let type_params = if generic {
        crate::type_params::parse_params(p)?
    } else {
        Vec::new()
    };
    if !p.is_p("(") {
        if generic {
            return Err(p.unexpected("("));
        }
        return Ok(None);
    }
    if !generic
        && !definitely_arrow(p)
        && !type_parser::followed_by_arrow(p)
        && !return_typed_arrow(p)
    {
        return Ok(None);
    }
    let params = p.parse_params()?;
    let ret = p.parse_return_type()?;
    if !p.is_p("=>") {
        return Err(p.unexpected("=>"));
    }
    let mut func = arrow_body(p, loc, params, ret, is_async)?;
    func.type_params = type_params;
    Ok(Some(p.expr(loc, ExprData::Arrow(func))))
}

/// `(...): Type =>`: the matching `)` is followed by a return type and `=>`
/// (tried speculatively; nothing consumed, no error kept).
pub(super) fn return_typed_arrow(p: &mut Parser) -> bool {
    let s = p.save();
    let saved_diag = p.diag.clone();
    let result = return_typed_arrow_inner(p);
    p.restore(s);
    p.diag = saved_diag;
    result
}

pub(super) fn return_typed_arrow_inner(p: &mut Parser) -> bool {
    let mut depth: usize = 0;
    while p.cur.kind != Kind::Eof {
        if p.is_p("(") || p.is_p("[") || p.is_p("{") {
            depth += 1;
        } else if p.is_p(")") || p.is_p("]") || p.is_p("}") {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        p.advance();
    }
    if !p.is_p(")") {
        return false;
    }
    p.advance();
    if !p.is_p(":") {
        return false;
    }
    p.advance();
    if p.parse_type().is_err() {
        return false;
    }
    p.is_p("=>")
}

/// tsc's quick check (`isParenthesizedArrowFunctionExpression`): `() =>`,
/// `():` and `(a:` can only start an arrow function. Other shapes (`(a)`,
/// `(a,`) are tried speculatively by `try_arrow`.
pub(super) fn definitely_arrow(p: &mut Parser) -> bool {
    let s = p.save();
    p.advance(); // (
    let result = if p.is_p(")") {
        p.advance();
        p.is_p("=>") || p.is_p(":")
    } else if p.cur.kind != Kind::Identifier {
        false
    } else {
        p.advance();
        p.is_p(":")
    };
    p.restore(s);
    result
}

pub fn arrow_body(
    p: &mut Parser,
    loc: u32,
    params: Vec<Param>,
    ret: Option<TypeRef>,
    is_async: bool,
) -> Result<Box<Function>, Error> {
    p.advance(); // =>
    let outer = std::mem::replace(&mut p.in_async, is_async);
    let body = if p.is_p("{") {
        p.parse_block().map(crate::ast::Body::Block)
    } else {
        parse_assign(p).map(crate::ast::Body::Expr)
    };
    p.in_async = outer;
    let mut function = Box::new(Function {
        type_params: Vec::new(),
        loc,
        name: Vec::new(),
        params,
        ret,
        body: body?,
    });
    if is_async {
        crate::async_lower::lower(p, &mut function)?;
    }
    Ok(function)
}
