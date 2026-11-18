//! Inline HTML tag scanning and supported tag styles.

use crate::markdown::ast::Style;

use super::bytes::{at, index_of_scalar_pos};

pub(super) struct TagStyle {
    pub(super) name: &'static str,
    pub(super) apply: fn(&mut Style),
}

fn set_bold(s: &mut Style) {
    s.bold = true;
}
fn set_italic(s: &mut Style) {
    s.italic = true;
}
fn set_strike(s: &mut Style) {
    s.strike = true;
}
fn set_underline(s: &mut Style) {
    s.underline = true;
}
fn set_code(s: &mut Style) {
    s.code = true;
}
fn set_mark(s: &mut Style) {
    s.highlight = true;
}
fn set_sup(s: &mut Style) {
    s.superscript = true;
}
fn set_sub(s: &mut Style) {
    s.subscript = true;
}
fn set_nothing(_: &mut Style) {}

pub(super) const INLINE_TAGS: [TagStyle; 20] = [
    TagStyle {
        name: "b",
        apply: set_bold,
    },
    TagStyle {
        name: "strong",
        apply: set_bold,
    },
    TagStyle {
        name: "i",
        apply: set_italic,
    },
    TagStyle {
        name: "em",
        apply: set_italic,
    },
    TagStyle {
        name: "s",
        apply: set_strike,
    },
    TagStyle {
        name: "del",
        apply: set_strike,
    },
    TagStyle {
        name: "strike",
        apply: set_strike,
    },
    TagStyle {
        name: "u",
        apply: set_underline,
    },
    TagStyle {
        name: "ins",
        apply: set_underline,
    },
    TagStyle {
        name: "code",
        apply: set_code,
    },
    TagStyle {
        name: "kbd",
        apply: set_code,
    },
    TagStyle {
        name: "mark",
        apply: set_mark,
    },
    TagStyle {
        name: "sup",
        apply: set_sup,
    },
    TagStyle {
        name: "sub",
        apply: set_sub,
    },
    TagStyle {
        name: "span",
        apply: set_nothing,
    },
    TagStyle {
        name: "small",
        apply: set_nothing,
    },
    TagStyle {
        name: "abbr",
        apply: set_nothing,
    },
    TagStyle {
        name: "cite",
        apply: set_nothing,
    },
    TagStyle {
        name: "q",
        apply: set_nothing,
    },
    TagStyle {
        name: "a",
        apply: set_nothing,
    },
];

pub(super) struct Tag<'a> {
    pub(super) name: &'a str,
    pub(super) closing: bool,
    pub(super) self_closing: bool,
    pub(super) end: usize,
    pub(super) href: &'a str,
}

pub(super) fn parse_tag(text: &str, i: usize) -> Option<Tag<'_>> {
    if at(text, i) != b'<' {
        return None;
    }
    let bytes = text.as_bytes();
    let mut j = i + 1;
    let closing = at(text, j) == b'/';
    if closing {
        j += 1;
    }
    let name_start = j;
    while j < text.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
        j += 1;
    }
    if j == name_start || !bytes[name_start].is_ascii_alphabetic() {
        return None;
    }
    let name = &text[name_start..j];
    let close = index_of_scalar_pos(text, j, b'>')?;
    let attrs = &text[j..close];
    if attrs.as_bytes().contains(&b'<') || attrs.as_bytes().contains(&b'\n') {
        return None;
    }
    let mut href: &str = "";
    if let Some(h) = attrs.find("href=\"") {
        let s = h + 6;
        if let Some(e) = index_of_scalar_pos(attrs, s, b'"') {
            href = &attrs[s..e];
        }
    }
    Some(Tag {
        name,
        closing,
        self_closing: !attrs.is_empty() && attrs.as_bytes()[attrs.len() - 1] == b'/',
        end: close + 1,
        href,
    })
}

pub(super) fn find_closing_tag(text: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let mut i = from;
    let mut depth: usize = 0;
    while let Some(lt) = index_of_scalar_pos(text, i, b'<') {
        if let Some(tag) = parse_tag(text, lt) {
            if tag.name.eq_ignore_ascii_case(name) {
                if tag.closing {
                    if depth == 0 {
                        return Some((lt, tag.end));
                    }
                    depth -= 1;
                } else if !tag.self_closing {
                    depth += 1;
                }
            }
            i = tag.end;
        } else {
            i = lt + 1;
        }
    }
    None
}
