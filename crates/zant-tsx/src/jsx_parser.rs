//! JSX elements, attributes and children. Tags are read with the lexer's
//! JSX tag mode (so `>` never merges into `>=`), text with its raw mode, and
//! `{expressions}` with the normal expression parser.

use crate::ast::{Attr, Child, Jsx, JsxText, Name};
use crate::expr_parser;
use crate::jsx_text;
use crate::lexer::{Kind, Token};
use crate::parser::{Parser, bytes_lossy};

pub type Error = crate::error::ParseError;

/// Parses an element whose `<` was just read. Leaves the lexer right
/// after the element's final `>`; `cur` is stale.
pub fn parse_jsx_after_lt(p: &mut Parser, loc: u32) -> Result<Box<Jsx>, Error> {
    let mut t = p.lx.jsx_tag_token();
    let mut attrs: Vec<Attr> = Vec::new();
    if t.is(p.src, ">") {
        return parse_jsx_children(p, loc, Vec::new(), attrs); // fragment
    }
    if t.kind != Kind::Identifier {
        return Err(p.fail(t.start, "expected a tag name after `<`"));
    }
    let name = tag_name(p, t)?;
    loop {
        t = p.lx.jsx_tag_token();
        if t.kind == Kind::Identifier {
            let attr_name = Name {
                name: t.text(p.src).as_bytes().to_vec(),
                loc: t.start,
            };
            for a in &attrs {
                if a.name.name == attr_name.name {
                    let msg = format!("duplicate attribute `{}`", bytes_lossy(&attr_name.name));
                    return Err(p.fail(t.start, msg));
                }
            }
            let before = p.lx.pos;
            let eq = p.lx.jsx_tag_token();
            if !eq.is(p.src, "=") {
                p.lx.pos = before;
                attrs.push(Attr {
                    name: attr_name,
                    value: None,
                    spread: false,
                });
                continue;
            }
            let v = p.lx.jsx_tag_token();
            if v.kind == Kind::String {
                let raw = v.text(p.src);
                let value = jsx_text::decode_entities(&raw.as_bytes()[1..raw.len() - 1]);
                attrs.push(Attr {
                    name: attr_name,
                    value: Some(p.expr(v.start, crate::ast::ExprData::String(value))),
                    spread: false,
                });
            } else if v.is(p.src, "{") {
                p.advance();
                if p.is_p("}") {
                    let msg = format!(
                        "attribute `{}` has an empty expression",
                        bytes_lossy(&attr_name.name)
                    );
                    return Err(p.fail(p.cur.start, msg));
                }
                let value = expr_parser::parse_expr(p)?;
                if !p.is_p("}") {
                    return Err(p.unexpected("}"));
                }
                attrs.push(Attr {
                    name: attr_name,
                    value: Some(value),
                    spread: false,
                });
            } else if v.is(p.src, "<") {
                let msg = format!(
                    "wrap element attribute values in braces: `{}={{<... />}}`",
                    bytes_lossy(&attr_name.name)
                );
                return Err(p.fail(v.start, msg));
            } else {
                let msg = format!(
                    "expected a string or `{{...}}` for attribute `{}`",
                    bytes_lossy(&attr_name.name)
                );
                return Err(p.fail(v.start, msg));
            }
        } else if t.is(p.src, "{") {
            // `{...common}`
            p.advance();
            if !p.is_p("...") {
                return Err(p.fail(
                    p.cur.start,
                    "expected `...` in a spread attribute (`{...props}`)",
                ));
            }
            p.advance();
            let value = expr_parser::parse_expr(p)?;
            if !p.is_p("}") {
                return Err(p.unexpected("}"));
            }
            attrs.push(Attr {
                name: Name {
                    name: Vec::new(),
                    loc: t.start,
                },
                value: Some(value),
                spread: true,
            });
        } else if t.is(p.src, "/") {
            let gt = p.lx.jsx_tag_token();
            if !gt.is(p.src, ">") {
                let msg = format!("expected `>` after `/` in <{} />", bytes_lossy(&name));
                return Err(p.fail(gt.start, msg));
            }
            return Ok(Box::new(Jsx {
                loc,
                name,
                attrs,
                children: Vec::new(),
            }));
        } else if t.is(p.src, ">") {
            return parse_jsx_children(p, loc, name, attrs);
        } else if t.kind == Kind::Eof {
            let msg = format!("unterminated <{}> tag", bytes_lossy(&name));
            return Err(p.fail(loc, msg));
        } else {
            let msg = format!(
                "unexpected `{}` in <{}> tag",
                t.text(p.src),
                bytes_lossy(&name)
            );
            return Err(p.fail(t.start, msg));
        }
    }
}

pub fn parse_jsx_children(
    p: &mut Parser,
    loc: u32,
    name: Vec<u8>,
    attrs: Vec<Attr>,
) -> Result<Box<Jsx>, Error> {
    let mut children: Vec<Child> = Vec::new();
    loop {
        let raw = p.lx.jsx_text();
        let raw_text = raw.text(p.src);
        if let Some(i) = raw_text.bytes().position(|b| b == b'>' || b == b'}') {
            let c = raw_text.as_bytes()[i] as char;
            let entity = if c == '>' { "&gt;" } else { "&#125;" };
            return Err(p.fail(
                raw.start + i as u32,
                format!("unexpected `{c}` in JSX text; write `{{'{c}'}}` or `{entity}`"),
            ));
        }
        let decoded = jsx_text::decode_entities(raw_text.as_bytes());
        if let Some(value) = jsx_text::clean_jsx_text(&decoded) {
            // point errors at the text itself, not the indentation before it
            let lead = raw_text
                .bytes()
                .position(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
                .unwrap_or(0);
            children.push(Child::Text(JsxText {
                value,
                loc: raw.start + lead as u32,
            }));
        }
        if p.lx.pos as usize >= p.src.len() {
            return if name.is_empty() {
                Err(p.fail(loc, "unterminated fragment: missing `</>`"))
            } else {
                let msg = format!(
                    "unterminated <{}>: missing `</{}>`",
                    bytes_lossy(&name),
                    bytes_lossy(&name)
                );
                Err(p.fail(loc, msg))
            };
        }
        let child_loc = p.lx.pos;
        if p.src.as_bytes()[p.lx.pos as usize] == b'{' {
            p.lx.pos += 1;
            p.advance();
            if p.is_p("}") {
                children.push(Child::Expr(None));
                continue;
            }
            let e = expr_parser::parse_expr(p)?;
            if !p.is_p("}") {
                return Err(p.unexpected("}"));
            }
            children.push(Child::Expr(Some(e)));
            continue;
        }
        // `<`: a closing tag or a child element
        p.lx.pos += 1;
        let after_lt = p.lx.pos;
        let t = p.lx.jsx_tag_token();
        if t.is(p.src, "/") {
            let close = p.lx.jsx_tag_token();
            if name.is_empty() {
                if !close.is(p.src, ">") {
                    return Err(p.fail(close.start, "expected `</>` to close the fragment"));
                }
            } else {
                if close.kind != Kind::Identifier || tag_name(p, close)? != name {
                    let msg = format!(
                        "expected `</{}>` to close <{}>",
                        bytes_lossy(&name),
                        bytes_lossy(&name)
                    );
                    return Err(p.fail(close.start, msg));
                }
                let gt = p.lx.jsx_tag_token();
                if !gt.is(p.src, ">") {
                    let msg = format!("expected `>` after `</{}`", bytes_lossy(&name));
                    return Err(p.fail(gt.start, msg));
                }
            }
            return Ok(Box::new(Jsx {
                loc,
                name,
                attrs,
                children,
            }));
        }
        p.lx.pos = after_lt;
        let child = parse_jsx_after_lt(p, child_loc)?;
        children.push(Child::Element(child));
    }
}

/// Read dotted tag names without consuming the token following the name.
fn tag_name(p: &mut Parser, first: Token) -> Result<Vec<u8>, Error> {
    let mut name: Vec<u8> = first.text(p.src).as_bytes().to_vec();
    loop {
        let before = p.lx.pos;
        let dot = p.lx.jsx_tag_token();
        if !dot.is(p.src, ".") {
            p.lx.pos = before;
            return Ok(name);
        }
        let part = p.lx.jsx_tag_token();
        if part.kind != Kind::Identifier {
            return Err(p.fail(part.start, "expected a member tag name"));
        }
        name.push(b'.');
        name.extend_from_slice(part.text(p.src).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{Body, Child, ExprData, StmtData};
    use crate::parser::{expect_syntax_error, parse_ok};

    fn text(value: &[u8]) -> &str {
        std::str::from_utf8(value).expect("text is UTF-8")
    }

    #[test]
    fn text_errors_point_at_the_text_not_its_indentation() {
        let src = "function A() { return <View>\n    hello\n  </View>; }";
        let m = parse_ok(src);
        let body = match &m.functions[0].func.body {
            Body::Block(stmts) => stmts,
            _ => panic!("expected a block body"),
        };
        let jsx = match &body[0].data {
            StmtData::Ret(Some(e)) => match &e.data {
                ExprData::Jsx(j) => j,
                _ => panic!("expected JSX"),
            },
            _ => panic!("expected a return"),
        };
        let child = match &jsx.children[0] {
            Child::Text(t) => t,
            _ => panic!("expected text"),
        };
        assert_eq!("hello", &src[child.loc as usize..child.loc as usize + 5]);
    }

    #[test]
    fn gt_and_brace_in_jsx_text_name_the_fix() {
        expect_syntax_error(
            "function f() { return <Text>a > b</Text>; }",
            "unexpected `>` in JSX text; write `{'>'}` or `&gt;`",
            "> b",
        );
        expect_syntax_error(
            "function f() { return <Text>a } b</Text>; }",
            "unexpected `}` in JSX text; write `{'}'}` or `&#125;`",
            "} b",
        );
    }

    #[test]
    fn attribute_strings_use_the_same_entity_decoder_as_jsx_text() {
        let m = parse_ok(
            "function App() { return <Input placeholder=\"&copy; &#x1F600; &#X41; &unknown;\"/>; }",
        );
        let body = match &m.functions[0].func.body {
            Body::Block(stmts) => stmts,
            _ => panic!("expected a block body"),
        };
        let jsx = match &body[0].data {
            StmtData::Ret(Some(e)) => match &e.data {
                ExprData::Jsx(j) => j,
                _ => panic!("expected JSX"),
            },
            _ => panic!("expected a return"),
        };
        let value = jsx.attrs[0].value.as_ref().expect("attribute value");
        match &value.data {
            ExprData::String(value) => assert_eq!("© 😀 &#X41; &unknown;", text(value)),
            _ => panic!("expected a string"),
        }
    }
}
