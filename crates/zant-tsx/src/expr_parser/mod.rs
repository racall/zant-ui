//! Expressions of the TSX subset: precedence climbing for binary operators,
//! arrow functions and generic calls found by backtracking (`Parser::save`),
//! template literals, object literals and the JSX entry point.

use crate::ast::{
    As, Assign, Binary, Call, Cond, Expr, ExprData, Function, Index, Member, Name, Param, Property,
    Template, TypeRef, Unary, Update,
};
use crate::jsx_parser;
use crate::lexer::{self, Kind, TemplateEnd};
use crate::parser::{Parser, bytes_lossy};
use crate::string_method_names::SUPPORTED;
use crate::type_parser;

mod arrows;
mod operators;
mod postfix;
mod primary;
#[cfg(test)]
mod tests;

pub use self::arrows::*;
pub use self::operators::*;
pub use self::postfix::*;
pub use self::primary::*;

pub type Error = crate::error::ParseError;

pub fn parse_expr(p: &mut Parser) -> Result<Box<Expr>, Error> {
    parse_assign(p)
}

pub fn parse_assign(p: &mut Parser) -> Result<Box<Expr>, Error> {
    if let Some(arrow) = try_arrow(p)? {
        return Ok(arrow);
    }
    let lhs = parse_cond(p)?;
    for op in [
        "=", "+=", "-=", "*=", "/=", "%=", "**=", "&=", "|=", "^=", "<<=", ">>=", ">>>=",
    ] {
        if p.is_p(op) {
            p.advance();
            let value = parse_assign(p)?;
            let loc = lhs.loc;
            return Ok(p.expr(
                loc,
                ExprData::Assign(Assign {
                    op: op.as_bytes().to_vec(),
                    target: lhs,
                    value,
                }),
            ));
        }
    }
    Ok(lhs)
}

pub fn parse_cond(p: &mut Parser) -> Result<Box<Expr>, Error> {
    let c = parse_binary(p, 0)?;
    if !p.is_p("?") {
        return Ok(c);
    }
    p.advance();
    let then = parse_assign(p)?;
    p.expect_p(":")?;
    let else_ = parse_assign(p)?;
    let loc = c.loc;
    Ok(p.expr(
        loc,
        ExprData::Cond(Cond {
            cond: c,
            then,
            else_,
        }),
    ))
}
