use super::{Error, JsonValue, Str, Thrown};
use std::collections::HashMap;

struct Parser<'a> {
    text: &'a str,
    at: usize,
}
enum Frame {
    Array(Vec<JsonValue>, u8),
    Object(Vec<(Str, JsonValue)>, HashMap<Str, usize>, Option<Str>, u8),
}

/// Parse ECMA-404 JSON with an explicit stack, limited to 512 containers.
pub fn parse(text: &str) -> Result<JsonValue, Thrown> {
    Parser { text, at: 0 }
        .parse()
        .map_err(|msg| Thrown::error(Error::new("SyntaxError", Str::from(msg))))
}
impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }
    fn location(&self, message: &str) -> String {
        let prefix = &self.text[..self.at];
        let position = prefix.encode_utf16().count();
        let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count()
            + 1;
        format!("{message} in JSON at position {position} (line {line} column {column})")
    }
    fn unexpected(&self) -> String {
        let Some(token) = self.text[self.at..].chars().next() else {
            return "Unexpected end of JSON input".into();
        };
        if matches!(
            self.text,
            "undefined" | "NaN" | "Infinity" | "[object Object]"
        ) {
            return format!("\"{}\" is not valid JSON", self.text);
        }
        let units: Vec<char> = self.text.chars().collect();
        let at = self.text[..self.at].chars().count();
        let context = if units.len() <= 20 {
            format!("\"{}\"", self.text)
        } else if at < 10 {
            format!("\"{}\"...", units[..10].iter().collect::<String>())
        } else if at + 10 >= units.len() {
            format!(
                "...\"{}\"",
                units[units.len() - 10..].iter().collect::<String>()
            )
        } else {
            format!(
                "...\"{}\"...",
                units[at - 10..at + 10].iter().collect::<String>()
            )
        };
        format!("Unexpected token '{token}', {context} is not valid JSON")
    }
    fn parse(&mut self) -> Result<JsonValue, String> {
        let mut stack = Vec::new();
        let mut pending = None;
        loop {
            self.ws();
            if let Some(value) = pending.take() {
                match stack.last_mut() {
                    Some(Frame::Array(items, state)) => {
                        items.push(value);
                        *state = 1;
                    }
                    Some(Frame::Object(items, index, key, state)) => {
                        let key = key.take().expect("object key before value");
                        if let Some(&i) = index.get(&key) {
                            items[i].1 = value;
                        } else {
                            index.insert(key.clone(), items.len());
                            items.push((key, value));
                        }
                        *state = 1;
                    }
                    None => {
                        if self.peek().is_some() {
                            // This diagnostic omits the usual "in JSON" phrase.
                            return Err(self
                                .location("Unexpected non-whitespace character after JSON")
                                .replace(" in JSON at", " at"));
                        }
                        return Ok(value);
                    }
                }
                continue;
            }
            match stack.last_mut() {
                Some(Frame::Array(_, state)) => {
                    if *state == 0 && self.peek() == Some(b']') {
                        self.at += 1;
                        let Frame::Array(items, _) = stack.pop().unwrap() else {
                            unreachable!()
                        };
                        pending = Some(JsonValue::Array(items));
                        continue;
                    }
                    if *state == 1 {
                        match self.peek() {
                            Some(b']') => {
                                self.at += 1;
                                let Frame::Array(items, _) = stack.pop().unwrap() else {
                                    unreachable!()
                                };
                                pending = Some(JsonValue::Array(items));
                            }
                            Some(b',') => {
                                self.at += 1;
                                *state = 2;
                            }
                            _ => {
                                return Err(
                                    self.location("Expected ',' or ']' after array element")
                                );
                            }
                        }
                        continue;
                    }
                }
                Some(Frame::Object(_, _, key, state)) => match *state {
                    0 | 2 => {
                        if *state == 0 && self.peek() == Some(b'}') {
                            self.at += 1;
                            let Frame::Object(items, ..) = stack.pop().unwrap() else {
                                unreachable!()
                            };
                            pending = Some(JsonValue::Object(items));
                            continue;
                        }
                        if self.peek() != Some(b'"') {
                            return Err(self.location(if *state == 0 {
                                "Expected property name or '}'"
                            } else {
                                "Expected double-quoted property name"
                            }));
                        }
                        *key = Some(self.string()?);
                        *state = 3;
                        continue;
                    }
                    3 => {
                        if self.peek() != Some(b':') {
                            return Err(self.location("Expected ':' after property name"));
                        }
                        self.at += 1;
                        *state = 4;
                        continue;
                    }
                    1 => {
                        match self.peek() {
                            Some(b'}') => {
                                self.at += 1;
                                let Frame::Object(items, ..) = stack.pop().unwrap() else {
                                    unreachable!()
                                };
                                pending = Some(JsonValue::Object(items));
                            }
                            Some(b',') => {
                                self.at += 1;
                                *state = 2;
                            }
                            _ => {
                                return Err(
                                    self.location("Expected ',' or '}' after property value")
                                );
                            }
                        }
                        continue;
                    }
                    _ => {}
                },
                None => {}
            }
            pending = match self.peek() {
                Some(b'[' | b'{') => {
                    if stack.len() >= 512 {
                        return Err(self.location("JSON nesting limit of 512 exceeded"));
                    }
                    let frame = if self.peek() == Some(b'[') {
                        Frame::Array(Vec::new(), 0)
                    } else {
                        Frame::Object(Vec::new(), HashMap::new(), None, 0)
                    };
                    self.at += 1;
                    stack.push(frame);
                    None
                }
                Some(b'"') => Some(JsonValue::String(self.string()?)),
                Some(b'-' | b'0'..=b'9') => Some(JsonValue::Number(self.number()?)),
                Some(b'n') => {
                    self.literal("null")?;
                    Some(JsonValue::Null)
                }
                Some(b't') => {
                    self.literal("true")?;
                    Some(JsonValue::Bool(true))
                }
                Some(b'f') => {
                    self.literal("false")?;
                    Some(JsonValue::Bool(false))
                }
                _ => return Err(self.unexpected()),
            };
        }
    }
    fn literal(&mut self, literal: &str) -> Result<(), String> {
        for b in literal.bytes() {
            if self.peek() != Some(b) {
                return Err(self.unexpected());
            }
            self.at += 1;
        }
        Ok(())
    }
    fn number(&mut self) -> Result<f64, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.location("No number after minus sign"));
            }
        }
        if self.peek() == Some(b'0') {
            self.at += 1;
            if matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.location("Unexpected number"));
            }
        } else {
            self.digits();
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.location("Unterminated fractional number"));
            }
            self.digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.location("Exponent part is missing a number"));
            }
            self.digits();
        }
        self.text[start..self.at]
            .parse()
            .map_err(|_| self.unexpected())
    }
    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
    }
    fn hex(&mut self) -> Result<u16, String> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = self.peek().and_then(|b| (b as char).to_digit(16));
            let Some(digit) = digit else {
                return Err(self.location("Bad Unicode escape"));
            };
            value = value * 16 + digit as u16;
            self.at += 1;
        }
        Ok(value)
    }
    fn string(&mut self) -> Result<Str, String> {
        self.at += 1;
        let mut start = self.at;
        let mut out: Option<String> = None;
        loop {
            match self.peek() {
                None => return Err(self.location("Unterminated string")),
                Some(b'"') => {
                    let tail = &self.text[start..self.at];
                    self.at += 1;
                    return Ok(match out {
                        Some(mut out) => {
                            out.push_str(tail);
                            Str::from(out)
                        }
                        None => Str::copy(tail),
                    });
                }
                Some(0..=31) => {
                    return Err(self.location("Bad control character in string literal"));
                }
                Some(b'\\') => {
                    let out = out.get_or_insert_with(String::new);
                    out.push_str(&self.text[start..self.at]);
                    self.at += 1;
                    match self.peek() {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{8}'),
                        Some(b'f') => out.push('\u{c}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            self.at += 1;
                            let high = self.hex()?;
                            let mut scalar = u32::from(high);
                            if (0xd800..=0xdbff).contains(&high)
                                && self.text[self.at..].starts_with("\\u")
                            {
                                let saved = self.at;
                                self.at += 2;
                                let low = self.hex()?;
                                if (0xdc00..=0xdfff).contains(&low) {
                                    scalar = 0x10000
                                        + ((u32::from(high) - 0xd800) << 10)
                                        + u32::from(low)
                                        - 0xdc00;
                                } else {
                                    self.at = saved;
                                }
                            }
                            out.push(char::from_u32(scalar).unwrap_or('\u{fffd}'));
                            start = self.at;
                            continue;
                        }
                        None => return Err(self.location("Unterminated string")),
                        _ => return Err(self.location("Bad escaped character")),
                    }
                    self.at += 1;
                    start = self.at;
                }
                // Scan UTF-8 bytes; slices are taken only at ASCII boundaries.
                Some(_) => self.at += 1,
            }
        }
    }
}
