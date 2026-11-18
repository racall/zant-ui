//! Named symbols, operators, accents, arrows, and explicit character and atom classes.

use super::tokens::strip_braces;
use super::{
    AccentNode, Atom, Class, Node, Op, OverUnder, OverUnderKind, ParseError, Parser, XArrow,
    XArrowKind,
};
use crate::math::{fonts, symbols};

impl<'a> Parser<'a> {
    pub(super) fn symbol_or_operator(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if let Some(sym) = symbols::lookup(name) {
            // .normal italicises only lowercase Greek; other symbols are unchanged
            return Ok(Some(Node::Atom(Atom {
                text: sym.text.to_string(),
                class: sym.class,
                variant: fonts::Variant::Normal,
            })));
        }
        if let Some(op) = symbols::big_op(name) {
            return Ok(Some(Node::Op(Op {
                text: op.text.to_string(),
                symbol: true,
                integral: op.integral,
                limits: op.limits,
                forced: None,
            })));
        }
        if let Some(f) = symbols::function(name) {
            return Ok(Some(Node::Op(Op {
                text: f.text.to_string(),
                symbol: false,
                integral: false,
                limits: f.limits,
                forced: None,
            })));
        }
        if name == "operatorname" || name == "operatornamewithlimits" {
            let mut limits = name == "operatornamewithlimits";
            if self.peek_byte() == b'*' {
                self.i += 1;
                limits = true;
            }
            let raw = self.raw_arg()?;
            return Ok(Some(Node::Op(Op {
                text: strip_braces(raw),
                symbol: false,
                integral: false,
                limits,
                forced: None,
            })));
        }
        if name == "bmod" {
            return Ok(Some(Node::Atom(Atom {
                text: "mod".to_string(),
                class: Class::Bin,
                variant: fonts::Variant::Roman,
            })));
        }
        if name == "pmod" || name == "mod" || name == "pod" {
            let arg = self.parse_arg()?;
            let mut parts: Vec<Node> = Vec::new();
            parts.push(Node::Space(if name == "mod" { 0.75 } else { 1.0 }));
            if name != "mod" {
                parts.push(Node::Atom(Atom {
                    text: "(".to_string(),
                    class: Class::Open,
                    variant: fonts::Variant::Normal,
                }));
            }
            if name != "pod" {
                parts.push(Node::Atom(Atom {
                    text: "mod".to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Roman,
                }));
                parts.push(Node::Space(1.0 / 3.0));
            }
            parts.push(Node::Group(arg));
            if name != "mod" {
                parts.push(Node::Atom(Atom {
                    text: ")".to_string(),
                    class: Class::Close,
                    variant: fonts::Variant::Normal,
                }));
            }
            return Ok(Some(Node::Group(parts)));
        }
        Ok(None)
    }

    pub(super) fn accent_or_arrow(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if let Some(acc) = symbols::accent(name) {
            let base = self.parse_arg()?;
            return Ok(Some(Node::Accent(AccentNode {
                base,
                glyph: acc.glyph.to_string(),
                wide: acc.wide,
                name: acc.name.to_string(),
            })));
        }
        for (n, kind) in [
            ("overline", OverUnderKind::Overline),
            ("underline", OverUnderKind::Underline),
            ("overbrace", OverUnderKind::Overbrace),
            ("underbrace", OverUnderKind::Underbrace),
            ("overrightarrow", OverUnderKind::Overrightarrow),
            ("overleftarrow", OverUnderKind::Overleftarrow),
            ("overleftrightarrow", OverUnderKind::Overleftrightarrow),
            ("underrightarrow", OverUnderKind::Underrightarrow),
            ("underleftarrow", OverUnderKind::Underleftarrow),
        ] {
            if name == n {
                return Ok(Some(Node::OverUnder(OverUnder {
                    body: self.parse_arg()?,
                    kind,
                })));
            }
        }
        for (n, kind) in [
            ("xrightarrow", XArrowKind::Right),
            ("xleftarrow", XArrowKind::Left),
            ("xleftrightarrow", XArrowKind::LeftRight),
            ("xRightarrow", XArrowKind::DoubleRight),
            ("xLeftarrow", XArrowKind::DoubleLeft),
            ("xLeftrightarrow", XArrowKind::DoubleLeftRight),
            ("xhookrightarrow", XArrowKind::HookRight),
            ("xhookleftarrow", XArrowKind::HookLeft),
            ("xmapsto", XArrowKind::MapsTo),
            ("xlongequal", XArrowKind::LongEqual),
            ("xrightleftharpoons", XArrowKind::HarpoonsRl),
            ("xleftrightharpoons", XArrowKind::HarpoonsLr),
            ("xrightleftarrows", XArrowKind::ArrowsRl),
            ("xtofrom", XArrowKind::ArrowsRl),
            ("xrightequilibrium", XArrowKind::HarpoonsRl),
            ("xleftequilibrium", XArrowKind::HarpoonsLr),
            ("xtwoheadrightarrow", XArrowKind::TwoHeadRight),
            ("xtwoheadleftarrow", XArrowKind::TwoHeadLeft),
        ] {
            if name == n {
                self.skip_space();
                let below: Option<Vec<Node>> = if self.peek_byte() == b'[' {
                    self.optional_arg()?
                } else {
                    None
                };
                return Ok(Some(Node::XArrow(XArrow {
                    kind,
                    above: self.parse_arg()?,
                    below,
                })));
            }
        }
        Ok(None)
    }

    pub(super) fn unicode_command(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if name == "unicode" || name == "char" {
            let raw: &str = if name == "unicode" {
                self.raw_arg()?
            } else {
                self.skip_space();
                let start = self.i;
                while self.i < self.src.len() {
                    let b = self.src.as_bytes()[self.i];
                    if b.is_ascii_alphanumeric() || b == b'"' || b == b'\'' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                self.slice(start, self.i)
            };
            let Some(cp) = parse_char_code(raw) else {
                return Ok(Some(Node::Unknown(name.to_string())));
            };
            return Ok(Some(Node::Atom(Atom {
                text: cp.to_string(),
                class: Class::Ord,
                variant: fonts::Variant::Normal,
            })));
        }
        Ok(None)
    }

    pub(super) fn class_command(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if name == "mathop" {
            return Ok(Some(Node::Op(Op {
                text: strip_braces(self.raw_arg()?),
                symbol: false,
                integral: false,
                limits: true,
                forced: None,
            })));
        }
        if matches!(
            name,
            "mathrel"
                | "mathbin"
                | "mathord"
                | "mathopen"
                | "mathclose"
                | "mathpunct"
                | "mathinner"
        ) {
            let body = self.parse_arg()?;
            let class: Class = if name == "mathrel" {
                Class::Rel
            } else if name == "mathbin" {
                Class::Bin
            } else if name == "mathopen" {
                Class::Open
            } else if name == "mathclose" {
                Class::Close
            } else if name == "mathpunct" {
                Class::Punct
            } else if name == "mathinner" {
                Class::Inner
            } else {
                Class::Ord
            };
            if body.len() == 1 {
                match body.into_iter().next().expect("one element") {
                    Node::Atom(mut atom) => {
                        atom.class = class;
                        return Ok(Some(Node::Atom(atom)));
                    }
                    other => return Ok(Some(Node::Group(vec![other]))),
                }
            }
            return Ok(Some(Node::Group(body)));
        }
        Ok(None)
    }
}

/// `\unicode{x41}`, `\unicode{65}`, `\char"41`, `\char'101`, `\char65`.
fn parse_char_code(raw: &str) -> Option<char> {
    let t = raw.trim_matches(|c| c == ' ' || c == '{' || c == '}');
    if t.is_empty() {
        return None;
    }
    let bytes = t.as_bytes();
    let v: u32 = if bytes[0] == b'x' || bytes[0] == b'"' {
        u32::from_str_radix(&t[1..], 16).ok()?
    } else if bytes[0] == b'\'' {
        u32::from_str_radix(&t[1..], 8).ok()?
    } else {
        t.parse::<u32>().ok()?
    };
    if v > 0x10FFFF || (0xD800..=0xDFFF).contains(&v) {
        return None;
    }
    char::from_u32(v)
}
