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

pub type Error = crate::error::ParseError;

pub fn parse_expr(p: &mut Parser) -> Result<Box<Expr>, Error> {
    parse_assign(p)
}

pub fn parse_assign(p: &mut Parser) -> Result<Box<Expr>, Error> {
    if let Some(arrow) = try_arrow(p)? {
        return Ok(arrow);
    }
    let lhs = parse_cond(p)?;
    for op in ["=", "+=", "-=", "*=", "/=", "%="] {
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

/// `x => ...`, `() => ...`, `(a: T, b) => ...`, `(t: Todo): string => ...`;
/// null when the tokens are not an arrow function (nothing consumed).
/// Arrows are recognised before parsing them (tsc's approach), so errors
/// inside their parameters are reported as such.
pub fn try_arrow(p: &mut Parser) -> Result<Option<Box<Expr>>, Error> {
    let loc = p.cur.start;
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
        let func = arrow_body(p, loc, params, None)?;
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
    let mut func = arrow_body(p, loc, params, ret)?;
    func.type_params = type_params;
    Ok(Some(p.expr(loc, ExprData::Arrow(func))))
}

/// `(...): Type =>`: the matching `)` is followed by a return type and `=>`
/// (tried speculatively; nothing consumed, no error kept).
fn return_typed_arrow(p: &mut Parser) -> bool {
    let s = p.save();
    let saved_diag = p.diag.clone();
    let result = return_typed_arrow_inner(p);
    p.restore(s);
    p.diag = saved_diag;
    result
}

fn return_typed_arrow_inner(p: &mut Parser) -> bool {
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
fn definitely_arrow(p: &mut Parser) -> bool {
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
) -> Result<Box<Function>, Error> {
    p.advance(); // =>
    let body = if p.is_p("{") {
        crate::ast::Body::Block(p.parse_block()?)
    } else {
        crate::ast::Body::Expr(parse_assign(p)?)
    };
    Ok(Box::new(Function {
        type_params: Vec::new(),
        loc,
        name: Vec::new(),
        params,
        ret,
        body,
    }))
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
    let table: [(&str, u8); 20] = [
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
        ("+", 9),
        ("-", 9),
        ("*", 10),
        ("/", 10),
        ("%", 10),
        ("**", 11),
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
    for op in ["!", "-", "+"] {
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
    for kw in ["void", "await"] {
        if p.is_kw(kw) {
            return Err(p.fail(loc, format!("`{kw}` is not supported")));
        }
    }
    parse_postfix(p)
}

/// `new Map<K, V>(args)` / `new Set()` / `new Map`: a constructor call.
fn parse_new(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
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

pub fn parse_postfix(p: &mut Parser) -> Result<Box<Expr>, Error> {
    let e = parse_primary(p)?;
    parse_postfix_on(p, e)
}

/// Members, calls, indexes, `!`, `++` and `as` after `e`.
fn parse_postfix_on(p: &mut Parser, mut e: Box<Expr>) -> Result<Box<Expr>, Error> {
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
fn parse_index(p: &mut Parser, object: Box<Expr>, optional: bool) -> Result<Box<Expr>, Error> {
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

/// A regular expression literal `/body/flags` where an expression starts
/// (TypeScript's scanner rescans the `/` token the same way). It is
/// `new RegExp("body", "flags")`: a literal makes a new object each time it
/// is evaluated, and the body keeps its escapes as written.
fn parse_regex(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
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
fn parse_array(p: &mut Parser, loc: u32) -> Result<Box<Expr>, Error> {
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
fn spread_or_expr(p: &mut Parser) -> Result<Box<Expr>, Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Body, Child, Expr, Function, Stmt, StmtData, TypeDetail};
    use crate::parser::{expect_syntax_error, parse_ok};

    fn block(f: &Function) -> &[Box<Stmt>] {
        match &f.body {
            Body::Block(stmts) => stmts,
            Body::Expr(_) => panic!("expected a block body"),
        }
    }

    fn ret_expr(s: &Stmt) -> &Expr {
        match &s.data {
            StmtData::Ret(Some(e)) => e,
            _ => panic!("expected a return with a value"),
        }
    }

    #[test]
    fn regex_literals_become_regexp_constructions_and_division_stays() {
        let module = parse_ok(
            r#"function f(s: string, a: number, b: number): number {
  const re = /a[/\]]b\/c/gi;
  const total = a / b / 2;
  return re.test(s) ? total : 0;
}"#,
        );
        let ss = block(&module.functions[0].func);
        let StmtData::Decl(decl) = &ss[0].data else {
            panic!("a declaration")
        };
        let ExprData::Call(call) = &decl.init.as_ref().unwrap().data else {
            panic!("a call")
        };
        assert!(call.is_new);
        assert!(matches!(&call.callee.data, ExprData::Ident(n) if n == b"RegExp"));
        assert!(matches!(&call.args[0].data, ExprData::String(v) if v == br"a[/\]]b\/c"));
        assert!(matches!(&call.args[1].data, ExprData::String(v) if v == b"gi"));
        let StmtData::Decl(decl) = &ss[1].data else {
            panic!("a declaration")
        };
        assert!(matches!(&decl.init.as_ref().unwrap().data, ExprData::Binary(b) if b.op == b"/"));
        expect_syntax_error(
            "function f(): number { const re = /abc; return 1; }",
            "unterminated regular expression literal",
            "/abc",
        );
    }

    fn expect_codegen_error(source: &str, message: &str, at: &str) {
        parse_ok(source);
        let diag = crate::compile::compile(source, "test.tsx").expect_err("codegen error");
        assert_eq!(diag.msg, message);
        assert_eq!(
            diag.loc as usize,
            source.find(at).expect("diagnostic location")
        );
    }

    fn text(value: &[u8]) -> &str {
        std::str::from_utf8(value).expect("text is UTF-8")
    }

    #[test]
    fn const_assertions_preserve_literal_expressions() {
        for literal in [
            "42",
            "-42",
            "true",
            "false",
            r#""ready""#,
            "`ready`",
            "[]",
            r#"[1, "two", { done: true }]"#,
            r#"({ kind: "ready", values: [1, 2] })"#,
            r#"{ kind: "ready" as const, values: [1, 2] as const }"#,
        ] {
            let plain = parse_ok(&format!("const value = {literal};"));
            let asserted = parse_ok(&format!("const value = {literal} as const;"));
            assert_eq!(plain.consts[0].value, asserted.consts[0].value, "{literal}");
        }
        let m = parse_ok("const value = ([1, 2] as const)[0];");
        assert!(matches!(&m.consts[0].value.data, ExprData::Index(_)));
    }

    #[test]
    fn const_assertions_do_not_relax_other_assertion_targets() {
        expect_codegen_error(
            "const value = 1 as Promise<number>;\nexport default function App() { return null; }",
            "generic type `Promise<…>` is not supported yet; declare it in this file (`interface X<T>`) or use a concrete type",
            "Promise",
        );
        // `typeof` names a top-level constant of the file
        expect_codegen_error(
            "const value = 1 as typeof other;\nexport default function App() { return null; }",
            "`typeof other`: only top-level constants of this file can be named here",
            "other",
        );
    }

    #[test]
    fn comparison_is_not_a_type_argument_list() {
        let m = parse_ok("function f() { return a < b; }");
        let e = ret_expr(&block(&m.functions[0].func)[0]);
        match &e.data {
            ExprData::Binary(b) => assert_eq!("<", text(&b.op)),
            _ => panic!("expected a binary expression"),
        }
    }

    #[test]
    fn typed_arrows_report_errors_inside_their_parameters() {
        expect_codegen_error(
            "function f() { return (v: Promise<number>) => v; }",
            "generic type `Promise<…>` is not supported yet; declare it in this file (`interface X<T>`) or use a concrete type",
            "Promise",
        );
        expect_syntax_error(
            "function f() { return (): number => ; }",
            "unexpected `;`",
            ";",
        );
    }

    #[test]
    fn phase_2_expressions_literals_with_spread_indexing_optional_chain_non_null_as() {
        let m = parse_ok(
            r#"const empty: Todo[] = [];
function f() {
  const [todos, setTodos] = useState<Todo[]>([])
  const next = [...todos, { id: 1, ...base, done: false }]
  const first = todos[0]?.title ?? "none"
  const found = todos.find((t: Todo): boolean => t.done)!
  const n = count as u32
  return todos.map((t, i) => <Text key={t.id}>{i}</Text>)
}"#,
        );
        assert_eq!(m.consts.len(), 1);
        match &m.consts[0].value.data {
            ExprData::Array(items) => assert_eq!(items.len(), 0),
            _ => panic!("expected an array literal"),
        }
        let body = block(&m.functions[0].func);
        let init = match &body[0].data {
            StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
            _ => panic!("expected a declaration"),
        };
        let call = match &init.data {
            ExprData::Call(c) => c,
            _ => panic!("expected a call"),
        };
        match call.type_args[0].detail.as_deref() {
            Some(TypeDetail::Array(inner)) => assert_eq!("Todo", text(&inner.name)),
            _ => panic!("expected an array type"),
        }
        let next = match &body[1].data {
            StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
            _ => panic!("expected a declaration"),
        };
        let items = match &next.data {
            ExprData::Array(items) => items,
            _ => panic!("expected an array literal"),
        };
        assert!(matches!(&items[0].data, ExprData::Spread(_)));
        let obj = match &items[1].data {
            ExprData::Object(props) => props,
            _ => panic!("expected an object literal"),
        };
        assert!(obj[1].spread && !obj[0].spread);
        let coalesce = match &body[2].data {
            StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
            _ => panic!("expected a declaration"),
        };
        let binary = match &coalesce.data {
            ExprData::Binary(b) => b,
            _ => panic!("expected a binary expression"),
        };
        assert_eq!("??", text(&binary.op));
        let member = match &binary.lhs.data {
            ExprData::Member(m) => m,
            _ => panic!("expected a member expression"),
        };
        assert!(member.optional);
        assert!(matches!(&member.object.data, ExprData::Index(_)));
        assert!(matches!(
            &body[3].data,
            StmtData::Decl(d) if matches!(&d.init.as_ref().unwrap().data, ExprData::NonNull(_))
        ));
        let as_expr = match &body[4].data {
            StmtData::Decl(d) => d.init.as_ref().expect("initializer"),
            _ => panic!("expected a declaration"),
        };
        match &as_expr.data {
            ExprData::As(a) => assert_eq!("u32", text(&a.ty.name)),
            _ => panic!("expected an `as` expression"),
        }
        let map_call = match &ret_expr(&body[5]).data {
            ExprData::Call(c) => c,
            _ => panic!("expected a call"),
        };
        let arrow = match &map_call.args[0].data {
            ExprData::Arrow(f) => f,
            _ => panic!("expected an arrow function"),
        };
        assert_eq!(arrow.params.len(), 2);
    }

    #[test]
    fn a_parenthesized_expression_is_not_taken_for_an_arrow() {
        let m = parse_ok("function f() { return x ? (a) : b; }");
        let e = ret_expr(&block(&m.functions[0].func)[0]);
        let c = match &e.data {
            ExprData::Cond(c) => c,
            _ => panic!("expected a conditional"),
        };
        assert!(matches!(&c.then.data, ExprData::Ident(_)));
    }

    #[test]
    fn greater_than_joins_into_greater_or_equal_only_as_an_operator() {
        let m = parse_ok("function f() { return a >= 10 && b > 2; }");
        let e = ret_expr(&block(&m.functions[0].func)[0]);
        let binary = match &e.data {
            ExprData::Binary(b) => b,
            _ => panic!("expected a binary expression"),
        };
        assert_eq!("&&", text(&binary.op));
        let lhs = match &binary.lhs.data {
            ExprData::Binary(b) => b,
            _ => panic!("expected a binary expression"),
        };
        assert_eq!(">=", text(&lhs.op));
        let rhs = match &binary.rhs.data {
            ExprData::Binary(b) => b,
            _ => panic!("expected a binary expression"),
        };
        assert_eq!(">", text(&rhs.op));
        // `>` closing a tag followed by `=` text stays apart
        let t = parse_ok("function f() { return <Text>=</Text>; }");
        let jsx = match &ret_expr(&block(&t.functions[0].func)[0]).data {
            ExprData::Jsx(j) => j,
            _ => panic!("expected JSX"),
        };
        match &jsx.children[0] {
            Child::Text(child) => assert_eq!("=", text(&child.value)),
            _ => panic!("expected text"),
        }
    }
}
