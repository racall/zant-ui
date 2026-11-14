//! remend `inline-code-handler.ts`: completes inline code that is still open
//! at the end of a streamed text.

use super::context::Context;
use super::patterns as p;

pub fn handle_incomplete_inline_code(ctx: &mut Context, text: &str) -> String {
    let (open_fence, span) = {
        let scan = ctx.scan(text);
        (scan.open_fence, scan.open_span)
    };
    if open_fence.is_some() {
        return text.to_owned();
    }
    let Some(span) = span else {
        return text.to_owned();
    };
    let content = &text[span.start + span.run_length..];
    if content.is_empty() || p::is_whitespace_or_markers(content) {
        return text.to_owned();
    }
    let mut trailing = 0usize;
    let mut i = text.len();
    let b = text.as_bytes();
    while i > 0 && b[i - 1] == b'`' {
        i -= 1;
        trailing += 1;
    }
    if trailing >= span.run_length {
        return text.to_owned();
    }
    let closer = "`".repeat(span.run_length - trailing);
    ctx.concat(&[text, &closer])
}
