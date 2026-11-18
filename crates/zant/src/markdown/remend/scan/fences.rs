//! Painting fenced code blocks and finding an unclosed fence.

use super::*;

pub(super) fn is_fence_closer(
    text: &str,
    content_start: usize,
    line_end: usize,
    fence: OpenFence,
) -> bool {
    let b = text.as_bytes();
    let mut i = content_start;
    while i < line_end && b[i] == b' ' {
        i += 1;
    }
    let mut run = 0usize;
    while i < line_end && b[i] == fence.char {
        i += 1;
        run += 1;
    }
    if run < fence.length {
        return false;
    }
    while i < line_end {
        if b[i] != b' ' && b[i] != b'\t' && b[i] != b'\r' {
            return false;
        }
        i += 1;
    }
    true
}

pub(super) fn open_fence_at(
    text: &str,
    regions: &mut [Region],
    line_start: usize,
    line_end: usize,
) -> Option<OpenFence> {
    let b = text.as_bytes();
    let content_end = if line_end > line_start && b[line_end - 1] == b'\r' {
        line_end - 1
    } else {
        line_end
    };
    let prefix = skip_container_prefix(text, line_start, content_end);
    let line = &b[prefix.content_start.min(content_end)..content_end];
    // /^(`{3,}|~{3,})(.*)$/
    if line.len() < 3 || (line[0] != b'`' && line[0] != b'~') {
        return None;
    }
    let ch = line[0];
    let mut run = 0usize;
    while run < line.len() && line[run] == ch {
        run += 1;
    }
    if run < 3 {
        return None;
    }
    let info = &line[run..];
    if ch == b'`' && info.contains(&b'`') {
        return None;
    }
    let marker_start = prefix.content_start;
    fill(
        regions,
        Region::FenceMarker,
        marker_start,
        marker_start + run,
    );
    fill(regions, Region::FenceInfo, marker_start + run, line_end + 1);
    Some(OpenFence {
        char: ch,
        length: run,
        list_indent: prefix.list_indent,
        quote_depth: prefix.quote_depth,
    })
}

pub(super) fn fence_content_start(
    text: &str,
    line_start: usize,
    line_end: usize,
    fence: OpenFence,
) -> Option<usize> {
    let b = text.as_bytes();
    let content_start = skip_quote_markers(text, line_start, line_end, fence.quote_depth)?;
    if fence.list_indent == 0 {
        return Some(content_start);
    }
    let mut i = content_start;
    while i < line_end && b[i] == b' ' {
        i += 1;
    }
    let blank = i == line_end || (b[i] == b'\r' && i + 1 == line_end);
    if blank || i - content_start >= fence.list_indent {
        Some(content_start)
    } else {
        None
    }
}

pub(super) fn paint_fences(text: &str, regions: &mut [Region]) -> Option<OpenFence> {
    let n = text.len();
    let mut open_fence: Option<OpenFence> = None;
    let mut line_start = 0usize;
    while line_start < n {
        let line_end = text[line_start..]
            .find('\n')
            .map(|i| i + line_start)
            .unwrap_or(n);
        let mut content_start: Option<usize> = None;
        if let Some(fence) = open_fence {
            content_start = fence_content_start(text, line_start, line_end, fence);
            if content_start.is_none() {
                open_fence = None;
            }
        }
        if open_fence.is_none() {
            open_fence = open_fence_at(text, regions, line_start, line_end);
        } else if is_fence_closer(
            text,
            content_start.expect("content start while a fence is open"),
            line_end,
            open_fence.expect("open fence"),
        ) {
            fill(regions, Region::FenceMarker, line_start, line_end);
            open_fence = None;
        } else {
            fill(
                regions,
                Region::FenceBody,
                line_start,
                (line_end + 1).min(n),
            );
        }
        line_start = line_end + 1;
    }
    open_fence
}
