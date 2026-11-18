//! Links, images, footnote references, and reference label lookup.

use std::collections::HashMap;

use crate::markdown::{ast::Style, remend};

use super::bytes::{at, index_of_scalar_pos, run_length};
use super::code_math::code_span_end;
use super::parsing::parse_into;
use super::{Builder, Context};

#[derive(Clone)]
pub struct LinkDef {
    pub url: String,
    pub title: String,
}

pub type RefMap = HashMap<String, LinkDef>;

/// Balanced `[...]` starting at `open` (`text[open] == '['`). Returns index of `]`.
fn match_bracket(text: &str, open: usize) -> Option<usize> {
    let mut depth: usize = 0;
    let mut i = open;
    while i < text.len() {
        match text.as_bytes()[i] {
            b'\\' => i += 2,
            b'`' => {
                let n = run_length(text, i, b'`');
                i = if let Some(end) = code_span_end(text, i, n) {
                    end + n
                } else {
                    i + n
                };
            }
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

struct Destination<'a> {
    url: &'a str,
    title: &'a str,
    end: usize,
}

/// Parses `(url "title")` starting at `open` (`text[open] == '('`).
fn parse_destination(text: &str, open: usize) -> Option<Destination<'_>> {
    let bytes = text.as_bytes();
    let mut i = open + 1;
    while i < text.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
        i += 1;
    }
    let url_start: usize;
    let url_end: usize;
    if at(text, i) == b'<' {
        let close = index_of_scalar_pos(text, i + 1, b'>')?;
        url_start = i + 1;
        url_end = close;
        i = close + 1;
    } else {
        url_start = i;
        let mut depth: usize = 0;
        loop {
            if i >= text.len() {
                break;
            }
            let c = bytes[i];
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == b'(' {
                depth += 1;
            }
            if c == b')' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            if c == b' ' || c == b'\n' {
                break;
            }
            i += 1;
        }
        url_end = i.min(text.len());
    }
    while i < text.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
        i += 1;
    }
    let mut title: &str = "";
    if i < text.len() && (bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b'(') {
        let close_char: u8 = if bytes[i] == b'(' { b')' } else { bytes[i] };
        let close = index_of_scalar_pos(text, i + 1, close_char)?;
        title = &text[i + 1..close];
        i = close + 1;
        while i < text.len() && (bytes[i] == b' ' || bytes[i] == b'\n') {
            i += 1;
        }
    }
    if at(text, i) != b')' {
        return None;
    }
    Some(Destination {
        url: &text[url_start..url_end],
        title,
        end: i + 1,
    })
}

fn normalize_label(label: &str) -> String {
    let mut out: Vec<u8> = Vec::new();
    let mut space = false;
    for c in label.trim_matches([' ', '\t', '\n']).bytes() {
        if c == b' ' || c == b'\t' || c == b'\n' {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(b' ');
        }
        space = false;
        out.push(c.to_ascii_lowercase());
    }
    String::from_utf8(out).expect("labels stay valid UTF-8")
}

pub fn lookup_ref(ctx: &Context<'_>, label: &str) -> Option<LinkDef> {
    if ctx.refs.is_empty() {
        return None;
    }
    let key = normalize_label(label);
    ctx.refs.get(&key).cloned()
}

pub fn normalize_ref_label(label: &str) -> String {
    normalize_label(label)
}

/// `[text](url)`, `[text][ref]`, `[ref][]`, `[ref]`, `[^note]`, images.
/// Returns the index after the construct, or `None` when it is not a link.
#[allow(clippy::too_many_arguments)]
pub(super) fn parse_link_like(
    b: &mut Builder<'_, '_>,
    text: &str,
    start: usize,
    open: usize,
    is_image: bool,
    base: Style,
    url: &str,
    plain: usize,
) -> Option<usize> {
    let close = match_bracket(text, open)?;
    let label = &text[open + 1..close];

    // footnote reference [^label]
    if !is_image
        && label.len() > 1
        && label.as_bytes()[0] == b'^'
        && at(text, close + 1) != b':'
        && !label
            .as_bytes()
            .iter()
            .any(|c| *c == b' ' || *c == b'\t' || *c == b'\n')
    {
        b.push_text(&text[plain..start], base, url);
        let name = &label[1..];
        let mut number: usize = 0;
        for (idx, existing) in b.ctx.footnote_order.iter().enumerate() {
            if existing == name {
                number = idx + 1;
            }
        }
        if number == 0 {
            b.ctx.footnote_order.push(name.to_string());
            number = b.ctx.footnote_order.len();
        }
        let mut style = base;
        style.footnote_ref = true;
        style.superscript = true;
        let number_text = number.to_string();
        b.push(&number_text, style, name);
        return Some(close + 1);
    }

    let mut target: Option<LinkDef> = None;
    let mut end = close + 1;
    if at(text, close + 1) == b'('
        && let Some(dest) = parse_destination(text, close + 1)
    {
        target = Some(LinkDef {
            url: dest.url.to_string(),
            title: dest.title.to_string(),
        });
        end = dest.end;
    }
    if target.is_none()
        && at(text, close + 1) == b'['
        && let Some(ref_close) = index_of_scalar_pos(text, close + 2, b']')
    {
        let reference = &text[close + 2..ref_close];
        target = lookup_ref(
            b.ctx,
            if reference.is_empty() {
                label
            } else {
                reference
            },
        );
        if target.is_some() {
            end = ref_close + 1;
        }
    }
    if target.is_none() {
        target = lookup_ref(b.ctx, label);
        end = close + 1;
    }
    let t = target?;

    b.push_text(&text[plain..start], base, url);
    let mut style = base;
    if is_image {
        style.image = true;
        // alt text rendered plainly; renderer decides to load the image
        let alt = if !label.is_empty() { label } else { "image" };
        b.push(alt, style, &t.url);
    } else {
        style.link = true;
        let target_url = if t.url == remend::INCOMPLETE_LINK {
            "#incomplete".to_string()
        } else {
            t.url.clone()
        };
        parse_into(b, label, style, &target_url);
    }
    Some(end)
}
