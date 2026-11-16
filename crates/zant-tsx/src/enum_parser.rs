//! TypeScript `enum` / `const enum` declarations, lowered to what tsc's
//! output means for this subset:
//!
//! - a string enum `enum Mode { Light = "light", Dark = "dark" }` is the
//!   string-literal union `"light" | "dark"` (the type `Mode`), and
//!   `Mode.Dark` is `"dark"` (printed as such in templates);
//! - a numeric enum `enum Level { Low, High = 10, Max }` is `number` (the
//!   type `Level`) with tsc's auto-increment (0, 10, 11).
//!
//! Members are inlined where they are read (`codegen::enums`), as tsc does
//! for `const enum`. Heterogeneous enums, computed members and reverse
//! mappings (`Level[0]`) are not supported and say so.

use crate::ast::{EnumDecl, EnumMember, EnumValue, Name, TypeDecl, TypeDetail, TypeRef};
use crate::lexer::{self, Kind};
use crate::parser::{Error, Parser};

/// What one `enum` declaration adds to the module: its type and members.
pub struct Lowered {
    pub ty: TypeDecl,
    pub decl: EnumDecl,
}

enum Value {
    Text(Vec<u8>),
    Number(f64),
}

/// `enum Name { … }` with the parser at `enum` (`const` already consumed).
pub fn parse(p: &mut Parser, exported: bool) -> Result<Lowered, Error> {
    p.advance(); // enum
    let name = p.expect_ident("an enum name")?;
    p.expect_p("{")?;
    let mut members: Vec<(Name, Value)> = Vec::new();
    while !p.is_p("}") {
        if p.cur.kind == Kind::String {
            return Err(p.fail(
                p.cur.start,
                "enum members are identifiers here; rename the member",
            ));
        }
        let member = p.expect_ident("an enum member")?;
        if members.iter().any(|(known, _)| known.name == member.name) {
            return Err(p.fail(member.loc, "duplicate enum member"));
        }
        let value = if p.eat_p("=") {
            let loc = p.cur.start;
            let value = initializer(p)?;
            // `A = 1 + 2`: an expression, not a literal
            if !p.is_p(",") && !p.is_p("}") {
                return Err(p.fail(
                    loc,
                    "enum members are string or number literals; computed members are not supported",
                ));
            }
            value
        } else {
            match members.last() {
                None => Value::Number(0.0),
                Some((_, Value::Number(previous))) => Value::Number(previous + 1.0),
                Some((_, Value::Text(_))) => {
                    return Err(p.fail(
                        member.loc,
                        "a member after a string member needs a value (`Member = \"…\"`)",
                    ));
                }
            }
        };
        members.push((member, value));
        if !p.eat_p(",") {
            break;
        }
    }
    p.expect_p("}")?;
    if members.is_empty() {
        return Err(p.fail(name.loc, "an enum needs at least one member"));
    }
    let text = members
        .iter()
        .all(|(_, value)| matches!(value, Value::Text(_)));
    let number = members
        .iter()
        .all(|(_, value)| matches!(value, Value::Number(_)));
    if !text && !number {
        return Err(p.fail(
            name.loc,
            "enums mixing string and number members are not supported; use one kind",
        ));
    }
    Ok(lower(name, members, exported, text))
}

/// A string literal or a (negative) number literal.
fn initializer(p: &mut Parser) -> Result<Value, Error> {
    let loc = p.cur.start;
    let negative = p.eat_p("-");
    match p.cur.kind {
        Kind::String if !negative => {
            let text = lexer::decode_string(p.text().as_bytes())
                .map_err(|_| p.fail(loc, "invalid escape in string"))?;
            p.advance();
            Ok(Value::Text(text))
        }
        Kind::Number => {
            let text = p.text().replace('_', "");
            let value = parse_number(&text).ok_or_else(|| p.fail(loc, "invalid number"))?;
            p.advance();
            Ok(Value::Number(if negative { -value } else { value }))
        }
        _ => Err(p.fail(
            loc,
            "enum members are string or number literals; computed members are not supported",
        )),
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(digits) = lower.strip_prefix(prefix) {
            return u64::from_str_radix(digits, radix).ok().map(|n| n as f64);
        }
    }
    text.parse().ok()
}

fn named(name: &[u8], loc: u32) -> TypeRef {
    TypeRef {
        name: name.to_vec(),
        loc,
        detail: None,
        args: Vec::new(),
    }
}

fn detailed(loc: u32, detail: TypeDetail) -> TypeRef {
    TypeRef {
        name: Vec::new(),
        loc,
        detail: Some(Box::new(detail)),
        args: Vec::new(),
    }
}

fn lower(name: Name, members: Vec<(Name, Value)>, exported: bool, text: bool) -> Lowered {
    let loc = name.loc;
    let ty = if text {
        let literals: Vec<TypeRef> = members
            .iter()
            .map(|(_, value)| match value {
                Value::Text(text) => detailed(loc, TypeDetail::StringLit(text.clone())),
                Value::Number(_) => unreachable!("string enum"),
            })
            .collect();
        match <[TypeRef; 1]>::try_from(literals) {
            Ok([single]) => single,
            Err(literals) => detailed(loc, TypeDetail::Union(literals)),
        }
    } else {
        named(b"number", loc)
    };
    let members = members
        .into_iter()
        .map(|(member, value)| EnumMember {
            name: member,
            value: match value {
                Value::Text(text) => EnumValue::Text(text),
                Value::Number(number) => EnumValue::Number(format!("{number:?}")),
            },
        })
        .collect();
    Lowered {
        ty: TypeDecl {
            name: name.clone(),
            ty,
            exported,
            type_params: Vec::new(),
        },
        decl: EnumDecl {
            name,
            members,
            exported,
        },
    }
}
