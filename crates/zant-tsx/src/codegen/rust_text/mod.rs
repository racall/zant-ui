//! Rust identifiers, string literals, operator precedence, format text, CSS
//! color literals and the small source scanner used for label / usage
//! counting. A TSX name that collides with a Rust keyword becomes a
//! `_`-suffixed identifier (Rust raw identifiers are legal, but the usage
//! scanner and golden output stay simpler without `r#`).

use crate::ast;

use super::scope;
use super::types::{self, Type, Value};
use super::{Generator, Result, fail};

mod formatting;
mod names;
mod scan;
#[cfg(test)]
mod tests;

pub use self::formatting::*;
pub use self::names::*;
pub use self::scan::*;

/// Rust binding strength, from a conditional expression to a primary value.
/// `|` = 5 and assignment = 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Precedence {
    Assignment = 0,
    Conditional = 1,
    Range = 2,
    LogicalOr = 3,
    LogicalAnd = 4,
    BitOr = 5,
    BitXor = 6,
    BitAnd = 7,
    Comparison = 8,
    Shift = 9,
    Sum = 10,
    Product = 11,
    Cast = 12,
    Unary = 13,
    Suffix = 14,
    #[default]
    Primary = 20,
}

/// Parenthesize only operands whose binding strength would change the parse.
pub fn operand(v: &Value, parent: Precedence, equal: bool) -> String {
    let precedence = v.precedence;
    if precedence < parent || (equal && precedence == parent) || block_like(&v.code) {
        format!("({})", v.code)
    } else {
        v.code.clone()
    }
}

/// A block expression (`'label: { … }`, `{ … }`, `if`, `match`): at the start
/// of a statement Rust reads it as a statement, so `'s: { … } || b` would
/// end the expression after the block; as an operand it is parenthesized.
fn block_like(code: &str) -> bool {
    let code = code.trim_start();
    code.starts_with('\'')
        || code.starts_with('{')
        || code.starts_with("if ")
        || code.starts_with("match ")
}

/// Convert CSS RGB/RGBA literals into ARGB, or type-check numeric color
/// expressions.
pub fn color(g: &mut Generator<'_>, c: &mut scope::Context, e: &ast::Expr) -> Result<String> {
    if let ast::ExprData::Cond(cond) = &e.data {
        return Ok(format!(
            "if {} {{ {} }} else {{ {} }}",
            super::expr::condition(g, c, &cond.cond)?.code,
            color(g, c, &cond.then)?,
            color(g, c, &cond.else_)?
        ));
    }
    if let ast::ExprData::String(s) = &e.data {
        if s.is_empty() || s[0] != b'#' || (s.len() != 4 && s.len() != 7 && s.len() != 9) {
            return fail(g, e.loc, "color must be #rgb, #rrggbb, or #rrggbbaa");
        }
        let digits = match std::str::from_utf8(&s[1..])
            .ok()
            .and_then(|text| u32::from_str_radix(text, 16).ok())
        {
            Some(digits) => digits,
            None => return fail(g, e.loc, "invalid hexadecimal color"),
        };
        let argb: u32 = match s.len() {
            4 => {
                0xff00_0000
                    | (((digits >> 8) * 17) << 16)
                    | ((((digits >> 4) & 15) * 17) << 8)
                    | ((digits & 15) * 17)
            }
            7 => 0xff00_0000 | digits,
            9 => (digits >> 8) | ((digits & 255) << 24),
            _ => unreachable!("checked color literal length"),
        };
        return Ok(format!("0x{argb:08x}"));
    }
    Ok(super::expr::emit(g, c, e, Some(Type::U32), false)?.code)
}
