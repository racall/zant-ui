//! Token reading, math lists, groups, arguments, and scripts.

use super::{Atom, Class, Node, ParseError, Parser, Stop, SupSub};
use crate::math::{fonts, symbols};

impl<'a> Parser<'a> {
    pub fn init(src: &'a str) -> Self {
        Self { src, i: 0 }
    }

    pub fn parse_all(&mut self) -> Result<Vec<Node>, ParseError> {
        let list = self.parse_list(Stop::None)?;
        self.skip_space();
        if self.i < self.src.len() {
            return Err(ParseError); // stray '}' etc.
        }
        Ok(list)
    }

    pub(super) fn slice(&self, start: usize, end: usize) -> &'a str {
        let src = self.src;
        let end = end.min(src.len());
        &src[start..end]
    }

    pub(super) fn peek_byte(&self) -> u8 {
        self.src.as_bytes().get(self.i).copied().unwrap_or(0)
    }

    pub(super) fn next_char_len(&self) -> usize {
        // `self.i` is always on a UTF-8 boundary here.
        self.src
            .get(self.i..)
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8)
    }

    pub(super) fn skip_space(&mut self) {
        while self.i < self.src.len() && self.src.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    pub(super) fn read_command_name(&mut self) -> &'a str {
        // at '\'
        self.i += 1;
        let start = self.i;
        if self.i >= self.src.len() {
            return "";
        }
        if !self.src.as_bytes()[self.i].is_ascii_alphabetic() {
            // advance one code point so the slice stays valid UTF-8
            self.i += self.next_char_len();
            return self.slice(start, self.i);
        }
        while self.i < self.src.len() && self.src.as_bytes()[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
        // `\operatorname*`
        self.slice(start, self.i)
    }

    fn at_end(&mut self, stop: Stop) -> bool {
        self.skip_space();
        if self.i >= self.src.len() {
            return true;
        }
        let c = self.peek_byte();
        match stop {
            Stop::None | Stop::Brace => c == b'}',
            Stop::Bracket => c == b']' || c == b'}',
            Stop::Right => c == b'}' || self.looking_at_command("right"),
            Stop::EndEnv => {
                c == b'}'
                    || c == b'&'
                    || self.looking_at_command("end")
                    || self.looking_at("\\\\")
                    || self.looking_at_command("cr")
                    || self.looking_at_command("hline")
            }
        }
    }

    pub(super) fn looking_at(&self, s: &str) -> bool {
        self.src.as_bytes()[self.i.min(self.src.len())..].starts_with(s.as_bytes())
    }

    pub(super) fn looking_at_command(&self, name: &str) -> bool {
        let bytes = self.src.as_bytes();
        if self.i >= bytes.len() || bytes[self.i] != b'\\' {
            return false;
        }
        let rest = &bytes[self.i + 1..];
        if !rest.starts_with(name.as_bytes()) {
            return false;
        }
        rest.len() == name.len() || !rest[name.len()].is_ascii_alphabetic()
    }

    pub(crate) fn parse_list(&mut self, stop: Stop) -> Result<Vec<Node>, ParseError> {
        let mut out: Vec<Node> = Vec::new();
        while !self.at_end(stop) {
            if stop == Stop::None && self.peek_byte() == b'}' {
                break;
            }
            let c = self.peek_byte();
            if c == b'^' || c == b'_' {
                // script without a base
                self.attach_scripts(&mut out, None)?;
                continue;
            }
            if c == b'\'' {
                self.attach_scripts(&mut out, None)?;
                continue;
            }
            let node = match self.parse_atom(&mut out)? {
                Some(node) => node,
                None => continue,
            };
            out.push(node);
            self.skip_space();
            let n = self.peek_byte();
            if n == b'^' || n == b'_' || n == b'\'' {
                let base = out.pop().expect("just pushed");
                self.attach_scripts(&mut out, Some(base))?;
            }
        }
        Ok(out)
    }

    fn attach_scripts(
        &mut self,
        out: &mut Vec<Node>,
        base: Option<Node>,
    ) -> Result<(), ParseError> {
        let mut sup: Option<Box<Node>> = None;
        let mut sub: Option<Box<Node>> = None;
        let mut primes: usize = 0;
        loop {
            self.skip_space();
            let c = self.peek_byte();
            if c == b'\'' {
                primes += 1;
                self.i += 1;
                continue;
            }
            if c != b'^' && c != b'_' {
                break;
            }
            self.i += 1;
            let arg = self.parse_script_arg()?;
            if c == b'^' {
                if sup.is_some() {
                    return Err(ParseError); // double superscript
                }
                sup = Some(arg);
            } else {
                if sub.is_some() {
                    return Err(ParseError);
                }
                sub = Some(arg);
            }
        }
        if primes > 0 {
            let prime_text = "′".repeat(primes);
            let prime_node = Node::Atom(Atom {
                text: prime_text,
                class: Class::Ord,
                variant: fonts::Variant::Roman,
            });
            if let Some(existing) = sup {
                sup = Some(Box::new(Node::Group(vec![prime_node, *existing])));
            } else {
                sup = Some(Box::new(prime_node));
            }
        }
        // \limits / \nolimits apply to the op base (handled at parse of the op)
        out.push(Node::SupSub(SupSub {
            base: base.map(Box::new),
            sup,
            sub,
        }));
        Ok(())
    }

    fn parse_script_arg(&mut self) -> Result<Box<Node>, ParseError> {
        self.skip_space();
        if self.i >= self.src.len() {
            return Err(ParseError);
        }
        let c = self.peek_byte();
        if c == b'{' {
            let list = self.parse_group_body()?;
            return Ok(Box::new(Node::Group(list)));
        }
        if c == b'\\' {
            let mut dummy: Vec<Node> = Vec::new();
            let node = match self.parse_atom(&mut dummy)? {
                Some(node) => node,
                None => return Err(ParseError),
            };
            return Ok(Box::new(node));
        }
        // a single character
        let len = self.next_char_len();
        let text = self.slice(self.i, self.i + len);
        self.i += len;
        Ok(Box::new(Self::char_node(text)))
    }

    fn parse_group_body(&mut self) -> Result<Vec<Node>, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'{' {
            return Err(ParseError);
        }
        self.i += 1;
        let list = self.parse_list(Stop::Brace)?;
        self.skip_space();
        if self.peek_byte() != b'}' {
            return Err(ParseError);
        }
        self.i += 1;
        Ok(list)
    }

    /// A required argument: `{...}` or a single token.
    pub(super) fn parse_arg(&mut self) -> Result<Vec<Node>, ParseError> {
        self.skip_space();
        if self.peek_byte() == b'{' {
            return self.parse_group_body();
        }
        if self.i >= self.src.len() {
            return Err(ParseError);
        }
        let mut dummy: Vec<Node> = Vec::new();
        if self.peek_byte() == b'\\' {
            let node = match self.parse_atom(&mut dummy)? {
                Some(node) => node,
                None => return Err(ParseError),
            };
            return Ok(vec![node]);
        }
        let len = self.next_char_len();
        let text = self.slice(self.i, self.i + len);
        self.i += len;
        Ok(vec![Self::char_node(text)])
    }

    /// Raw text of a `{...}` argument (balanced braces).
    pub(super) fn raw_arg(&mut self) -> Result<&'a str, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'{' {
            if self.i >= self.src.len() {
                return Err(ParseError);
            }
            let len = self.next_char_len();
            let t = self.slice(self.i, self.i + len);
            self.i += len;
            return Ok(t);
        }
        let start = self.i + 1;
        let mut depth: usize = 0;
        while self.i < self.src.len() {
            let c = self.src.as_bytes()[self.i];
            if c == b'\\' {
                // skip the escaped byte too
                self.i += 2;
                continue;
            }
            if c == b'{' {
                depth += 1;
            }
            if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    let out = self.slice(start, self.i);
                    self.i += 1;
                    return Ok(out);
                }
            }
            self.i += 1;
        }
        Err(ParseError)
    }

    pub(super) fn optional_arg(&mut self) -> Result<Option<Vec<Node>>, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'[' {
            return Ok(None);
        }
        self.i += 1;
        let list = self.parse_list(Stop::Bracket)?;
        if self.peek_byte() != b']' {
            return Err(ParseError);
        }
        self.i += 1;
        Ok(Some(list))
    }

    fn char_node(text: &str) -> Node {
        let cp = text.chars().next().unwrap_or('\u{FFFD}');
        if text.len() == 1 {
            let c = text.as_bytes()[0];
            if c == b'-' {
                return Node::Atom(Atom {
                    text: "−".to_string(),
                    class: Class::Bin,
                    variant: fonts::Variant::Normal,
                });
            }
            if c == b'*' {
                return Node::Atom(Atom {
                    text: "∗".to_string(),
                    class: Class::Bin,
                    variant: fonts::Variant::Normal,
                });
            }
            if c == b'~' {
                return Node::Space(0.25);
            }
            if c.is_ascii_digit() || c == b'.' {
                return Node::Atom(Atom {
                    text: text.to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Roman,
                });
            }
        }
        Node::Atom(Atom {
            text: text.to_string(),
            class: symbols::char_class(cp),
            variant: fonts::Variant::Normal,
        })
    }

    pub(super) fn parse_atom(&mut self, out: &mut Vec<Node>) -> Result<Option<Node>, ParseError> {
        self.skip_space();
        let c = self.peek_byte();
        if c == b'{' {
            let list = self.parse_group_body()?;
            return Ok(Some(Node::Group(list)));
        }
        if c == b'&' {
            return Err(ParseError); // outside an environment
        }
        if c != b'\\' {
            let len = self.next_char_len();
            let text = self.slice(self.i, self.i + len);
            self.i += len;
            // merge runs of digits ("123" is one atom, "1.5" too)
            if c.is_ascii_digit() {
                let start = self.i - len;
                while self.i < self.src.len() {
                    let b = self.src.as_bytes()[self.i];
                    let digit_or_point = b.is_ascii_digit()
                        || (b == b'.'
                            && self.i + 1 < self.src.len()
                            && self.src.as_bytes()[self.i + 1].is_ascii_digit());
                    if !digit_or_point {
                        break;
                    }
                    self.i += 1;
                }
                return Ok(Some(Node::Atom(Atom {
                    text: self.slice(start, self.i).to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Roman,
                })));
            }
            return Ok(Some(Self::char_node(text)));
        }
        let name = self.read_command_name();
        self.command(name, out)
    }
}

pub(super) fn strip_braces(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'{' || c == b'}' {
            i += 1;
            continue;
        }
        if c == b'\\' && i + 1 < bytes.len() {
            i += 1;
            if bytes[i] == b',' || bytes[i] == b' ' {
                out.push(b' ');
                i += 1;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).expect("strip_braces keeps valid UTF-8")
}
