//! Port of `src/markdown/remend/single_tilde.zig` (remend
//! `single-tilde-handler.ts`): escapes a single `~` between word characters
//! (`20~25`) so it is not read as strikethrough.

use super::context::Context;
use super::patterns as p;

fn utf8_codepoint_sequence_length(cp: u32) -> Option<usize> {
    if cp < 0x80 {
        Some(1)
    } else if cp < 0x800 {
        Some(2)
    } else if cp < 0x10000 {
        Some(3)
    } else if cp <= 0x10FFFF {
        Some(4)
    } else {
        None
    }
}

/// `text.replace(/([\p{L}\p{N}_])~(?!~)(?=[\p{L}\p{N}_])/gu, ...)`
pub fn handle_single_tilde_escape(ctx: &mut Context, text: &str) -> String {
    if !text.contains('~') {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut copied = 0usize; // bytes of `text` already written to `out`
    let mut regex_pos = 0usize; // where the global regex resumes
    let mut from = 0usize;
    while let Some(t) = text[from..].find('~').map(|i| i + from) {
        from = t + 1;
        if t == 0 {
            continue;
        }
        let prev = p::prev_char(text, t);
        let prev_len = utf8_codepoint_sequence_length(prev).unwrap_or(1);
        if t < prev_len || t - prev_len < regex_pos {
            continue;
        }
        if !p::is_word_char(prev) {
            continue;
        }
        let next = p::char_at(text, t + 1);
        if next == b'~' as u32 || !p::is_word_char(next) {
            continue;
        }
        regex_pos = t + 1;
        if ctx.is_inside_code_block(text, t) {
            continue;
        }
        out.push_str(&text[copied..t]);
        out.push_str("\\~");
        copied = t + 1;
    }
    if copied == 0 {
        return text.to_owned();
    }
    out.push_str(&text[copied..]);
    out
}
