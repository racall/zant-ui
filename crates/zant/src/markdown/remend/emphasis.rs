//! Port of `src/markdown/remend/emphasis.zig` (remend
//! `emphasis-handlers.ts`): completes `***`, `**`, `__`, `*` and `_` that are
//! still open at the end of a streamed text.

use super::context::Context;
use super::patterns as p;
use super::scan::{Region, Scan};

fn is_ws(cp: u32) -> bool {
    cp == b' ' as u32 || cp == b'\t' as u32 || cp == b'\n' as u32
}

fn byte_at(text: &str, i: usize) -> u8 {
    if i < text.len() {
        text.as_bytes()[i]
    } else {
        0
    }
}

fn should_skip_asterisk(scan: &mut Scan, text: &str, index: usize, prev: u32, next: u32) -> bool {
    if prev == b'\\' as u32 {
        return true;
    }
    if scan.in_math_at(index) {
        return true;
    }
    if prev != b'*' as u32 && next == b'*' as u32 {
        // first * of *** counts as a single asterisk; first of ** is skipped
        return byte_at(text, index + 2) != b'*';
    }
    if prev == b'*' as u32 {
        return true;
    }
    let prev_ws = prev == 0 || is_ws(prev);
    let next_ws = next == 0 || is_ws(next);
    prev_ws && next_ws
}

fn is_word_internal_asterisk(prev: u32, next: u32) -> bool {
    prev != 0 && next != 0 && p::is_word_char(prev) && p::is_word_char(next)
}

pub fn count_single_asterisks(ctx: &mut Context, text: &str) -> usize {
    let scan = ctx.scan(text);
    let mut count = 0usize;
    let mut in_word_chain = false;
    let mut index = 0usize;
    while index < text.len() {
        let c = text.as_bytes()[index];
        if c != b'*' || scan.regions[index] != Region::Prose {
            if (c & 0xC0) == 0x80 {
                index += 1;
                continue;
            }
            if !p::is_word_char(p::char_at(text, index)) {
                in_word_chain = false;
            }
            index += 1;
            continue;
        }
        let prev = p::prev_char(text, index);
        let next = p::char_at(text, index + 1);
        if should_skip_asterisk(scan, text, index, prev, next) {
            index += 1;
            continue;
        }
        let word_internal = is_word_internal_asterisk(prev, next);
        let can_open = next != 0 && !is_ws(next);
        let can_close = prev != 0 && !is_ws(prev);
        if word_internal && count.is_multiple_of(2) && !in_word_chain {
            index += 1;
            continue;
        }
        if (can_close && count % 2 == 1) || can_open {
            count += 1;
            in_word_chain = word_internal;
        }
        index += 1;
    }
    count
}

fn should_skip_underscore(scan: &mut Scan, index: usize, prev: u32, next: u32) -> bool {
    if prev == b'\\' as u32 {
        return true;
    }
    if scan.in_math_at(index) {
        return true;
    }
    if scan.in_link_url_at(index) {
        return true;
    }
    if scan.in_html_tag_at(index) {
        return true;
    }
    if prev == b'_' as u32 || next == b'_' as u32 {
        return true;
    }
    if prev != 0 && next != 0 && p::is_word_char(prev) && p::is_word_char(next) {
        return true;
    }
    false
}

pub fn count_single_underscores(ctx: &mut Context, text: &str) -> usize {
    let scan = ctx.scan(text);
    let mut count = 0usize;
    for (index, c) in text.bytes().enumerate() {
        if c != b'_' || scan.regions[index] != Region::Prose {
            continue;
        }
        if !should_skip_underscore(
            scan,
            index,
            p::prev_char(text, index),
            p::char_at(text, index + 1),
        ) {
            count += 1;
        }
    }
    count
}

pub fn count_triple_asterisks(ctx: &mut Context, text: &str) -> usize {
    let scan = ctx.scan(text);
    let mut count = 0usize;
    let mut run = 0usize;
    for (i, c) in text.bytes().enumerate() {
        if c == b'*' && scan.regions[i] == Region::Prose {
            run += 1;
        } else {
            if run >= 3 {
                count += run / 3;
            }
            run = 0;
        }
    }
    if run >= 3 {
        count += run / 3;
    }
    count
}

fn count_double_asterisks(ctx: &mut Context, text: &str) -> usize {
    ctx.scan(text).count_double_pairs(b'*')
}

fn is_line_boundary_char(cp: u32) -> bool {
    cp == 0 || cp == b' ' as u32 || cp == b'\t' as u32 || cp == b'\n' as u32
}

#[derive(Default)]
struct ThematicBreakMemo {
    line_end: isize,
    result: bool,
}

fn is_thematic_break_run(
    text: &str,
    run_start: usize,
    prev: u32,
    next: u32,
    memo: &mut ThematicBreakMemo,
) -> bool {
    if !(is_line_boundary_char(prev) && is_line_boundary_char(next)) {
        return false;
    }
    if run_start as isize > memo.line_end {
        let line_end = text[run_start..]
            .find('\n')
            .map(|i| i + run_start)
            .unwrap_or(text.len());
        memo.line_end = line_end as isize;
        memo.result = p::is_horizontal_rule(text, run_start, b'_');
    }
    memo.result
}

fn double_underscore_run_flips(
    scan: &mut Scan,
    text: &str,
    initial_start: usize,
    run_end: usize,
    memo: &mut ThematicBreakMemo,
) -> bool {
    let mut run_start = initial_start;
    let mut escaped = false;
    if run_start > 0 && text.as_bytes()[run_start - 1] == b'\\' {
        run_start += 1;
        escaped = true;
    }
    if run_end < run_start || run_end - run_start < 2 {
        return false;
    }
    let run_length = run_end - run_start;
    let prev = if escaped {
        b'\\' as u32
    } else {
        p::prev_char(text, run_start)
    };
    let next = p::char_at(text, run_end);
    if p::is_word_char(prev) && p::is_word_char(next) {
        return false;
    }
    if is_thematic_break_run(text, run_start, prev, next, memo) {
        return false;
    }
    if scan.in_math_at(run_start)
        || scan.in_link_url_at(run_start)
        || scan.in_html_tag_at(run_start)
    {
        return false;
    }
    (run_length / 2) % 2 == 1
}

fn has_unmatched_double_underscore(ctx: &mut Context, text: &str) -> bool {
    let scan = ctx.scan(text);
    let mut memo = ThematicBreakMemo::default();
    let mut unmatched = false;
    let mut i = 0usize;
    while i < text.len() {
        if text.as_bytes()[i] != b'_' || scan.regions[i] != Region::Prose {
            i += 1;
            continue;
        }
        let run_start = i;
        let mut run_end = i + 1;
        while run_end < text.len()
            && text.as_bytes()[run_end] == b'_'
            && scan.regions[run_end] == Region::Prose
        {
            run_end += 1;
        }
        i = run_end;
        if double_underscore_run_flips(scan, text, run_start, run_end, &mut memo) {
            unmatched = !unmatched;
        }
    }
    unmatched
}

fn line_before(text: &str, marker_index: usize) -> &str {
    let before = &text[..marker_index];
    let line_start = if let Some(nl) = before.rfind('\n') {
        nl + 1
    } else {
        0
    };
    &text[line_start..marker_index]
}

fn should_skip_completion(text: &str, content: &str, marker_index: usize, marker: u8) -> bool {
    if content.is_empty() || p::is_whitespace_or_markers(content) {
        return true;
    }
    if p::is_list_item_prefix(line_before(text, marker_index)) && content.contains('\n') {
        return true;
    }
    p::is_horizontal_rule(text, marker_index, marker)
}

/// Completes incomplete bold formatting (**)
pub fn handle_incomplete_bold(ctx: &mut Context, text: &str) -> String {
    let Some(m) = p::match_trailing(text, p::BOLD) else {
        return text.to_owned();
    };
    let Some(marker_index) = p::last_index_of(text, m.marker) else {
        return text.to_owned();
    };
    if ctx.is_inside_code_block(text, marker_index)
        || ctx.is_within_complete_inline_code(text, marker_index)
    {
        return text.to_owned();
    }
    if should_skip_completion(text, m.content, marker_index, b'*') {
        return text.to_owned();
    }
    if count_double_asterisks(ctx, text) % 2 == 1 {
        if !m.content.is_empty() && m.content.as_bytes()[m.content.len() - 1] == b'*' {
            return ctx.concat(&[text, "*"]);
        }
        return ctx.concat(&[text, "**"]);
    }
    text.to_owned()
}

/// Completes incomplete italic formatting with double underscores (__)
pub fn handle_incomplete_double_underscore_italic(ctx: &mut Context, text: &str) -> String {
    let Some(m) = p::match_trailing(text, p::ITALIC) else {
        if let Some(half) = p::match_trailing(text, p::HALF_COMPLETE_UNDERSCORE) {
            let Some(marker_index) = p::last_index_of(text, half.marker) else {
                return text.to_owned();
            };
            if !(ctx.is_inside_code_block(text, marker_index)
                || ctx.is_within_complete_inline_code(text, marker_index))
                && has_unmatched_double_underscore(ctx, text)
            {
                return ctx.concat(&[text, "_"]);
            }
        }
        return text.to_owned();
    };
    let Some(marker_index) = p::last_index_of(text, m.marker) else {
        return text.to_owned();
    };
    if ctx.is_inside_code_block(text, marker_index)
        || ctx.is_within_complete_inline_code(text, marker_index)
    {
        return text.to_owned();
    }
    if should_skip_completion(text, m.content, marker_index, b'_') {
        return text.to_owned();
    }
    if has_unmatched_double_underscore(ctx, text) {
        return ctx.concat(&[text, "__"]);
    }
    text.to_owned()
}

fn is_lone_prose_asterisk(scan: &mut Scan, text: &str, i: usize) -> bool {
    text.as_bytes()[i] == b'*'
        && scan.regions[i] == Region::Prose
        && (i == 0 || text.as_bytes()[i - 1] != b'*')
        && byte_at(text, i + 1) != b'*'
        && (i == 0 || text.as_bytes()[i - 1] != b'\\')
        && !scan.in_math_at(i)
}

fn find_first_single_asterisk_index(ctx: &mut Context, text: &str) -> Option<usize> {
    let scan = ctx.scan(text);
    let mut from = 0usize;
    loop {
        let i = text[from..].find('*').map(|x| x + from)?;
        from = i + 1;
        if !is_lone_prose_asterisk(scan, text, i) {
            continue;
        }
        let prev = p::prev_char(text, i);
        let next = p::char_at(text, i + 1);
        let prev_ws = prev == 0 || is_ws(prev);
        let next_ws = next == 0 || is_ws(next);
        if prev_ws && next_ws {
            continue;
        }
        if prev != 0 && next != 0 && p::is_word_char(prev) && p::is_word_char(next) {
            continue;
        }
        if next_ws {
            continue;
        }
        return Some(i);
    }
}

/// Completes incomplete italic formatting with single asterisks (*)
pub fn handle_incomplete_single_asterisk_italic(ctx: &mut Context, text: &str) -> String {
    if p::match_trailing(text, p::SINGLE_ASTERISK).is_none() {
        return text.to_owned();
    }
    let Some(first) = find_first_single_asterisk_index(ctx, text) else {
        return text.to_owned();
    };
    if ctx.is_inside_code_block(text, first) || ctx.is_within_complete_inline_code(text, first) {
        return text.to_owned();
    }
    let after = &text[first + 1..];
    if after.is_empty() || p::is_whitespace_or_markers(after) {
        return text.to_owned();
    }
    if count_single_asterisks(ctx, text) % 2 == 1 {
        return ctx.concat(&[text, "*"]);
    }
    text.to_owned()
}

fn find_first_single_underscore_index(ctx: &mut Context, text: &str) -> Option<usize> {
    let scan = ctx.scan(text);
    let mut from = 0usize;
    loop {
        let i = text[from..].find('_').map(|x| x + from)?;
        from = i + 1;
        if scan.regions[i] == Region::Prose
            && (i == 0 || text.as_bytes()[i - 1] != b'_')
            && byte_at(text, i + 1) != b'_'
            && (i == 0 || text.as_bytes()[i - 1] != b'\\')
            && !scan.in_math_at(i)
            && !scan.in_link_url_at(i)
        {
            let prev = p::prev_char(text, i);
            let next = p::char_at(text, i + 1);
            if prev != 0 && next != 0 && p::is_word_char(prev) && p::is_word_char(next) {
                continue;
            }
            return Some(i);
        }
    }
}

fn insert_closing_underscore(ctx: &mut Context, text: &str) -> String {
    let mut end = text.len();
    while end > 0 && text.as_bytes()[end - 1] == b'\n' {
        end -= 1;
    }
    if end < text.len() {
        return ctx.concat(&[&text[..end], "_", &text[end..]]);
    }
    ctx.concat(&[text, "_"])
}

fn handle_trailing_asterisks_for_underscore(ctx: &mut Context, text: &str) -> Option<String> {
    if !text.ends_with("**") {
        return None;
    }
    let without = &text[..text.len() - 2];
    if count_double_asterisks(ctx, without) % 2 != 1 {
        return None;
    }
    let first_double = without.find("**");
    let underscore = find_first_single_underscore_index(ctx, without);
    match (first_double, underscore) {
        (Some(first_double), Some(underscore)) if first_double < underscore => {
            Some(ctx.concat(&[without, "_**"]))
        }
        _ => None,
    }
}

/// Completes incomplete italic formatting with single underscores (_)
pub fn handle_incomplete_single_underscore_italic(ctx: &mut Context, text: &str) -> String {
    if p::match_trailing(text, p::SINGLE_UNDERSCORE).is_none() {
        return text.to_owned();
    }
    let Some(first) = find_first_single_underscore_index(ctx, text) else {
        return text.to_owned();
    };
    let after = &text[first + 1..];
    if after.is_empty() || p::is_whitespace_or_markers(after) {
        return text.to_owned();
    }
    if ctx.is_inside_code_block(text, first) || ctx.is_within_complete_inline_code(text, first) {
        return text.to_owned();
    }
    if count_single_underscores(ctx, text) % 2 == 1 {
        if let Some(result) = handle_trailing_asterisks_for_underscore(ctx, text) {
            return result;
        }
        return insert_closing_underscore(ctx, text);
    }
    text.to_owned()
}

fn are_bold_italic_markers_balanced(ctx: &mut Context, text: &str) -> bool {
    count_double_asterisks(ctx, text).is_multiple_of(2)
        && count_single_asterisks(ctx, text).is_multiple_of(2)
}

/// Completes incomplete bold-italic formatting (***)
pub fn handle_incomplete_bold_italic(ctx: &mut Context, text: &str) -> String {
    if p::is_four_or_more_asterisks(text) {
        return text.to_owned();
    }
    let Some(m) = p::match_trailing(text, p::BOLD_ITALIC) else {
        return text.to_owned();
    };
    let Some(marker_index) = p::last_index_of(text, m.marker) else {
        return text.to_owned();
    };
    if m.content.is_empty() || p::is_whitespace_or_markers(m.content) {
        return text.to_owned();
    }
    if ctx.is_inside_code_block(text, marker_index)
        || ctx.is_within_complete_inline_code(text, marker_index)
    {
        return text.to_owned();
    }
    if p::is_horizontal_rule(text, marker_index, b'*') {
        return text.to_owned();
    }
    if count_triple_asterisks(ctx, text) % 2 == 1 {
        if are_bold_italic_markers_balanced(ctx, text) {
            return text.to_owned();
        }
        return ctx.concat(&[text, "***"]);
    }
    text.to_owned()
}
