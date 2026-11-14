//! Port of `src/markdown/remend/setext_heading.zig` (remend
//! `setext-heading-handler.ts`): a trailing `-`/`--`/`=`/`==` under a text line
//! would turn that line into a heading mid-stream; append a zero-width space to
//! break the pattern.

use super::context::Context;

fn only_run(line: &str, c: u8) -> bool {
    let t = line.trim_matches(|ch: char| matches!(ch, ' ' | '\t' | '\r' | '\n'));
    if t.is_empty() || t.len() > 2 {
        return false;
    }
    for &x in t.as_bytes() {
        if x != c {
            return false;
        }
    }
    true
}

/// `/^[\s]*-{1,2}[\s]+$/` (a dash run followed by whitespace)
fn run_with_trailing_space(line: &str, c: u8) -> bool {
    let b = line.as_bytes();
    let mut i = 0usize;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    while i < b.len() && b[i] == c {
        i += 1;
    }
    let run = i - start;
    if !(1..=2).contains(&run) {
        return false;
    }
    if i >= b.len() {
        return false;
    }
    while i < b.len() {
        if !b[i].is_ascii_whitespace() {
            return false;
        }
        i += 1;
    }
    true
}

pub fn handle_incomplete_setext_heading(ctx: &mut Context, text: &str) -> String {
    let Some(nl) = text.rfind('\n') else {
        return text.to_owned();
    };
    let last_line = &text[nl + 1..];
    let previous = &text[..nl];
    let prev_line = if let Some(j) = previous.rfind('\n') {
        &previous[j + 1..]
    } else {
        previous
    };
    let prev_has_text = !prev_line
        .trim_matches(|ch: char| matches!(ch, ' ' | '\t' | '\r' | '\n'))
        .is_empty();
    for c in [b'-', b'='] {
        if only_run(last_line, c) && !run_with_trailing_space(last_line, c) && prev_has_text {
            return ctx.concat(&[text, "\u{200B}"]);
        }
    }
    text.to_owned()
}
