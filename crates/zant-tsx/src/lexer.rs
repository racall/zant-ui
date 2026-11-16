//! Tokenizer for the TSX subset. JS tokens come from `next`; the parser
//! switches to `jsx_tag_token`, `jsx_text` and `template_part` where TSX's
//! grammar is context sensitive.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Eof,
    Identifier,
    Number,
    /// Quoted string; the token spans the quotes.
    String,
    /// Opening backtick of a template literal.
    Template,
    Punct,
    /// A byte the lexer does not understand.
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub start: u32,
    pub end: u32,
    /// A line break comes before the token (ASI, `return`, postfix `++`).
    pub nl_before: bool,
}

impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start as usize..self.end as usize]
    }

    pub fn is(&self, source: &str, value: &str) -> bool {
        (self.kind == Kind::Punct || self.kind == Kind::Identifier) && self.text(source) == value
    }
}

/// Longest match first. `>` is always its own token, as in tsc and oxc:
/// the parser joins `>=`, `>>`, `>>>`, `>>=` and `>>>=` where an operator is
/// expected
/// (`Parser.rescan_greater`), so `>` closing type arguments or a JSX tag
/// never needs splitting.
const PUNCTS: [&str; 25] = [
    "===", "!==", "...", "**=", "<<=", "=>", "==", "!=", "<=", "<<", "&&", "||", "??", "?.", "++",
    "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "**",
];
const SINGLE_PUNCTS: &str = "{}()[];,<>+-*/%=!?:.&|^~@#";

/// Failure of [`cook`] / [`decode_string`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LexError {
    #[error("bad escape")]
    BadEscape,
}

pub struct Lexer<'a> {
    pub source: &'a str,
    pub pos: u32,
}

impl<'a> Lexer<'a> {
    pub fn init(source: &'a str) -> Lexer<'a> {
        Lexer { source, pos: 0 }
    }

    fn peek_byte(&self, offset: u32) -> u8 {
        let i = self.pos + offset;
        if (i as usize) < self.source.len() {
            self.source.as_bytes()[i as usize]
        } else {
            0
        }
    }

    /// Skips whitespace and comments. Returns false for an unterminated
    /// block comment (left at its start).
    pub fn skip_trivia(&mut self) -> bool {
        while (self.pos as usize) < self.source.len() {
            let c = self.source.as_bytes()[self.pos as usize];
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                self.pos += 1;
            } else if c == b'/' && self.peek_byte(1) == b'/' {
                while (self.pos as usize) < self.source.len()
                    && self.source.as_bytes()[self.pos as usize] != b'\n'
                {
                    self.pos += 1;
                }
            } else if c == b'/' && self.peek_byte(1) == b'*' {
                let from = (self.pos + 2) as usize;
                let Some(end) = self.source[from..].find("*/") else {
                    return false;
                };
                self.pos = (from + end + 2) as u32;
            } else if c == 0xEF && self.peek_byte(1) == 0xBB && self.peek_byte(2) == 0xBF {
                self.pos += 3; // UTF-8 BOM
            } else {
                break;
            }
        }
        true
    }

    // Named `next` like a lexer's usual entry point; it is not an `Iterator`
    // (it never returns `Option<Token>`).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Token {
        let before = self.pos;
        let mut t = self.next_token();
        t.nl_before = self.source.as_bytes()[before as usize..t.start as usize].contains(&b'\n');
        t
    }

    fn next_token(&mut self) -> Token {
        if !self.skip_trivia() {
            return self.make(Kind::Invalid, self.pos, self.source.len() as u32);
        }
        let start = self.pos;
        if start as usize >= self.source.len() {
            return self.make(Kind::Eof, start, start);
        }
        let c = self.source.as_bytes()[start as usize];
        if is_ident_start(c) {
            self.pos += 1;
            while (self.pos as usize) < self.source.len()
                && is_ident_char(self.source.as_bytes()[self.pos as usize])
            {
                self.pos += 1;
            }
            return self.make(Kind::Identifier, start, self.pos);
        }
        if is_digit(c) || (c == b'.' && is_digit(self.peek_byte(1))) {
            return self.number(start);
        }
        if c == b'"' || c == b'\'' {
            return self.string(start, c);
        }
        if c == b'`' {
            self.pos += 1;
            return self.make(Kind::Template, start, self.pos);
        }
        for p in PUNCTS {
            if self.source[start as usize..].starts_with(p) {
                self.pos += p.len() as u32;
                return self.make(Kind::Punct, start, self.pos);
            }
        }
        if SINGLE_PUNCTS.as_bytes().contains(&c) {
            self.pos += 1;
            return self.make(Kind::Punct, start, self.pos);
        }
        self.pos += 1;
        self.make(Kind::Invalid, start, self.pos)
    }

    /// Inside a JSX tag: names (with `-`), strings, and single-character
    /// punctuation so `>` never merges into `>=`.
    pub fn jsx_tag_token(&mut self) -> Token {
        if !self.skip_trivia() {
            return self.make(Kind::Invalid, self.pos, self.source.len() as u32);
        }
        let start = self.pos;
        if start as usize >= self.source.len() {
            return self.make(Kind::Eof, start, start);
        }
        let c = self.source.as_bytes()[start as usize];
        if is_ident_start(c) {
            self.pos += 1;
            while (self.pos as usize) < self.source.len() {
                let b = self.source.as_bytes()[self.pos as usize];
                if !is_ident_char(b) && b != b'-' {
                    break;
                }
                self.pos += 1;
            }
            return self.make(Kind::Identifier, start, self.pos);
        }
        if c == b'"' || c == b'\'' {
            // JSX attribute strings have no escapes
            let Some(end) = self.source.as_bytes()[start as usize + 1..]
                .iter()
                .position(|&b| b == c)
            else {
                self.pos = self.source.len() as u32;
                return self.make(Kind::Invalid, start, self.pos);
            };
            self.pos = start + 1 + end as u32 + 1;
            return self.make(Kind::String, start, self.pos);
        }
        self.pos += 1;
        if b"{}=/>.:".contains(&c) {
            return self.make(Kind::Punct, start, self.pos);
        }
        self.make(Kind::Invalid, start, self.pos)
    }

    /// Raw JSX text up to the next `{` or `<` (or the end).
    pub fn jsx_text(&mut self) -> Token {
        let start = self.pos;
        while (self.pos as usize) < self.source.len() {
            let c = self.source.as_bytes()[self.pos as usize];
            if c == b'{' || c == b'<' {
                break;
            }
            self.pos += 1;
        }
        self.make(Kind::String, start, self.pos)
    }

    /// Raw template text from `pos` to the closing backtick or the next
    /// `${`, which is consumed.
    pub fn template_part(&mut self) -> TemplatePart<'a> {
        let source = self.source;
        let start = self.pos;
        while (self.pos as usize) < source.len() {
            let c = source.as_bytes()[self.pos as usize];
            if c == b'\\' {
                self.pos = (self.pos + 2).min(source.len() as u32);
                continue;
            }
            if c == b'`' {
                let raw = &source.as_bytes()[start as usize..self.pos as usize];
                self.pos += 1;
                return TemplatePart {
                    raw,
                    start,
                    end: TemplateEnd::Backtick,
                };
            }
            if c == b'$' && self.peek_byte(1) == b'{' {
                let raw = &source.as_bytes()[start as usize..self.pos as usize];
                self.pos += 2;
                return TemplatePart {
                    raw,
                    start,
                    end: TemplateEnd::Substitution,
                };
            }
            self.pos += 1;
        }
        TemplatePart {
            raw: &source.as_bytes()[start as usize..],
            start,
            end: TemplateEnd::Eof,
        }
    }

    fn number(&mut self, start: u32) -> Token {
        let c = self.source.as_bytes()[start as usize];
        if c == b'0'
            && ((self.peek_byte(1) | 0x20) == b'x'
                || (self.peek_byte(1) | 0x20) == b'b'
                || (self.peek_byte(1) | 0x20) == b'o')
        {
            self.pos += 2;
            while (self.pos as usize) < self.source.len()
                && (self.source.as_bytes()[self.pos as usize].is_ascii_hexdigit()
                    || self.source.as_bytes()[self.pos as usize] == b'_')
            {
                self.pos += 1;
            }
            return self.make(Kind::Number, start, self.pos);
        }
        while (self.pos as usize) < self.source.len()
            && (is_digit(self.source.as_bytes()[self.pos as usize])
                || self.source.as_bytes()[self.pos as usize] == b'_')
        {
            self.pos += 1;
        }
        if (self.peek_byte(0) == b'.' && is_digit(self.peek_byte(1)))
            || (self.peek_byte(0) == b'.'
                && !is_ident_start(self.peek_byte(1))
                && self.peek_byte(1) != b'.')
        {
            self.pos += 1;
            while (self.pos as usize) < self.source.len()
                && (is_digit(self.source.as_bytes()[self.pos as usize])
                    || self.source.as_bytes()[self.pos as usize] == b'_')
            {
                self.pos += 1;
            }
        }
        if (self.peek_byte(0) | 0x20) == b'e' {
            let sign: u32 = if self.peek_byte(1) == b'+' || self.peek_byte(1) == b'-' {
                1
            } else {
                0
            };
            if is_digit(self.peek_byte(1 + sign)) {
                self.pos += 1 + sign;
                while (self.pos as usize) < self.source.len()
                    && is_digit(self.source.as_bytes()[self.pos as usize])
                {
                    self.pos += 1;
                }
            }
        }
        self.make(Kind::Number, start, self.pos)
    }

    fn string(&mut self, start: u32, quote: u8) -> Token {
        self.pos += 1;
        while (self.pos as usize) < self.source.len() {
            let c = self.source.as_bytes()[self.pos as usize];
            if c == b'\\' {
                self.pos += 2;
                continue;
            }
            if c == b'\n' {
                break;
            }
            self.pos += 1;
            if c == quote {
                return self.make(Kind::String, start, self.pos);
            }
        }
        self.pos = self.pos.min(self.source.len() as u32);
        self.make(Kind::Invalid, start, self.pos)
    }

    fn make(&self, kind: Kind, start: u32, end: u32) -> Token {
        Token {
            kind,
            start,
            end,
            nl_before: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateEnd {
    Backtick,
    Substitution,
    Eof,
}

pub struct TemplatePart<'a> {
    pub raw: &'a [u8],
    pub start: u32,
    pub end: TemplateEnd,
}

pub fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$' || c >= 0x80
}

pub fn is_ident_char(c: u8) -> bool {
    is_ident_start(c) || is_digit(c)
}

fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

/// Decodes a quoted JS string token (escapes included).
pub fn decode_string(raw: &[u8]) -> Result<Vec<u8>, LexError> {
    cook(&raw[1..raw.len() - 1])
}

/// Applies JS escapes (`\n`, `\u{1F600}`, ...) to string or template text.
pub fn cook(raw: &[u8]) -> Result<Vec<u8>, LexError> {
    if !raw.contains(&b'\\') {
        return Ok(raw.to_vec());
    }
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < raw.len() {
        let c = raw[i];
        if c != b'\\' || i + 1 >= raw.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let e = raw[i + 1];
        i += 2;
        match e {
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'r' => out.push(b'\r'),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'v' => out.push(11),
            b'0' => out.push(0),
            b'\n' => {} // line continuation
            b'u' => {
                let cp: u32;
                if i < raw.len() && raw[i] == b'{' {
                    let end = raw[i..]
                        .iter()
                        .position(|&b| b == b'}')
                        .map(|p| p + i)
                        .ok_or(LexError::BadEscape)?;
                    cp = parse_hex(&raw[i + 1..end])?;
                    i = end + 1;
                } else {
                    if i + 4 > raw.len() {
                        return Err(LexError::BadEscape);
                    }
                    cp = parse_hex(&raw[i..i + 4])?;
                    i += 4;
                }
                let ch = char::from_u32(cp).ok_or(LexError::BadEscape)?;
                let mut buf = [0u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            b'x' => {
                if i + 2 > raw.len() {
                    return Err(LexError::BadEscape);
                }
                let v = parse_hex(&raw[i..i + 2])?;
                i += 2;
                let ch = char::from_u32(v).expect("u8 hex escape is a scalar value");
                let mut buf = [0u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            _ => out.push(e),
        }
    }
    Ok(out)
}

fn parse_hex(raw: &[u8]) -> Result<u32, LexError> {
    if raw.is_empty() {
        return Err(LexError::BadEscape);
    }
    let mut value: u32 = 0;
    for &c in raw {
        let digit: u32 = match c {
            b'0'..=b'9' => (c - b'0') as u32,
            b'a'..=b'f' => (c - b'a' + 10) as u32,
            b'A'..=b'F' => (c - b'A' + 10) as u32,
            _ => return Err(LexError::BadEscape),
        };
        value = value
            .checked_mul(16)
            .and_then(|v| v.checked_add(digit))
            .ok_or(LexError::BadEscape)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        let mut lx = Lexer::init("const [n, setN] = useState<u32>(0); // c\n x === 1.5e3 => `");
        let src = lx.source;
        let want = [
            "const", "[", "n", ",", "setN", "]", "=", "useState", "<", "u32", ">", "(", "0", ")",
            ";", "x", "===", "1.5e3", "=>", "`",
        ];
        for w in want {
            let t = lx.next();
            assert_eq!(w, t.text(src));
        }
        assert_eq!(Kind::Eof, lx.next().kind);
    }

    #[test]
    fn jsx_tag_tokens_keep_gt_apart() {
        let mut lx = Lexer::init("Text size={1}>=");
        assert_eq!("Text", lx.jsx_tag_token().text(lx.source));
        assert_eq!("size", lx.jsx_tag_token().text(lx.source));
        assert_eq!("=", lx.jsx_tag_token().text(lx.source));
        assert_eq!("{", lx.jsx_tag_token().text(lx.source));
        lx.next();
        assert_eq!("}", lx.jsx_tag_token().text(lx.source));
        assert_eq!(">", lx.jsx_tag_token().text(lx.source));
        assert_eq!("=", lx.jsx_text().text(lx.source));
    }

    #[test]
    fn cook_escapes() {
        assert_eq!(
            cook(b"a\\nb\\u{1F600}\\u00e9").unwrap(),
            "a\nb\u{1F600}é".as_bytes()
        );
    }
}
