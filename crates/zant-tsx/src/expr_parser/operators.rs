//! Operators: binary precedence climbing, unary operators and `new`.

use super::*;

pub fn binary_prec(p: &mut Parser) -> Option<u8> {
    p.rescan_greater();
    // `key in record`: relational, like `<`
    if p.is_kw("in") || p.is_kw("instanceof") {
        return Some(8);
    }
    if p.cur.kind != Kind::Punct {
        return None;
    }
    let t = p.text();
    let table: [(&str, u8); 23] = [
        ("??", 1),
        ("||", 2),
        ("&&", 3),
        ("|", 4),
        ("^", 5),
        ("&", 6),
        ("==", 7),
        ("!=", 7),
        ("===", 7),
        ("!==", 7),
        ("<", 8),
        (">", 8),
        ("<=", 8),
        (">=", 8),
        ("<<", 9),
        (">>", 9),
        (">>>", 9),
        ("+", 10),
        ("-", 10),
        ("*", 11),
        ("/", 11),
        ("%", 11),
        ("**", 12),
    ];
    for (op, prec) in table {
        if t == op {
            return Some(prec);
        }
    }
    None
}

pub fn parse_binary(p: &mut Parser, min_prec: u8) -> Result<Box<Expr>, Error> {
    let mut lhs = parse_unary(p)?;
    while let Some(prec) = binary_prec(p) {
        if prec < min_prec {
            break;
        }
        let op = p.text().as_bytes().to_vec();
        p.advance();
        // `**` is right-associative, the rest left-associative
        let rhs = if op.as_slice() == b"**" {
            parse_binary(p, prec)?
        } else {
            parse_binary(p, prec + 1)?
        };
        let loc = lhs.loc;
        lhs = p.expr(loc, ExprData::Binary(Binary { op, lhs, rhs }));
    }
    Ok(lhs)
}

pub fn parse_unary(p: &mut Parser) -> Result<Box<Expr>, Error> {
    let loc = p.cur.start;
    for op in ["!", "-", "+", "~"] {
        if p.is_p(op) {
            p.advance();
            let operand = parse_unary(p)?;
            return Ok(p.expr(
                loc,
                ExprData::Unary(Unary {
                    op: op.as_bytes().to_vec(),
                    operand,
                }),
            ));
        }
    }
    for op in ["++", "--"] {
        if p.is_p(op) {
            p.advance();
            let target = parse_unary(p)?;
            return Ok(p.expr(
                loc,
                ExprData::Update(Update {
                    op: op.as_bytes().to_vec(),
                    target,
                }),
            ));
        }
    }
    if p.is_kw("delete") {
        // `delete record[key]` (a statement; codegen checks)
        p.advance();
        let operand = parse_unary(p)?;
        return Ok(p.expr(
            loc,
            ExprData::Unary(Unary {
                op: b"delete".to_vec(),
                operand,
            }),
        ));
    }
    if p.is_kw("new") {
        return parse_new(p, loc);
    }
    if p.is_kw("typeof") {
        p.advance();
        let operand = parse_unary(p)?;
        return Ok(p.expr(
            loc,
            ExprData::Unary(Unary {
                op: b"typeof".to_vec(),
                operand,
            }),
        ));
    }
    if p.is_kw("await") {
        if !p.in_async {
            return Err(p.fail(loc, "`await` is only valid inside an async function"));
        }
        p.advance();
        let operand = parse_unary(p)?;
        return Ok(p.expr(
            loc,
            ExprData::Unary(Unary {
                op: b"await".to_vec(),
                operand,
            }),
        ));
    }
    if p.is_kw("void") {
        return Err(p.fail(loc, "`void` is not supported"));
    }
    parse_postfix(p)
}

/// `new Map<K, V>(args)` / `new Set()` / `new Map`: a constructor call.
pub(super) fn parse_new(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
    p.advance(); // new
    let name = p.expect_ident("a class name after `new`")?;
    let callee = p.expr(name.loc, ExprData::Ident(name.name));
    let type_args = if p.is_p("<") {
        crate::type_params::parse_args(p)?
    } else {
        Vec::new()
    };
    let args = if p.is_p("(") {
        parse_args(p, &callee)?
    } else {
        Vec::new()
    };
    let call = p.expr(
        loc,
        ExprData::Call(Call {
            is_new: true,
            callee,
            type_args,
            args,
            optional: false,
        }),
    );
    // `new Map().set(…)`: members and calls may follow
    parse_postfix_on(p, call)
}
