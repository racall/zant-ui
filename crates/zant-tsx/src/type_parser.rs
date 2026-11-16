//! Type annotations of the TSX subset (`docs/tsx.md`): named types (dotted
//! too), `T[]`, `Array<T>`, unions, string literal types, `null` /
//! `undefined`, object types, intersections and function types. `interface`
//! bodies are object types.
//!
//! Like tsc, a `(` starts a function type when the matching `)` is followed
//! by `=>`, and a parenthesized type otherwise.

use crate::ast::{FuncType, Name, TypeDetail, TypeField, TypeRef};
use crate::lexer::{self, Kind};
use crate::parser::{Parser, bytes_lossy};

pub type Error = crate::error::ParseError;

/// `A | B | C` (a leading `|` is allowed, as in TypeScript).
pub fn parse_type(p: &mut Parser) -> Result<TypeRef, Error> {
    let loc = p.cur.start;
    p.eat_p("|");
    let first = parse_intersection(p)?;
    if !p.is_p("|") {
        return Ok(first);
    }
    let mut members: Vec<TypeRef> = vec![first];
    while p.eat_p("|") {
        members.push(parse_intersection(p)?);
    }
    Ok(detailed(loc, TypeDetail::Union(members)))
}

/// `A & B`: binds tighter than `|`, as in TypeScript (a leading `&` is
/// allowed too).
fn parse_intersection(p: &mut Parser) -> Result<TypeRef, Error> {
    let loc = p.cur.start;
    p.eat_p("&");
    let first = parse_postfix(p)?;
    if !p.is_p("&") {
        return Ok(first);
    }
    let mut members: Vec<TypeRef> = vec![first];
    while p.eat_p("&") {
        members.push(parse_postfix(p)?);
    }
    Ok(detailed(loc, TypeDetail::Intersection(members)))
}

/// A primary type followed by any number of `[]`.
fn parse_postfix(p: &mut Parser) -> Result<TypeRef, Error> {
    let loc = p.cur.start;
    let readonly = p.is_kw("readonly");
    if readonly {
        p.advance();
    }
    let mut t = parse_primary(p)?;
    let mut array = false;
    while p.is_p("[") {
        p.advance();
        if !p.is_p("]") {
            return Err(p.fail(
                p.cur.start,
                "indexed access types are not supported; write `T[]` for an array",
            ));
        }
        p.advance();
        t = detailed(loc, TypeDetail::Array(t));
        array = true;
    }
    // TypeScript permits this modifier only on array syntax, not named types.
    if readonly && !array {
        return Err(p.fail(loc, "`readonly` requires an array type written as `T[]`"));
    }
    Ok(t)
}

fn parse_primary(p: &mut Parser) -> Result<TypeRef, Error> {
    let loc = p.cur.start;
    match p.cur.kind {
        Kind::String => {
            let value = match lexer::decode_string(p.text().as_bytes()) {
                Ok(value) => value,
                Err(_) => return Err(p.fail(loc, "invalid escape in string")),
            };
            p.advance();
            return Ok(detailed(loc, TypeDetail::StringLit(value)));
        }
        Kind::Number => {
            return Err(p.fail(
                loc,
                "number literal types are not supported; use a string literal union",
            ));
        }
        Kind::Identifier => {}
        _ => {
            if p.is_p("{") {
                return Ok(detailed(loc, TypeDetail::Object(parse_members(p)?)));
            }
            if p.is_p("[") {
                return parse_tuple(p, loc);
            }
            if p.is_p("<") {
                let type_params = crate::type_params::parse_params(p)?;
                return parse_function(p, loc, type_params);
            }
            if p.is_p("(") {
                return parse_paren_or_function(p);
            }
            return Err(p.unexpected("a type"));
        }
    }
    let word = p.text();
    if word == "null" || word == "undefined" {
        p.advance();
        return Ok(detailed(loc, TypeDetail::Null));
    }
    for kw in ["typeof", "keyof", "infer", "unique", "readonly"] {
        if word == kw {
            return Err(p.fail(loc, format!("`{kw}` types are not supported")));
        }
    }
    // a name, dotted (`JSX.Element`)
    let mut end = p.cur.end;
    p.advance();
    while p.is_p(".") {
        p.advance();
        if p.cur.kind != Kind::Identifier {
            return Err(p.unexpected("a type name"));
        }
        end = p.cur.end;
        p.advance();
    }
    let name = p.src_slice(loc, end);
    if name.ends_with(".FC") {
        return Err(p.fail(loc, "write a function component: function X(props: P)"));
    }
    let mut args = if p.is_p("<") {
        crate::type_params::parse_args(p)?
    } else {
        Vec::new()
    };
    if (name == "Array" || name == "ReadonlyArray") && !args.is_empty() {
        if args.len() != 1 {
            return Err(p.fail(loc, "`Array` takes one type argument"));
        }
        return Ok(detailed(loc, TypeDetail::Array(args.remove(0))));
    }
    Ok(TypeRef {
        name: name.as_bytes().to_vec(),
        loc,
        detail: None,
        args,
    })
}

/// `{ id: number; title?: string }`: members end with `;`, `,` or a line
/// break.
pub fn parse_members(p: &mut Parser) -> Result<Vec<TypeField>, Error> {
    p.expect_p("{")?;
    let mut fields: Vec<TypeField> = Vec::new();
    while !p.is_p("}") {
        if p.cur.kind == Kind::Eof {
            return Err(p.unexpected("}"));
        }
        if p.is_p("[") {
            return Err(p.fail(
                p.cur.start,
                "index signatures are not supported; use an array of objects",
            ));
        }
        if p.is_kw("readonly") {
            let s = p.save();
            p.advance();
            if p.cur.kind != Kind::Identifier {
                p.restore(s);
            }
        }
        let name: Name = match p.cur.kind {
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
            _ => return Err(p.unexpected("a field name")),
        };
        p.advance();
        for f in &fields {
            if f.name.name == name.name {
                let msg = format!("duplicate field `{}`", bytes_lossy(&name.name));
                return Err(p.fail(name.loc, msg));
            }
        }
        let optional = p.eat_p("?");
        if p.is_p("(") {
            let msg = format!(
                "methods are not supported in object types; write `{}: (...) => T`",
                bytes_lossy(&name.name)
            );
            return Err(p.fail(p.cur.start, msg));
        }
        p.expect_p(":")?;
        let ty = parse_type(p)?;
        fields.push(TypeField { name, ty, optional });
        if p.eat_p(";") || p.eat_p(",") {
            continue;
        }
        if p.is_p("}") || p.cur.nl_before {
            continue;
        }
        return Err(p.unexpected(";"));
    }
    p.advance();
    Ok(fields)
}

/// `(A | B)` or `(id: number) => void`.
fn parse_paren_or_function(p: &mut Parser) -> Result<TypeRef, Error> {
    let loc = p.cur.start;
    if !followed_by_arrow(p) {
        p.advance();
        let inner = parse_type(p)?;
        p.expect_p(")")?;
        return Ok(inner);
    }
    parse_function(p, loc, Vec::new())
}

fn parse_function(
    p: &mut Parser,
    loc: u32,
    type_params: Vec<crate::ast::TypeParam>,
) -> Result<TypeRef, Error> {
    let params = p.parse_params()?;
    p.expect_p("=>")?;
    let ret = parse_type(p)?;
    Ok(detailed(
        loc,
        TypeDetail::Func(FuncType {
            type_params,
            params,
            ret,
        }),
    ))
}

/// Whether the `(` at `cur` is matched by a `)` followed by `=>` (nothing
/// consumed).
pub fn followed_by_arrow(p: &mut Parser) -> bool {
    let s = p.save();
    let mut result = false;
    let mut depth: usize = 0;
    while p.cur.kind != Kind::Eof {
        if p.is_p("(") || p.is_p("[") || p.is_p("{") {
            depth += 1;
        } else if p.is_p(")") || p.is_p("]") || p.is_p("}") {
            depth -= 1;
            if depth == 0 {
                p.advance();
                result = p.is_p("=>");
                break;
            }
        } else if p.cur.kind == Kind::Template {
            break; // never inside parameter lists or types
        }
        p.advance();
    }
    p.restore(s);
    result
}

fn detailed(loc: u32, detail: TypeDetail) -> TypeRef {
    TypeRef {
        args: Vec::new(),
        name: Vec::new(),
        loc,
        detail: Some(Box::new(detail)),
    }
}

/// `[A, B]`: fixed elements (optional `?` and rest `...` elements are not
/// supported).
fn parse_tuple(p: &mut Parser, loc: u32) -> Result<TypeRef, Error> {
    p.advance(); // [
    let mut items = Vec::new();
    while !p.is_p("]") {
        if p.is_p("...") {
            return Err(p.fail(
                p.cur.start,
                "rest elements in tuple types are not supported; use an array `T[]`",
            ));
        }
        // a labeled element `[name: string, age: number]`
        let state = p.save();
        if p.cur.kind == Kind::Identifier {
            p.advance();
            if !p.eat_p(":") {
                p.restore(state);
            }
        }
        items.push(parse_type(p)?);
        if p.is_p("?") {
            return Err(p.fail(
                p.cur.start,
                "optional tuple elements are not supported; use `T | null`",
            ));
        }
        if !p.eat_p(",") {
            break;
        }
    }
    p.expect_p("]")?;
    if items.is_empty() {
        return Err(p.fail(loc, "an empty tuple type is not supported"));
    }
    Ok(detailed(loc, TypeDetail::Tuple(items)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_ok;

    fn text(value: &[u8]) -> &str {
        std::str::from_utf8(value).expect("text is UTF-8")
    }

    #[test]
    fn readonly_arrays_use_the_readonly_array_representation() {
        for (source, depth) in [
            ("readonly string[]", 1),
            ("ReadonlyArray<string>", 1),
            ("readonly string[][]", 2),
            ("ReadonlyArray<string[]>", 2),
        ] {
            let mut p = Parser::init(source);
            let ty = p.parse_type().expect("array type");
            assert_eq!(p.cur.kind, Kind::Eof);
            let mut item = &ty;
            for _ in 0..depth {
                item = match item.detail.as_deref() {
                    Some(TypeDetail::Array(inner)) => inner,
                    _ => panic!("expected an array in {source}"),
                };
            }
            assert_eq!("string", text(&item.name));
            assert!(item.detail.is_none());
        }
        let m = parse_ok(
            "type Rows = readonly string[] | null;\n\
             interface Props { rows: readonly (string | null)[] }\n\
             function rows(value: readonly string[][]): readonly string[][] { return value; }",
        );
        assert!(matches!(
            m.types[0].ty.detail.as_deref(),
            Some(TypeDetail::Union(members))
                if matches!(members[0].detail.as_deref(), Some(TypeDetail::Array(_)))
                    && matches!(members[1].detail.as_deref(), Some(TypeDetail::Null))
        ));
    }

    #[test]
    fn readonly_still_requires_array_syntax() {
        for ty in ["readonly string", "readonly Array<string>"] {
            crate::parser::expect_syntax_error(
                &format!("type Rows = {ty};"),
                "`readonly` requires an array type written as `T[]`",
                "readonly",
            );
        }
    }

    #[test]
    fn phase_2_types_interfaces_unions_arrays_function_types() {
        let m = parse_ok(
            r#"export interface Todo {
  id: number;
  title: string
  done?: boolean,
  tags: string[]
}
type Filter = "all" | "active" | "done";
type Rows = Array<Array<Todo>>;
interface RowProps { todo: Todo; onToggle: (id: number) => void; note: string | null }"#,
        );
        assert_eq!(m.types.len(), 4);
        let todo = &m.types[0];
        assert!(todo.exported);
        let fields = match todo.ty.detail.as_deref() {
            Some(TypeDetail::Object(fields)) => fields,
            _ => panic!("expected an object type"),
        };
        assert_eq!(fields.len(), 4);
        assert!(fields[2].optional);
        match fields[3].ty.detail.as_deref() {
            Some(TypeDetail::Array(inner)) => assert_eq!("string", text(&inner.name)),
            _ => panic!("expected an array type"),
        }
        let filter = match m.types[1].ty.detail.as_deref() {
            Some(TypeDetail::Union(members)) => members,
            _ => panic!("expected a union type"),
        };
        assert_eq!(filter.len(), 3);
        match filter[1].detail.as_deref() {
            Some(TypeDetail::StringLit(value)) => assert_eq!("active", text(value)),
            _ => panic!("expected a string literal type"),
        }
        // nested generics close with two separate `>` tokens
        let rows = match m.types[2].ty.detail.as_deref() {
            Some(TypeDetail::Array(outer)) => match outer.detail.as_deref() {
                Some(TypeDetail::Array(inner)) => inner,
                _ => panic!("expected a nested array type"),
            },
            _ => panic!("expected an array type"),
        };
        assert_eq!("Todo", text(&rows.name));
        let row = match m.types[3].ty.detail.as_deref() {
            Some(TypeDetail::Object(fields)) => fields,
            _ => panic!("expected an object type"),
        };
        let on_toggle = match row[1].ty.detail.as_deref() {
            Some(TypeDetail::Func(f)) => f,
            _ => panic!("expected a function type"),
        };
        assert_eq!("id", text(&on_toggle.params[0].name.name));
        assert_eq!("void", text(&on_toggle.ret.name));
        let note = match row[2].ty.detail.as_deref() {
            Some(TypeDetail::Union(members)) => members,
            _ => panic!("expected a union type"),
        };
        assert!(matches!(note[1].detail.as_deref(), Some(TypeDetail::Null)));
    }
}
