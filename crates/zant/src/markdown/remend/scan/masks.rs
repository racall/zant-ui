//! Masks of math, link URLs and HTML tags, where emphasis markers do not count.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MathContext {
    None,
    InlineDollar,
    BlockDollar,
    InlineLatex,
    BlockLatex,
}

pub(super) fn build_math_mask(scan: &Scan) -> Vec<bool> {
    let text = scan.text.as_bytes();
    let mut mask = vec![false; text.len()];
    let mut context = MathContext::None;
    let mut i = 0usize;
    while i < text.len() {
        mask[i] = context != MathContext::None;
        if scan.regions[i] != Region::Prose {
            i += 1;
            continue;
        }
        let next = at(&scan.text, i + 1);
        let mut new_context: Option<MathContext> = None;
        let mut length = 1usize;
        if text[i] == b'\\' {
            if next == b'$' {
                new_context = Some(context);
                length = 2;
            } else {
                let latex: Option<MathContext> = if next == b'[' && context == MathContext::None {
                    Some(MathContext::BlockLatex)
                } else if next == b']' && context == MathContext::BlockLatex {
                    Some(MathContext::None)
                } else if next == b'(' && context == MathContext::None {
                    Some(MathContext::InlineLatex)
                } else if next == b')' && context == MathContext::InlineLatex {
                    Some(MathContext::None)
                } else {
                    None
                };
                if let Some(value) = latex {
                    new_context = Some(value);
                    length = 2;
                }
            }
        } else if text[i] == b'$'
            && context != MathContext::InlineLatex
            && context != MathContext::BlockLatex
        {
            let is_block = next == b'$';
            new_context = Some(if is_block {
                if context == MathContext::BlockDollar {
                    MathContext::None
                } else {
                    MathContext::BlockDollar
                }
            } else if context == MathContext::BlockDollar {
                context
            } else if context == MathContext::InlineDollar {
                MathContext::None
            } else {
                MathContext::InlineDollar
            });
            length = if is_block { 2 } else { 1 };
        }
        let Some(value) = new_context else {
            i += 1;
            continue;
        };
        context = value;
        if length == 2 && i + 1 < text.len() {
            mask[i + 1] = context != MathContext::None;
        }
        i += length;
    }
    mask
}

pub(super) fn build_link_url_mask(scan: &Scan) -> Vec<bool> {
    let text = scan.text.as_bytes();
    let mut mask = vec![false; text.len()];
    let mut line_start = 0usize;
    while line_start < text.len() {
        let line_end = scan.text[line_start..]
            .find('\n')
            .map(|i| i + line_start)
            .unwrap_or(text.len());
        // closerFollows: a ")" exists at or after i on this line
        let mut closer_after: Option<usize> = None;
        let mut follows = vec![false; line_end - line_start];
        let mut i = line_end;
        while i > line_start {
            i -= 1;
            if text[i] == b')' && scan.regions[i] == Region::Prose {
                closer_after = Some(i);
            }
            follows[i - line_start] = closer_after.is_some();
        }
        let mut in_url = false;
        let mut i = line_start;
        while i < line_end {
            if in_url && follows[i - line_start] {
                mask[i] = true;
            }
            if scan.regions[i] != Region::Prose {
                i += 1;
                continue;
            }
            if text[i] == b')' {
                in_url = false;
            } else if text[i] == b'(' {
                in_url = i > 0 && text[i - 1] == b']';
            }
            i += 1;
        }
        line_start = line_end + 1;
    }
    mask
}

pub(super) fn build_html_tag_mask(scan: &Scan) -> Vec<bool> {
    let text = scan.text.as_bytes();
    let mut mask = vec![false; text.len()];
    let mut in_tag = false;
    for (i, &c) in text.iter().enumerate() {
        if c == b'\n' {
            in_tag = false;
            continue;
        }
        mask[i] = in_tag;
        if scan.regions[i] != Region::Prose {
            continue;
        }
        if c == b'>' {
            in_tag = false;
        } else if c == b'<' {
            let next = at(&scan.text, i + 1);
            in_tag = next.is_ascii_alphabetic() || next == b'/';
        }
    }
    mask
}
