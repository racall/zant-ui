//! Port of `src/markdown/remend/link_image.zig` (remend
//! `link-image-handler.ts`): heals `[text`, `[text](partial` and `![alt` at the
//! end of a streamed text.

use super::context::Context;
use super::patterns as p;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkMode {
    Protocol,
    TextOnly,
}

/// Sentinel URL for links whose target has not streamed in yet.
pub const INCOMPLETE_LINK: &str = "streamdown:incomplete-link";
/// Sentinel source for images whose source has not streamed in yet.
pub const INCOMPLETE_IMAGE: &str = "streamdown:incomplete-image";

/// Brackets inside `$…$`, `$$…$$`, `\(…\)` and `\[…\]` (including a formula that is
/// still streaming) are math such as `[a,b]`, not links. Upstream remend only
/// checks code; healing them appends the sentinel URL to the formula.
fn in_math(ctx: &mut Context, text: &str, position: usize) -> bool {
    ctx.scan(text).in_math_at(position)
}

fn handle_incomplete_url(
    ctx: &mut Context,
    text: &str,
    last_paren: usize,
    mode: LinkMode,
) -> Option<String> {
    let after = &text[last_paren + 2..];
    if after.contains(')') {
        return None;
    }
    let open = p::find_matching_opening_bracket(text, last_paren)?;
    if ctx.is_inside_code_block(text, open) || in_math(ctx, text, open) {
        return None;
    }
    let is_image = open > 0 && text.as_bytes()[open - 1] == b'!';
    let start = if is_image { open - 1 } else { open };
    let before = &text[..start];
    let label = &text[open + 1..last_paren];
    if is_image {
        return Some(ctx.concat(&[before, "![", label, "](", INCOMPLETE_IMAGE, ")"]));
    }
    if mode == LinkMode::TextOnly {
        return Some(ctx.concat(&[before, label]));
    }
    Some(ctx.concat(&[before, "[", label, "](", INCOMPLETE_LINK, ")"]))
}

fn find_first_incomplete_bracket(ctx: &mut Context, text: &str, max_pos: usize) -> usize {
    let b = text.as_bytes();
    let mut j = 0usize;
    while j < max_pos {
        if b[j] == b'[' && !ctx.is_inside_code_block(text, j) && !in_math(ctx, text, j) {
            if j > 0 && b[j - 1] == b'!' {
                j += 1;
                continue;
            }
            let Some(closing) = p::find_matching_closing_bracket(text, j) else {
                return j;
            };
            let mut next = j + 1;
            if closing + 1 < text.len()
                && b[closing + 1] == b'('
                && let Some(url_end) = text[closing + 2..].find(')').map(|x| x + closing + 2)
            {
                next = url_end + 1;
            }
            j = next;
            continue;
        }
        j += 1;
    }
    max_pos
}

fn handle_incomplete_text(
    ctx: &mut Context,
    text: &str,
    i: usize,
    mode: LinkMode,
) -> Option<String> {
    let is_image = i > 0 && text.as_bytes()[i - 1] == b'!';
    let open = if is_image { i - 1 } else { i };
    let has_close = text[i + 1..].contains(']');
    let closing = if has_close {
        p::find_matching_closing_bracket(text, i)
    } else {
        None
    };
    if closing.is_some() {
        return None;
    }
    let before = &text[..open];
    if is_image {
        return Some(ctx.concat(&[before, "![", &text[i + 1..], "](", INCOMPLETE_IMAGE, ")"]));
    }
    if mode == LinkMode::TextOnly {
        let first = find_first_incomplete_bracket(ctx, text, i);
        return Some(ctx.concat(&[&text[..first], &text[first + 1..]]));
    }
    Some(ctx.concat(&[text, "](", INCOMPLETE_LINK, ")"]))
}

fn heal_trailing_link_or_image(ctx: &mut Context, text: &str, mode: LinkMode) -> String {
    if let Some(last_paren) = p::last_index_of(text, "](")
        && !ctx.is_inside_code_block(text, last_paren)
        && !in_math(ctx, text, last_paren)
        && let Some(result) = handle_incomplete_url(ctx, text, last_paren, mode)
    {
        return result;
    }
    let mut search_end = text.len();
    while let Some(i) = text[..search_end].rfind('[') {
        if !ctx.is_inside_code_block(text, i)
            && !in_math(ctx, text, i)
            && let Some(result) = handle_incomplete_text(ctx, text, i, mode)
        {
            return result;
        }
        if i == 0 {
            break;
        }
        search_end = i;
    }
    text.to_owned()
}

const MAX_HEAL_PASSES: usize = 32;

pub fn handle_incomplete_links_and_images(ctx: &mut Context, text: &str, mode: LinkMode) -> String {
    let mut current = text.to_owned();
    let mut pass = 0usize;
    while pass < MAX_HEAL_PASSES {
        let next = heal_trailing_link_or_image(ctx, &current, mode);
        if next.len() >= current.len() {
            return next;
        }
        current = next;
        pass += 1;
    }
    current
}
