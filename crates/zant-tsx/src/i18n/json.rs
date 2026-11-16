//! A JSON reader preserving duplicate members and source byte offsets.

use crate::error::Diag;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub loc: u32,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub key: String,
    pub key_loc: u32,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Object(Vec<Member>),
    Array(Vec<Value>),
    String(String),
    /// Original spelling, without conversion to a limited numeric type.
    Number(String),
    Bool(bool),
    Null,
}

/// Read one complete JSON value, optionally preceded by a UTF-8 BOM.
/// Locations include the BOM and point at opening quotes for strings and keys.
/// At most 32 nested containers are accepted, counting the root container.
pub fn parse(src: &str) -> Result<Value, Diag> {
    if src.len() > u32::MAX as usize {
        return Err(Diag {
            loc: 0,
            msg: "JSON exceeds the byte offset limit".into(),
        });
    }
    let mut reader = Reader {
        src,
        at: if src.starts_with('\u{feff}') { 3 } else { 0 },
    };
    let value = reader.value(0)?;
    reader.whitespace();
    if reader.at != src.len() {
        return reader.error("expected end of JSON");
    }
    Ok(value)
}

/// Map a decoded byte offset in a valid JSON string back to its source.
/// Escaped scalar bytes map to their backslash; the end maps to the closing quote.
pub fn source_offset(src: &str, string_loc: u32, decoded: u32) -> u32 {
    let mut reader = Reader {
        src,
        at: string_loc as usize + 1,
    };
    let mut remaining = decoded as usize;
    while remaining > 0 && !matches!(reader.peek(), None | Some(b'"')) {
        let start = reader.at;
        if reader.take(b'\\') {
            let Ok(c) = reader.escape() else {
                return start as u32;
            };
            if remaining < c.len_utf8() {
                return start as u32;
            }
            remaining -= c.len_utf8();
        } else {
            reader.at += 1;
            remaining -= 1;
        }
    }
    reader.at as u32
}

struct Reader<'a> {
    src: &'a str,
    at: usize,
}

impl Reader<'_> {
    fn error<T>(&self, msg: &str) -> Result<T, Diag> {
        Err(Diag {
            loc: self.at as u32,
            msg: msg.into(),
        })
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.at).copied()
    }

    fn take(&mut self, byte: u8) -> bool {
        if self.peek() != Some(byte) {
            return false;
        }
        self.at += 1;
        true
    }

    fn expect(&mut self, byte: u8, msg: &str) -> Result<(), Diag> {
        if self.take(byte) {
            Ok(())
        } else {
            self.error(msg)
        }
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.at += 1;
        }
    }

    fn value(&mut self, depth: u8) -> Result<Value, Diag> {
        self.whitespace();
        let loc = self.at as u32;
        if depth == 32 && matches!(self.peek(), Some(b'{' | b'[')) {
            return self.error("JSON nesting exceeds 32 levels");
        }
        let kind = match self.peek() {
            Some(b'{') => self.object(depth + 1)?,
            Some(b'[') => self.array(depth + 1)?,
            Some(b'"') => Kind::String(self.string()?),
            Some(b'-' | b'0'..=b'9') => self.number()?,
            Some(b't') => {
                self.literal("true")?;
                Kind::Bool(true)
            }
            Some(b'f') => {
                self.literal("false")?;
                Kind::Bool(false)
            }
            Some(b'n') => {
                self.literal("null")?;
                Kind::Null
            }
            _ => return self.error("expected a JSON value"),
        };
        Ok(Value { loc, kind })
    }

    fn object(&mut self, depth: u8) -> Result<Kind, Diag> {
        self.at += 1;
        self.whitespace();
        let mut members = Vec::new();
        if self.take(b'}') {
            return Ok(Kind::Object(members));
        }
        loop {
            let key_loc = self.at as u32;
            let key = self.string()?;
            self.whitespace();
            self.expect(b':', "expected `:` after object key")?;
            let value = self.value(depth)?;
            members.push(Member {
                key,
                key_loc,
                value,
            });
            self.whitespace();
            if self.take(b'}') {
                break;
            }
            self.expect(b',', "expected `,` or `}` after object value")?;
            self.whitespace();
        }
        Ok(Kind::Object(members))
    }

    fn array(&mut self, depth: u8) -> Result<Kind, Diag> {
        self.at += 1;
        self.whitespace();
        let mut values = Vec::new();
        if self.take(b']') {
            return Ok(Kind::Array(values));
        }
        loop {
            values.push(self.value(depth)?);
            self.whitespace();
            if self.take(b']') {
                break;
            }
            self.expect(b',', "expected `,` or `]` after array value")?;
        }
        Ok(Kind::Array(values))
    }

    fn literal(&mut self, text: &str) -> Result<(), Diag> {
        if !self.src[self.at..].starts_with(text) {
            return self.error("invalid JSON literal");
        }
        self.at += text.len();
        Ok(())
    }

    fn digits(&mut self) -> Result<(), Diag> {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        if self.at == start {
            return self.error("expected a digit");
        }
        Ok(())
    }

    fn number(&mut self) -> Result<Kind, Diag> {
        let start = self.at;
        self.take(b'-');
        if !self.take(b'0') {
            self.digits()?;
        }
        if self.take(b'.') {
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            self.digits()?;
        }
        Ok(Kind::Number(self.src[start..self.at].into()))
    }

    fn string(&mut self) -> Result<String, Diag> {
        self.expect(b'"', "expected a quoted JSON string")?;
        let mut output = String::new();
        let mut start = self.at;
        loop {
            match self.peek() {
                Some(b'"') => {
                    output.push_str(&self.src[start..self.at]);
                    self.at += 1;
                    return Ok(output);
                }
                Some(b'\\') => {
                    output.push_str(&self.src[start..self.at]);
                    self.at += 1;
                    output.push(self.escape()?);
                    start = self.at;
                }
                Some(0..=0x1f) => return self.error("escape control characters in JSON strings"),
                Some(_) => self.at += 1,
                None => return self.error("unterminated JSON string"),
            }
        }
    }

    fn escape(&mut self) -> Result<char, Diag> {
        let escaped = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.at += 1;
                let first = self.hex4()?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    self.expect(b'\\', "expected a low surrogate escape")?;
                    self.expect(b'u', "expected a low surrogate escape")?;
                    let second = self.hex4()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return self.error("expected a low surrogate escape");
                    }
                    0x10000 + ((first - 0xd800) << 10) + second - 0xdc00
                } else {
                    first
                };
                return char::from_u32(scalar).ok_or_else(|| Diag {
                    loc: (self.at - 4) as u32,
                    msg: "lone Unicode surrogate".into(),
                });
            }
            _ => return self.error("invalid JSON escape"),
        };
        self.at += 1;
        Ok(escaped)
    }

    fn hex4(&mut self) -> Result<u32, Diag> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(b'0'..=b'9') => self.peek().unwrap() - b'0',
                Some(b'a'..=b'f') => self.peek().unwrap() - b'a' + 10,
                Some(b'A'..=b'F') => self.peek().unwrap() - b'A' + 10,
                _ => return self.error("expected four hexadecimal digits"),
            };
            value = value * 16 + u32::from(digit);
            self.at += 1;
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_offsets_map_escapes_and_utf8_to_source_bytes() {
        let utf8: &[u32] = &[1, 2, 4, 4, 4, 10, 10, 10, 10, 22];
        let escapes: &[u32] = &[1, 3, 5, 7, 9, 11, 13, 15, 21];
        for (src, expected) in [
            (r#""a\n\u4E2D\uD83D\uDE00""#, utf8),
            ("\"中😀\"", &[1, 2, 3, 4, 5, 6, 7, 8][..]),
            (r#""\"\\\/\b\f\r\t\u0041""#, escapes),
            (r#""""#, &[1][..]),
        ] {
            for (prefix, loc) in [("", 0), ("中:", 4)] {
                let src = format!("{prefix}{src}");
                for (offset, source) in expected.iter().copied().enumerate() {
                    assert_eq!(source_offset(&src, loc, offset as u32), source + loc);
                }
            }
        }
    }

    #[test]
    fn strings_decode_every_escape_and_unicode() {
        let value = parse(r#""\"\\\/\b\f\n\r\t\u0041\u00e9\uD83D\uDE00中""#).unwrap();
        assert_eq!(
            value.kind,
            Kind::String("\"\\/\u{8}\u{c}\n\r\tAé😀中".into())
        );
        for src in [r#""\uD800""#, r#""\uDC00""#, r#""\uD800\u0041""#] {
            assert!(parse(src).is_err(), "{src}");
        }
    }

    #[test]
    fn grammar_accepts_all_value_types_without_numeric_rounding() {
        let value = parse(" \t\r\n[{}, [], true, false, null, -0, 1.5e+400, 12345678901234567890]")
            .unwrap();
        let Kind::Array(values) = value.kind else {
            panic!()
        };
        assert_eq!(values.len(), 8);
        assert_eq!(values[2].kind, Kind::Bool(true));
        assert_eq!(values[3].kind, Kind::Bool(false));
        assert_eq!(values[4].kind, Kind::Null);
        assert_eq!(values[6].kind, Kind::Number("1.5e+400".into()));
        assert_eq!(values[7].kind, Kind::Number("12345678901234567890".into()));
    }

    #[test]
    fn invalid_json_is_rejected() {
        for src in [
            "",
            "01",
            "-",
            "1.",
            "1e",
            "1e+",
            "+1",
            ".1",
            "NaN",
            "true false",
            "[1,]",
            "{\"a\":1,}",
            "{a:1}",
            "[1 2]",
            "{\"a\" 1}",
            "/*x*/null",
            "\u{a0}null",
            "\"\n\"",
            r#""\x""#,
            r#""\u12z4""#,
            "\"open",
        ] {
            assert!(parse(src).is_err(), "{src:?}");
        }
    }

    #[test]
    fn bom_and_duplicate_members_keep_byte_offsets() {
        let Kind::Object(members) = parse("\u{feff}{\"é\":\"中\",\"é\":0}").unwrap().kind else {
            panic!()
        };
        assert_eq!((members[0].key_loc, members[0].value.loc), (4, 9));
        assert_eq!((members[1].key_loc, members[1].value.loc), (15, 20));
        assert_eq!(members[0].key, members[1].key);
    }

    #[test]
    fn container_depth_is_limited_to_32() {
        let src = format!("{}0{}", "[".repeat(32), "]".repeat(32));
        assert!(parse(&src).is_ok());
        assert_eq!(parse(&format!("[{src}]")).unwrap_err().loc, 32);
        let src = format!("{}null{}", "{\"a\":".repeat(32), "}".repeat(32));
        assert!(parse(&src).is_ok());
        assert!(parse(&format!("{{\"a\":{src}}}")).is_err());
    }
}
