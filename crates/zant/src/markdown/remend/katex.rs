//! Port of `src/markdown/remend/katex.zig` (remend `katex-handler.ts`): closes
//! `$$` blocks (and, opt-in, single-dollar inline math).

use super::context::Context;
use super::scan::Region;

fn count_single_dollars(ctx: &mut Context, text: &str) -> usize {
    let scan = ctx.scan(text);
    let b = text.as_bytes();
    let mut count = 0usize;
    let mut i = 0usize;
    while i < text.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if scan.regions[i] != Region::Prose {
            i += 1;
            continue;
        }
        if b[i] == b'$' {
            if i + 1 < text.len() && b[i + 1] == b'$' {
                i += 2;
            } else {
                count += 1;
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    count
}

fn add_closing_katex(ctx: &mut Context, text: &str) -> String {
    if text.ends_with('$') && !text.ends_with("$$") {
        return ctx.concat(&[text, "$"]);
    }
    let first = text.find("$$");
    let newline_after = first.is_some_and(|idx| text[idx..].contains('\n'));
    if newline_after && !text.ends_with('\n') {
        return ctx.concat(&[text, "\n$$"]);
    }
    ctx.concat(&[text, "$$"])
}

pub fn handle_incomplete_block_katex(ctx: &mut Context, text: &str) -> String {
    if ctx.scan(text).count_double_pairs(b'$').is_multiple_of(2) {
        return text.to_owned();
    }
    add_closing_katex(ctx, text)
}

pub fn handle_incomplete_inline_katex(ctx: &mut Context, text: &str) -> String {
    if count_single_dollars(ctx, text) % 2 == 1 {
        return ctx.concat(&[text, "$"]);
    }
    text.to_owned()
}
