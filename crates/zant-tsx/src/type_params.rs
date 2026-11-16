//! Shared generic lists for declarations, types, calls and JSX.
//! The lexer keeps every `>` separate, including nested `>>` / `>>>`.

use crate::ast::{TypeParam, TypeRef};
use crate::parser::{Error, Parser};

/// An optional type parameter list, including constraints and defaults.
pub fn parse_params(p: &mut Parser) -> Result<Vec<TypeParam>, Error> {
    if !p.eat_p("<") {
        return Ok(Vec::new());
    }
    let mut params = Vec::new();
    loop {
        let mut is_const = false;
        while p.is_kw("const") || variance_modifier(p) {
            is_const |= p.is_kw("const");
            p.advance();
        }
        let name = p.expect_ident("a type parameter name")?;
        let constraint = if p.is_kw("extends") {
            p.advance();
            Some(p.parse_type()?)
        } else {
            None
        };
        let default = if p.eat_p("=") {
            Some(p.parse_type()?)
        } else {
            None
        };
        params.push(TypeParam {
            name,
            constraint,
            default,
            is_const,
        });
        if !p.eat_p(",") || p.is_p(">") {
            break;
        }
    }
    p.expect_p(">")?;
    Ok(params)
}

// `in` and `out` are contextual: `type F<out> = out` names a parameter.
fn variance_modifier(p: &mut Parser) -> bool {
    if !p.is_kw("in") && !p.is_kw("out") {
        return false;
    }
    let state = p.save();
    p.advance();
    let result = p.cur.kind == crate::lexer::Kind::Identifier && !p.is_kw("extends");
    p.restore(state);
    result
}

/// A nonempty type argument list. Trailing commas follow TypeScript.
pub fn parse_args(p: &mut Parser) -> Result<Vec<TypeRef>, Error> {
    p.expect_p("<")?;
    let mut args = vec![p.parse_type()?];
    while p.eat_p(",") {
        if p.is_p(">") {
            break;
        }
        args.push(p.parse_type()?);
    }
    p.expect_p(">")?;
    Ok(args)
}

/// tsc's TSX disambiguation: a comma, a default or a real constraint after
/// the first parameter distinguishes a generic arrow from a JSX element.
pub fn starts_arrow(p: &mut Parser) -> bool {
    let state = p.save();
    p.advance(); // <
    if p.is_kw("const") {
        p.advance();
    }
    let result = if p.cur.kind != crate::lexer::Kind::Identifier {
        false
    } else {
        p.advance();
        if p.is_kw("extends") {
            p.advance();
            !p.is_p("=") && !p.is_p(">") && !p.is_p("/")
        } else {
            p.is_p(",") || p.is_p("=")
        }
    };
    p.restore(state);
    result
}
