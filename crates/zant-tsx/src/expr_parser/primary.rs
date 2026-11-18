//! Primary expressions: literals, regular expressions, arrays, objects and template strings.

use super::*;

/// A regular expression literal `/body/flags` where an expression starts
/// (TypeScript's scanner rescans the `/` token the same way). It is
/// `new RegExp("body", "flags")`: a literal makes a new object each time it
/// is evaluated, and the body keeps its escapes as written.
pub(super) fn parse_regex(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
    let bytes = p.src.as_bytes();
    let start = p.cur.start as usize + 1;
    let mut index = start;
    let mut in_class = false;
    loop {
        match bytes.get(index) {
            None | Some(b'\n') | Some(b'\r') => {
                return Err(p.fail(loc, "unterminated regular expression literal"));
            }
            Some(b'\\') => index += 2,
            Some(b'[') => {
                in_class = true;
                index += 1;
            }
            Some(b']') => {
                in_class = false;
                index += 1;
            }
            Some(b'/') if !in_class => break,
            Some(_) => index += 1,
        }
    }
    let pattern = bytes[start..index].to_vec();
    let mut end = index + 1;
    while bytes
        .get(end)
        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
    {
        end += 1;
    }
    let flags = bytes[index + 1..end].to_vec();
    p.lx.pos = end as u32;
    p.advance();
    let string = |value: Vec<u8>| {
        Box::new(Expr {
            loc,
            data: ExprData::String(value),
        })
    };
    Ok(p.expr(
        loc,
        ExprData::Call(crate::ast::Call {
            callee: p.expr(loc, ExprData::Ident(b"RegExp".to_vec())),
            type_args: Vec::new(),
            args: vec![string(pattern), string(flags)],
            optional: false,
            is_new: true,
        }),
    ))
}

pub fn parse_primary(p: &mut Parser) -> Result<Box<Expr>, Error> {
    let loc = p.cur.start;
    match p.cur.kind {
        Kind::Number => {
            let raw = p.text().as_bytes().to_vec();
            p.advance();
            Ok(p.expr(loc, ExprData::Number(raw)))
        }
        Kind::String => {
            let value = match lexer::decode_string(p.text().as_bytes()) {
                Ok(value) => value,
                Err(_) => return Err(p.fail(loc, "invalid escape in string")),
            };
            p.advance();
            Ok(p.expr(loc, ExprData::String(value)))
        }
        Kind::Template => parse_template(p, loc),
        Kind::Identifier => {
            let name = p.text();
            if name == "true" || name == "false" {
                p.advance();
                return Ok(p.expr(loc, ExprData::Boolean(name.as_bytes()[0] == b't')));
            }
            if name == "null" || name == "undefined" {
                p.advance();
                return Ok(p.expr(loc, ExprData::NullLit));
            }
            if name == "function" {
                return Err(p.fail(
                    loc,
                    "use an arrow function (`() => ...`) inside expressions",
                ));
            }
            if name == "this" {
                return Err(p.fail(loc, "`this` is not supported; components are functions"));
            }
            let name = name.as_bytes().to_vec();
            p.advance();
            Ok(p.expr(loc, ExprData::Ident(name)))
        }
        Kind::Punct if p.is_p("/") || p.is_p("/=") => parse_regex(p, loc),
        Kind::Punct => {
            if p.is_p("(") {
                p.advance();
                let inner = parse_expr(p)?;
                p.expect_p(")")?;
                return Ok(inner);
            }
            if p.is_p("<") {
                let el = jsx_parser::parse_jsx_after_lt(p, loc)?;
                p.advance();
                return Ok(p.expr(loc, ExprData::Jsx(el)));
            }
            if p.is_p("{") {
                return parse_object(p, loc);
            }
            if p.is_p("[") {
                return parse_array(p, loc);
            }
            let text = p.text();
            Err(p.fail(loc, format!("unexpected `{text}`")))
        }
        Kind::Eof => Err(p.fail(loc, "expected an expression but the file ended")),
        Kind::Invalid => {
            let text = p.text();
            Err(p.fail(loc, format!("unexpected character `{text}`")))
        }
    }
}

/// `[a, b, ...rest]` (trailing comma allowed, holes are not).
pub(super) fn parse_array(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
    p.advance(); // [
    let mut items: Vec<Box<Expr>> = Vec::new();
    while !p.is_p("]") {
        if p.is_p(",") {
            return Err(p.fail(p.cur.start, "array holes are not supported"));
        }
        items.push(spread_or_expr(p)?);
        if !p.eat_p(",") {
            break;
        }
    }
    p.expect_p("]")?;
    Ok(p.expr(loc, ExprData::Array(items)))
}

/// `...x` or an expression (array literal items).
pub(super) fn spread_or_expr(p: &mut Parser) -> Result<Box<Expr>, Error> {
    if !p.is_p("...") {
        return parse_assign(p);
    }
    let loc = p.cur.start;
    p.advance();
    let inner = parse_assign(p)?;
    Ok(p.expr(loc, ExprData::Spread(inner)))
}

pub fn parse_object(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
    p.advance(); // {
    let mut props: Vec<Property> = Vec::new();
    while !p.is_p("}") {
        if p.is_p("...") {
            let loc_spread = p.cur.start;
            p.advance();
            let inner = parse_assign(p)?;
            props.push(Property {
                key: Name {
                    name: Vec::new(),
                    loc: loc_spread,
                },
                value: inner,
                spread: true,
            });
            if !p.eat_p(",") {
                break;
            }
            continue;
        }
        if p.is_p("[") {
            return Err(p.fail(p.cur.start, "computed keys are not supported"));
        }
        let key: Name = match p.cur.kind {
            Kind::Identifier => Name {
                name: p.text().as_bytes().to_vec(),
                loc: p.cur.start,
            },
            Kind::String => {
                let value = match lexer::decode_string(p.text().as_bytes()) {
                    Ok(value) => value,
                    Err(_) => return Err(p.fail(p.cur.start, "invalid escape in string")),
                };
                Name {
                    name: value,
                    loc: p.cur.start,
                }
            }
            _ => return Err(p.unexpected("a property name")),
        };
        p.advance();
        let value = if p.eat_p(":") {
            parse_assign(p)?
        } else if p.is_p(",") || p.is_p("}") {
            // shorthand `{ gap }`
            let name = key.name.clone();
            p.expr(key.loc, ExprData::Ident(name))
        } else {
            return Err(p.unexpected(":"));
        };
        props.push(Property {
            key,
            value,
            spread: false,
        });
        if !p.eat_p(",") {
            break;
        }
    }
    p.expect_p("}")?;
    Ok(p.expr(loc, ExprData::Object(props)))
}

pub fn parse_template(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
    let mut quasis: Vec<Vec<u8>> = Vec::new();
    let mut exprs: Vec<Box<Expr>> = Vec::new();
    loop {
        let part = p.lx.template_part();
        let cooked = match lexer::cook(part.raw) {
            Ok(cooked) => cooked,
            Err(_) => {
                return Err(p.fail(part.start, "invalid escape in template literal"));
            }
        };
        quasis.push(cooked);
        match part.end {
            TemplateEnd::Eof => return Err(p.fail(loc, "unterminated template literal")),
            TemplateEnd::Backtick => break,
            TemplateEnd::Substitution => {
                p.advance();
                exprs.push(parse_expr(p)?);
                if !p.is_p("}") {
                    return Err(p.unexpected("}"));
                }
                // the lexer sits right after `}`: continue the template there
            }
        }
    }
    p.advance();
    Ok(p.expr(loc, ExprData::Template(Template { quasis, exprs })))
}
