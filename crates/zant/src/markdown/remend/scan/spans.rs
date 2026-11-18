//! Painting code spans and finding an unclosed one.

use super::*;

pub(super) fn measure_backtick_run(text: &str, start: usize) -> usize {
    let b = text.as_bytes();
    let mut end = start + 1;
    while end < b.len() && b[end] == b'`' {
        end += 1;
    }
    end
}

pub(super) fn is_paragraph_break_at(text: &str, newline_index: usize) -> bool {
    let b = text.as_bytes();
    let mut j = newline_index + 1;
    while j < b.len() && (b[j] == b' ' || b[j] == b'\t' || b[j] == b'\r') {
        j += 1;
    }
    j < b.len() && b[j] == b'\n'
}

pub(super) fn next_span_opener(text: &str, regions: &[Region], from: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut i = from;
    loop {
        let next = text[i..].find('`').map(|x| x + i)?;
        let escaped = next > i && b[next - 1] == b'\\' && regions[next - 1] == Region::Prose;
        if regions[next] == Region::Prose && !escaped {
            return Some(next);
        }
        i = next + 1;
    }
}

pub(super) fn paint_spans(text: &str, regions: &mut [Region]) -> Option<OpenSpan> {
    let n = text.len();
    let b = text.as_bytes();
    let mut span_start: Option<usize> = None;
    let mut span_run = 0usize;
    let mut i = 0usize;
    while i < n {
        if span_start.is_none() {
            let Some(opener) = next_span_opener(text, regions, i) else {
                break;
            };
            let opener_end = measure_backtick_run(text, opener);
            span_start = Some(opener);
            span_run = opener_end - opener;
            i = opener_end;
            continue;
        }
        if regions[i] != Region::Prose {
            fill(
                regions,
                Region::CodeSpanOpen,
                span_start.expect("span start"),
                i,
            );
            span_start = None;
            i += 1;
            continue;
        }
        if b[i] == b'\n' && is_paragraph_break_at(text, i) {
            span_start = None;
            i += 1;
            continue;
        }
        if b[i] != b'`' {
            i += 1;
            continue;
        }
        let run_end = measure_backtick_run(text, i);
        if run_end - i == span_run {
            fill(
                regions,
                Region::CodeSpan,
                span_start.expect("span start"),
                run_end,
            );
            span_start = None;
        }
        i = run_end;
    }
    if let Some(start) = span_start {
        fill(regions, Region::CodeSpanOpen, start, n);
        return Some(OpenSpan {
            run_length: span_run,
            start,
        });
    }
    None
}
