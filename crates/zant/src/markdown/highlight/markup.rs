//! Language detection and tokenization for markup.

use super::{Kind, Token, is_ident, is_ident_start};

pub(super) fn is_markup(lang: &str) -> bool {
    let eq = |s: &str| lang.eq_ignore_ascii_case(s);
    eq("html") || eq("xml") || eq("svg") || eq("vue") || eq("svelte") || eq("htm") || eq("plist")
}

pub(super) fn markup_line(text: &str) -> Vec<Token<'_>> {
    let mut out: Vec<Token<'_>> = Vec::new();
    let bytes = text.as_bytes();
    let mut i: usize = 0;
    let mut plain: usize = 0;
    while i < text.len() {
        if text[i..].starts_with("<!--") {
            if i > plain {
                out.push(Token {
                    text: &text[plain..i],
                    kind: Kind::Plain,
                });
            }
            let end = match text[i..].find("-->") {
                Some(e) => i + e + 3,
                None => text.len(),
            };
            out.push(Token {
                text: &text[i..end],
                kind: Kind::Comment,
            });
            i = end;
            plain = i;
            continue;
        }
        if bytes[i] == b'<' {
            if i > plain {
                out.push(Token {
                    text: &text[plain..i],
                    kind: Kind::Plain,
                });
            }
            let mut j = i + 1;
            if j < text.len() && (bytes[j] == b'/' || bytes[j] == b'!' || bytes[j] == b'?') {
                j += 1;
            }
            out.push(Token {
                text: &text[i..j],
                kind: Kind::Punctuation,
            });
            let name_start = j;
            while j < text.len() && (is_ident(bytes[j]) || bytes[j] == b'-' || bytes[j] == b':') {
                j += 1;
            }
            if j > name_start {
                out.push(Token {
                    text: &text[name_start..j],
                    kind: Kind::Tag,
                });
            }
            // attributes until '>'
            while j < text.len() && bytes[j] != b'>' {
                if bytes[j] == b'"' || bytes[j] == b'\'' {
                    let q = bytes[j];
                    let end = match text[j + 1..].find(q as char) {
                        Some(e) => j + 1 + e + 1,
                        None => text.len(),
                    };
                    out.push(Token {
                        text: &text[j..end],
                        kind: Kind::String,
                    });
                    j = end;
                } else if is_ident_start(bytes[j]) {
                    let s = j;
                    while j < text.len()
                        && (is_ident(bytes[j]) || bytes[j] == b'-' || bytes[j] == b':')
                    {
                        j += 1;
                    }
                    out.push(Token {
                        text: &text[s..j],
                        kind: Kind::Attribute,
                    });
                } else {
                    out.push(Token {
                        text: &text[j..j + 1],
                        kind: Kind::Punctuation,
                    });
                    j += 1;
                }
            }
            if j < text.len() {
                out.push(Token {
                    text: &text[j..j + 1],
                    kind: Kind::Punctuation,
                });
                j += 1;
            }
            i = j;
            plain = i;
            continue;
        }
        i += 1;
    }
    if plain < text.len() {
        out.push(Token {
            text: &text[plain..],
            kind: Kind::Plain,
        });
    }
    out
}
