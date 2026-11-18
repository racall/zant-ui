//! Lightweight syntax highlighter (stands in for Streamdown's Shiki code
//! plugin). Tokenises per line so streaming code re-highlights cheaply.

mod extension;
mod keywords;
mod markup;
mod script_keywords;
mod spec;

#[cfg(test)]
mod tests;

pub use self::extension::extension;

use self::markup::{is_markup, markup_line};
use self::spec::spec_for;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Function,
    Type,
    Constant,
    Property,
    Tag,
    Attribute,
    Punctuation,
    Meta,
    Inserted,
    Deleted,
}

pub struct Token<'a> {
    pub text: &'a str,
    pub kind: Kind,
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

fn contains(list: &[&str], word: &str, ci: bool) -> bool {
    for w in list {
        if ci {
            if w.eq_ignore_ascii_case(word) {
                return true;
            }
        } else if *w == word {
            return true;
        }
    }
    false
}

/// Highlighter state carried across lines (open block comments / strings).
#[derive(Clone, Copy, Default)]
pub struct State {
    pub in_block_comment: bool,
    pub in_backtick: bool,
}

fn flush<'a>(out: &mut Vec<Token<'a>>, text: &'a str, from: usize, to: usize) {
    if to > from {
        out.push(Token {
            text: &text[from..to],
            kind: Kind::Plain,
        });
    }
}

fn index_of_pos(text: &str, from: usize, needle: &str) -> Option<usize> {
    let from = from.min(text.len());
    text.as_bytes()[from..]
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .map(|p| p + from)
}

fn index_of_any_pos(text: &str, from: usize, set: &[u8]) -> Option<usize> {
    let from = from.min(text.len());
    text.as_bytes()[from..]
        .iter()
        .position(|c| set.contains(c))
        .map(|p| p + from)
}

/// Tokenises one line (without the newline).
pub fn line<'a>(lang: &str, text: &'a str, state: &mut State) -> Vec<Token<'a>> {
    let mut out: Vec<Token<'a>> = Vec::new();
    let bytes = text.as_bytes();
    if lang.eq_ignore_ascii_case("diff") || lang.eq_ignore_ascii_case("patch") {
        let kind: Kind = if !text.is_empty() && bytes[0] == b'+' && !text.starts_with("+++") {
            Kind::Inserted
        } else if !text.is_empty() && bytes[0] == b'-' && !text.starts_with("---") {
            Kind::Deleted
        } else if text.starts_with("@@") {
            Kind::Meta
        } else {
            Kind::Plain
        };
        out.push(Token { text, kind });
        return out;
    }
    if is_markup(lang) {
        return markup_line(text);
    }
    let Some(spec) = spec_for(lang) else {
        out.push(Token {
            text,
            kind: Kind::Plain,
        });
        return out;
    };
    let is_json = lang.eq_ignore_ascii_case("json") || lang.eq_ignore_ascii_case("jsonc");
    let is_yaml = lang.eq_ignore_ascii_case("yaml") || lang.eq_ignore_ascii_case("yml");
    let is_toml = lang.eq_ignore_ascii_case("toml") || lang.eq_ignore_ascii_case("ini");
    let mut i: usize = 0;
    let mut plain_start: usize = 0;

    // yaml / toml keys
    if is_yaml || is_toml {
        let trimmed_start = text
            .bytes()
            .position(|c| c != b' ' && c != b'\t' && c != b'-')
            .unwrap_or(text.len());
        if is_toml && trimmed_start < text.len() && bytes[trimmed_start] == b'[' {
            out.push(Token {
                text,
                kind: Kind::Type,
            });
            return out;
        }
        let sep = index_of_any_pos(text, trimmed_start, if is_yaml { b":" } else { b"=" });
        if let Some(sep) = sep {
            let key = &text[trimmed_start..sep];
            if !key.is_empty() && !key.bytes().any(|c| c == b'"' || c == b'\'' || c == b'#') {
                flush(&mut out, text, 0, trimmed_start);
                out.push(Token {
                    text: key,
                    kind: Kind::Property,
                });
                i = sep;
                plain_start = sep;
            }
        }
    }

    while i < text.len() {
        if state.in_block_comment {
            let close = spec.block_comment.expect("block comment spec")[1];
            let end = match index_of_pos(text, i, close) {
                Some(e) => e + close.len(),
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::Comment,
            });
            if end < text.len() || text[i..end].ends_with(close) {
                state.in_block_comment = false;
            }
            i = end;
            plain_start = i;
            continue;
        }
        if state.in_backtick {
            let end = match text[i..].find('`') {
                Some(e) => i + e + 1,
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::String,
            });
            if end <= text.len() && end > i && bytes[end - 1] == b'`' {
                state.in_backtick = false;
            }
            i = end;
            plain_start = i;
            continue;
        }
        let c = bytes[i];
        // comments
        let mut matched_comment = false;
        for lc in spec.line_comments {
            if text[i..].starts_with(lc) {
                // `#` in shell-ish langs only starts a comment after whitespace
                if lc.len() == 1
                    && lc.as_bytes()[0] == b'#'
                    && i > 0
                    && !bytes[i - 1].is_ascii_whitespace()
                {
                    break;
                }
                flush(&mut out, text, plain_start, i);
                out.push(Token {
                    text: &text[i..],
                    kind: Kind::Comment,
                });
                i = text.len();
                plain_start = i;
                matched_comment = true;
                break;
            }
        }
        if matched_comment {
            break;
        }
        if let Some(bc) = spec.block_comment
            && text[i..].starts_with(bc[0])
        {
            flush(&mut out, text, plain_start, i);
            state.in_block_comment = true;
            let end = match index_of_pos(text, i + bc[0].len(), bc[1]) {
                Some(e) => {
                    state.in_block_comment = false;
                    e + bc[1].len()
                }
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::Comment,
            });
            i = end;
            plain_start = i;
            continue;
        }
        // strings
        if spec.quotes.as_bytes().contains(&c) || (c == b'`' && spec.backtick_strings) {
            flush(&mut out, text, plain_start, i);
            let mut j = i + 1;
            while j < text.len() && bytes[j] != c {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(text.len());
            if c == b'`' && j >= text.len() {
                state.in_backtick = true;
            }
            let mut kind: Kind = Kind::String;
            // JSON keys: a string followed by ':'
            if is_json {
                let mut k = end;
                while k < text.len() && bytes[k] == b' ' {
                    k += 1;
                }
                if k < text.len() && bytes[k] == b':' {
                    kind = Kind::Property;
                }
            }
            out.push(Token {
                text: &text[i..end],
                kind,
            });
            i = end;
            plain_start = i;
            continue;
        }
        // numbers
        if c.is_ascii_digit() && (i == 0 || !is_ident(bytes[i - 1])) {
            flush(&mut out, text, plain_start, i);
            let mut j = i + 1;
            while j < text.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'.' || bytes[j] == b'_')
            {
                j += 1;
            }
            out.push(Token {
                text: &text[i..j],
                kind: Kind::Number,
            });
            i = j;
            plain_start = i;
            continue;
        }
        // meta: @decorator, #include, @builtin
        if let Some(mp) = spec.meta_prefix
            && c == mp
            && i + 1 < text.len()
            && is_ident_start(bytes[i + 1])
        {
            let at_line_start = text[..i].trim_matches([' ', '\t']).is_empty();
            if mp == b'@' || at_line_start {
                flush(&mut out, text, plain_start, i);
                let mut j = i + 1;
                while j < text.len() && is_ident(bytes[j]) {
                    j += 1;
                }
                let kind = if mp == b'@' && !lang.eq_ignore_ascii_case("zig") {
                    Kind::Function
                } else {
                    Kind::Meta
                };
                out.push(Token {
                    text: &text[i..j],
                    kind,
                });
                i = j;
                plain_start = i;
                continue;
            }
        }
        if spec.dollar_vars
            && c == b'$'
            && i + 1 < text.len()
            && (is_ident_start(bytes[i + 1]) || bytes[i + 1] == b'{')
        {
            flush(&mut out, text, plain_start, i);
            let mut j = i + 1;
            if bytes[j] == b'{' {
                j = match index_of_any_pos(text, j, b"}") {
                    Some(e) => e + 1,
                    None => text.len(),
                };
            } else {
                while j < text.len() && is_ident(bytes[j]) {
                    j += 1;
                }
            }
            out.push(Token {
                text: &text[i..j],
                kind: Kind::Constant,
            });
            i = j;
            plain_start = i;
            continue;
        }
        // identifiers
        if is_ident_start(c) && (i == 0 || !is_ident(bytes[i - 1])) {
            let mut j = i + 1;
            while j < text.len()
                && (is_ident(bytes[j]) || (bytes[j] == b'-' && lang.eq_ignore_ascii_case("css")))
            {
                j += 1;
            }
            let word = &text[i..j];
            let mut next = j;
            while next < text.len() && bytes[next] == b' ' {
                next += 1;
            }
            let kind: Option<Kind> = if contains(spec.keywords, word, spec.case_insensitive) {
                Some(Kind::Keyword)
            } else if contains(spec.constants, word, spec.case_insensitive) {
                Some(Kind::Constant)
            } else if contains(spec.types, word, spec.case_insensitive) {
                Some(Kind::Type)
            } else if next < text.len() && bytes[next] == b'(' {
                Some(Kind::Function)
            } else if spec.capitalized_types && bytes[i].is_ascii_uppercase() && word.len() > 1 {
                Some(Kind::Type)
            } else if i > 0 && bytes[i - 1] == b'.' {
                Some(Kind::Property)
            } else {
                None
            };
            if let Some(k) = kind {
                flush(&mut out, text, plain_start, i);
                out.push(Token {
                    text: word,
                    kind: k,
                });
                plain_start = j;
            }
            i = j;
            continue;
        }
        if b"{}[]();,.:=<>+-*/%!&|^~?".contains(&c) {
            flush(&mut out, text, plain_start, i);
            out.push(Token {
                text: &text[i..i + 1],
                kind: Kind::Punctuation,
            });
            i += 1;
            plain_start = i;
            continue;
        }
        i += 1;
    }
    flush(&mut out, text, plain_start, text.len());
    out
}
