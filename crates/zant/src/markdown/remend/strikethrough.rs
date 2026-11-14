//! remend `strikethrough-handler.ts`: completes `~~` that is still open at
//! the end of a streamed text.

use super::context::Context;
use super::patterns as p;

pub fn handle_incomplete_strikethrough(ctx: &mut Context, text: &str) -> String {
    if let Some(m) = p::match_trailing(text, p::STRIKETHROUGH) {
        if m.content.is_empty() || p::is_whitespace_or_markers(m.content) {
            return text.to_owned();
        }
        let Some(marker_index) = p::last_index_of(text, m.marker) else {
            return text.to_owned();
        };
        if ctx.is_inside_code_block(text, marker_index)
            || ctx.is_within_complete_inline_code(text, marker_index)
        {
            return text.to_owned();
        }
        if ctx.scan(text).count_double_pairs(b'~') % 2 == 1 {
            return ctx.concat(&[text, "~~"]);
        }
    } else if let Some(half) = p::match_trailing(text, p::HALF_COMPLETE_TILDE) {
        let Some(marker_index) = p::last_index_of(text, &half.whole[0..2]) else {
            return text.to_owned();
        };
        if ctx.is_inside_code_block(text, marker_index)
            || ctx.is_within_complete_inline_code(text, marker_index)
        {
            return text.to_owned();
        }
        if ctx.scan(text).count_double_pairs(b'~') % 2 == 1 {
            return ctx.concat(&[text, "~"]);
        }
    }
    text.to_owned()
}
