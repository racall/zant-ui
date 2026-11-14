//! Port of `src/markdown/remend/comparison_operator.zig` (remend
//! `comparison-operator-handler.ts`): `- >5` in a list item would start a block
//! quote; escape the `>`.

use super::context::Context;

/// Matches `^(\s*(?:[-*+]|\d+[.)]) +)>(=?\s*[$]?\d)` at `line_start`; returns
/// the index of the `>` when it matches.
fn match_at(text: &str, line_start: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = line_start;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i >= b.len() {
        return None;
    }
    if b[i] == b'-' || b[i] == b'*' || b[i] == b'+' {
        i += 1;
    } else if b[i].is_ascii_digit() {
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'.' && b[i] != b')') {
            return None;
        }
        i += 1;
    } else {
        return None;
    }
    if i >= b.len() || b[i] != b' ' {
        return None;
    }
    while i < b.len() && b[i] == b' ' {
        i += 1;
    }
    if i >= b.len() || b[i] != b'>' {
        return None;
    }
    let gt = i;
    i += 1;
    if i < b.len() && b[i] == b'=' {
        i += 1;
    }
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i < b.len() && b[i] == b'$' {
        i += 1;
    }
    if i >= b.len() || !b[i].is_ascii_digit() {
        return None;
    }
    Some(gt)
}

pub fn handle_comparison_operators(ctx: &mut Context, text: &str) -> String {
    if !text.contains('>') {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut last = 0usize;
    let mut changed = false;
    let mut line_start = 0usize;
    while line_start <= text.len() {
        if let Some(gt) = match_at(text, line_start)
            && !ctx.is_inside_code_block(text, line_start)
        {
            out.push_str(&text[last..gt]);
            out.push_str("\\>");
            last = gt + 1;
            changed = true;
        }
        let Some(nl) = text[line_start..].find('\n').map(|i| i + line_start) else {
            break;
        };
        line_start = nl + 1;
    }
    if !changed {
        return text.to_owned();
    }
    out.push_str(&text[last..]);
    out
}
