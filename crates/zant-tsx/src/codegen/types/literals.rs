//! Literal checks: numeric literals and names, integer parsing and the literal predicates.

use super::*;

/// Reject an out-of-range integer literal at its TSX source position.
pub fn check_literal(g: &mut Generator<'_>, e: &ast::Expr, t: Type) -> Result<()> {
    if !t.integer() {
        return Ok(());
    }
    let raw: Vec<u8> = match &e.data {
        ast::ExprData::Number(n) => n.clone(),
        ast::ExprData::Unary(u) => match negative_literal(u) {
            Some(raw) => raw.into_bytes(),
            None => return Ok(()),
        },
        _ => return Ok(()),
    };
    if float_literal(&raw) {
        return Ok(());
    }
    let Some(n) = parse_integer(&raw) else {
        return fail(
            g,
            e.loc,
            format!("integer literal does not fit {}", t.rust(g)),
        );
    };
    let Some(info) = t.int_info() else {
        return Ok(());
    };
    let bound: i128 = 1i128 << (info.bits - if info.signed { 1 } else { 0 });
    let fits = if info.signed {
        n >= -bound && n < bound
    } else {
        n >= 0 && n < bound
    };
    if !fits {
        return fail(
            g,
            e.loc,
            format!(
                "integer literal does not fit {}; choose a wider or signed type",
                t.rust(g)
            ),
        );
    }
    Ok(())
}

/// Parse a TSX integer literal (decimal or `0x`/`0o`/`0b`, with `_`
/// separators) into an `i128`; `None` when it does not fit.
pub fn parse_integer(raw: &[u8]) -> Option<i128> {
    let text = std::str::from_utf8(raw).ok()?.replace('_', "");
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1i128, rest),
        None => (1i128, text.strip_prefix('+').unwrap_or(&text)),
    };
    let (radix, digits) = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, rest)
    } else if let Some(rest) = digits
        .strip_prefix("0o")
        .or_else(|| digits.strip_prefix("0O"))
    {
        (8, rest)
    } else if let Some(rest) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        (2, rest)
    } else {
        (10, digits)
    };
    i128::from_str_radix(digits, radix).ok().map(|n| sign * n)
}

pub fn numeric_name(n: &[u8]) -> Option<Type> {
    let ty = match n {
        b"u8" => Type::U8,
        b"u16" => Type::U16,
        b"u32" => Type::U32,
        b"u64" => Type::U64,
        b"i8" => Type::I8,
        b"i16" => Type::I16,
        b"i32" => Type::I32,
        b"i64" => Type::I64,
        b"usize" => Type::Usize,
        b"isize" => Type::Isize,
        b"f32" => Type::F32,
        b"f64" => Type::F64,
        _ => return None,
    };
    Some(ty)
}

pub fn float_literal(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let n = text.trim_start_matches(['+', '-']);
    let hexadecimal = n.starts_with("0x") || n.starts_with("0X");
    !hexadecimal && n.contains(['.', 'e', 'E'])
}

pub fn arithmetic(op: &[u8]) -> bool {
    matches!(op, b"+" | b"-" | b"*" | b"/" | b"%")
}

pub fn numeric_literals(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Number(_) => true,
        ast::ExprData::NonNull(inner) => numeric_literals(inner),
        ast::ExprData::Unary(u) => u.op != b"!" && numeric_literals(&u.operand),
        ast::ExprData::Binary(b) => {
            arithmetic(&b.op) && numeric_literals(&b.lhs) && numeric_literals(&b.rhs)
        }
        ast::ExprData::Cond(v) => numeric_literals(&v.then) && numeric_literals(&v.else_),
        _ => false,
    }
}

pub fn is_literal(e: &ast::Expr) -> bool {
    match &e.data {
        ast::ExprData::Number(_) | ast::ExprData::String(_) | ast::ExprData::Boolean(_) => true,
        ast::ExprData::Unary(u) => {
            u.op == b"-" && matches!(u.operand.data, ast::ExprData::Number(_))
        }
        _ => false,
    }
}

/// Recognize only the supported minus-number literal shape for range checks.
pub(super) fn negative_literal(unary: &ast::Unary) -> Option<String> {
    if unary.op != b"-" {
        return None;
    }
    let ast::ExprData::Number(n) = &unary.operand.data else {
        return None;
    };
    Some(format!("-{}", String::from_utf8_lossy(n)))
}

/// Element type names accepted both in annotations and Zant / React named
/// imports.
pub fn node_name(name: &[u8]) -> bool {
    matches!(
        name,
        b"JSX.Element"
            | b"React.ReactNode"
            | b"ReactNode"
            | b"React.ReactElement"
            | b"ReactElement"
    )
}
