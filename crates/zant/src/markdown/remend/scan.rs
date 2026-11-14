//! Port of `src/markdown/remend/scan.zig`: single-pass classification of a text
//! into code and prose regions, plus lazy masks for math, link URLs and HTML tags.
//!
//! Positions are UTF-8 byte offsets. Every delimiter remend looks at is ASCII,
//! so byte positions behave exactly like the UTF-16 positions of the original.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Region {
    #[default]
    Prose,
    /// The ``` or ~~~ run that opens or closes a fence
    FenceMarker,
    /// The info string on a fence opener line
    FenceInfo,
    FenceBody,
    /// A complete inline code span, including its backtick markers
    CodeSpan,
    /// An inline code span whose closing run has not arrived yet
    CodeSpanOpen,
}

#[derive(Clone, Copy, Debug)]
pub struct OpenFence {
    pub char: u8,
    pub length: usize,
    pub list_indent: usize,
    pub quote_depth: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct OpenSpan {
    pub run_length: usize,
    pub start: usize,
}

#[derive(Debug)]
pub struct Scan {
    pub text: String,
    pub regions: Vec<Region>,
    pub open_fence: Option<OpenFence>,
    pub open_span: Option<OpenSpan>,
    pub math_mask: Option<Vec<bool>>,
    pub link_url_mask: Option<Vec<bool>>,
    pub html_tag_mask: Option<Vec<bool>>,
}

impl Scan {
    pub fn new(text: &str) -> Scan {
        let mut regions = vec![Region::Prose; text.len()];
        let open_fence = paint_fences(text, &mut regions);
        let open_span = paint_spans(text, &mut regions);
        Scan {
            text: text.to_owned(),
            regions,
            open_fence,
            open_span,
            math_mask: None,
            link_url_mask: None,
            html_tag_mask: None,
        }
    }

    /// A code construct here means a fence or inline span.
    pub fn is_code_at(&self, position: isize) -> bool {
        if position >= self.regions.len() as isize {
            return self.open_fence.is_some() || self.open_span.is_some();
        }
        if position < 0 {
            return false;
        }
        self.regions[position as usize] != Region::Prose
    }

    /// Whether the position is inside a fenced code block (marker, info, or body)
    pub fn is_fence_at(&self, position: isize) -> bool {
        if position >= self.regions.len() as isize {
            return self.open_fence.is_some();
        }
        if position < 0 {
            return false;
        }
        matches!(
            self.regions[position as usize],
            Region::FenceMarker | Region::FenceInfo | Region::FenceBody
        )
    }

    pub fn is_complete_span_at(&self, position: isize) -> bool {
        if position < 0 || position >= self.regions.len() as isize {
            return false;
        }
        self.regions[position as usize] == Region::CodeSpan
    }

    pub fn is_prose(&self, position: usize) -> bool {
        position < self.regions.len() && self.regions[position] == Region::Prose
    }

    pub fn in_math_at(&mut self, position: usize) -> bool {
        if position >= self.text.len() {
            return false;
        }
        if self.math_mask.is_none() {
            let mask = if has_math_delimiters(&self.text) {
                build_math_mask(self)
            } else {
                Vec::new()
            };
            self.math_mask = Some(mask);
        }
        let mask = self.math_mask.as_ref().expect("math mask was built above");
        position < mask.len() && mask[position]
    }

    pub fn in_link_url_at(&mut self, position: usize) -> bool {
        if position >= self.text.len() {
            return false;
        }
        if self.link_url_mask.is_none() {
            let mask = if self.text.contains("](") {
                build_link_url_mask(self)
            } else {
                Vec::new()
            };
            self.link_url_mask = Some(mask);
        }
        let mask = self
            .link_url_mask
            .as_ref()
            .expect("link url mask was built above");
        position < mask.len() && mask[position]
    }

    pub fn in_html_tag_at(&mut self, position: usize) -> bool {
        if position >= self.text.len() {
            return false;
        }
        if self.html_tag_mask.is_none() {
            let mask = if self.text.contains('<') {
                build_html_tag_mask(self)
            } else {
                Vec::new()
            };
            self.html_tag_mask = Some(mask);
        }
        let mask = self
            .html_tag_mask
            .as_ref()
            .expect("html tag mask was built above");
        position < mask.len() && mask[position]
    }

    /// Counts non-overlapping double-character pairs (**, ~~, $$) in prose.
    pub fn count_double_pairs(&self, ch: u8) -> usize {
        let text = self.text.as_bytes();
        let mut count = 0usize;
        let mut i = 0usize;
        while i + 1 < text.len() {
            if text[i] == ch && text[i + 1] == ch {
                if self.regions[i] == Region::Prose {
                    count += 1;
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        count
    }
}

fn is_digit(text: &str, i: usize) -> bool {
    let b = text.as_bytes();
    i < b.len() && b[i] >= b'0' && b[i] <= b'9'
}

fn at(text: &str, i: usize) -> u8 {
    let b = text.as_bytes();
    if i < b.len() { b[i] } else { 0 }
}

/// Index just past a list marker and its space at `start`, or None.
fn skip_list_marker(text: &str, start: usize) -> Option<usize> {
    let mut i = start;
    let c = at(text, i);
    if c == b'-' || c == b'*' || c == b'+' {
        i += 1;
    } else {
        while is_digit(text, i) && i - start < 9 {
            i += 1;
        }
        if i == start || (at(text, i) != b'.' && at(text, i) != b')') {
            return None;
        }
        i += 1;
    }
    if at(text, i) == b' ' {
        Some(i + 1)
    } else {
        None
    }
}

struct ContainerPrefix {
    content_start: usize,
    list_indent: usize,
    quote_depth: usize,
}

fn skip_container_prefix(text: &str, line_start: usize, line_end: usize) -> ContainerPrefix {
    let b = text.as_bytes();
    let mut quote_depth = 0usize;
    let mut after_quotes = line_start;
    let mut starts_item = false;
    let mut i = line_start;
    loop {
        while i < line_end && b[i] == b' ' {
            i += 1;
        }
        if i < line_end && b[i] == b'>' {
            quote_depth += 1;
            i += if at(text, i + 1) == b' ' { 2 } else { 1 };
            after_quotes = i;
            starts_item = false;
            continue;
        }
        let after_marker = if i < line_end {
            skip_list_marker(text, i)
        } else {
            None
        };
        let Some(after) = after_marker else {
            return ContainerPrefix {
                content_start: i,
                list_indent: if starts_item { i - after_quotes } else { 0 },
                quote_depth,
            };
        };
        i = after;
        starts_item = true;
    }
}

fn skip_quote_markers(
    text: &str,
    line_start: usize,
    line_end: usize,
    depth: usize,
) -> Option<usize> {
    let mut i = line_start;
    let mut found = 0usize;
    while found < depth {
        while i < line_end && at(text, i) == b' ' {
            i += 1;
        }
        if at(text, i) != b'>' {
            return None;
        }
        i += 1;
        if at(text, i) == b' ' {
            i += 1;
        }
        found += 1;
    }
    Some(i)
}

fn fill(regions: &mut [Region], value: Region, start: usize, end: usize) {
    let s = start.min(regions.len());
    let e = end.min(regions.len());
    if s < e {
        regions[s..e].fill(value);
    }
}

fn is_fence_closer(text: &str, content_start: usize, line_end: usize, fence: OpenFence) -> bool {
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

fn open_fence_at(
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

fn fence_content_start(
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

fn paint_fences(text: &str, regions: &mut [Region]) -> Option<OpenFence> {
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

fn measure_backtick_run(text: &str, start: usize) -> usize {
    let b = text.as_bytes();
    let mut end = start + 1;
    while end < b.len() && b[end] == b'`' {
        end += 1;
    }
    end
}

fn is_paragraph_break_at(text: &str, newline_index: usize) -> bool {
    let b = text.as_bytes();
    let mut j = newline_index + 1;
    while j < b.len() && (b[j] == b' ' || b[j] == b'\t' || b[j] == b'\r') {
        j += 1;
    }
    j < b.len() && b[j] == b'\n'
}

fn next_span_opener(text: &str, regions: &[Region], from: usize) -> Option<usize> {
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

fn paint_spans(text: &str, regions: &mut [Region]) -> Option<OpenSpan> {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum MathContext {
    None,
    InlineDollar,
    BlockDollar,
    InlineLatex,
    BlockLatex,
}

fn has_math_delimiters(text: &str) -> bool {
    text.contains('$') || text.contains("\\(") || text.contains("\\[")
}

fn build_math_mask(scan: &Scan) -> Vec<bool> {
    let text = scan.text.as_bytes();
    let mut mask = vec![false; text.len()];
    let mut context = MathContext::None;
    let mut i = 0usize;
    while i < text.len() {
        mask[i] = context != MathContext::None;
        if scan.regions[i] != Region::Prose {
            i += 1;
            continue;
        }
        let next = at(&scan.text, i + 1);
        let mut new_context: Option<MathContext> = None;
        let mut length = 1usize;
        if text[i] == b'\\' {
            if next == b'$' {
                new_context = Some(context);
                length = 2;
            } else {
                let latex: Option<MathContext> = if next == b'[' && context == MathContext::None {
                    Some(MathContext::BlockLatex)
                } else if next == b']' && context == MathContext::BlockLatex {
                    Some(MathContext::None)
                } else if next == b'(' && context == MathContext::None {
                    Some(MathContext::InlineLatex)
                } else if next == b')' && context == MathContext::InlineLatex {
                    Some(MathContext::None)
                } else {
                    None
                };
                if let Some(value) = latex {
                    new_context = Some(value);
                    length = 2;
                }
            }
        } else if text[i] == b'$'
            && context != MathContext::InlineLatex
            && context != MathContext::BlockLatex
        {
            let is_block = next == b'$';
            new_context = Some(if is_block {
                if context == MathContext::BlockDollar {
                    MathContext::None
                } else {
                    MathContext::BlockDollar
                }
            } else if context == MathContext::BlockDollar {
                context
            } else if context == MathContext::InlineDollar {
                MathContext::None
            } else {
                MathContext::InlineDollar
            });
            length = if is_block { 2 } else { 1 };
        }
        let Some(value) = new_context else {
            i += 1;
            continue;
        };
        context = value;
        if length == 2 && i + 1 < text.len() {
            mask[i + 1] = context != MathContext::None;
        }
        i += length;
    }
    mask
}

fn build_link_url_mask(scan: &Scan) -> Vec<bool> {
    let text = scan.text.as_bytes();
    let mut mask = vec![false; text.len()];
    let mut line_start = 0usize;
    while line_start < text.len() {
        let line_end = scan.text[line_start..]
            .find('\n')
            .map(|i| i + line_start)
            .unwrap_or(text.len());
        // closerFollows: a ")" exists at or after i on this line
        let mut closer_after: Option<usize> = None;
        let mut follows = vec![false; line_end - line_start];
        let mut i = line_end;
        while i > line_start {
            i -= 1;
            if text[i] == b')' && scan.regions[i] == Region::Prose {
                closer_after = Some(i);
            }
            follows[i - line_start] = closer_after.is_some();
        }
        let mut in_url = false;
        let mut i = line_start;
        while i < line_end {
            if in_url && follows[i - line_start] {
                mask[i] = true;
            }
            if scan.regions[i] != Region::Prose {
                i += 1;
                continue;
            }
            if text[i] == b')' {
                in_url = false;
            } else if text[i] == b'(' {
                in_url = i > 0 && text[i - 1] == b']';
            }
            i += 1;
        }
        line_start = line_end + 1;
    }
    mask
}

fn build_html_tag_mask(scan: &Scan) -> Vec<bool> {
    let text = scan.text.as_bytes();
    let mut mask = vec![false; text.len()];
    let mut in_tag = false;
    for (i, &c) in text.iter().enumerate() {
        if c == b'\n' {
            in_tag = false;
            continue;
        }
        mask[i] = in_tag;
        if scan.regions[i] != Region::Prose {
            continue;
        }
        if c == b'>' {
            in_tag = false;
        } else if c == b'<' {
            let next = at(&scan.text, i + 1);
            in_tag = next.is_ascii_alphabetic() || next == b'/';
        }
    }
    mask
}

#[test]
fn fences_and_spans_are_painted() {
    let text = "a `b` c\n```js\nx\n```\n``open";
    let scan = Scan::new(text);
    assert_eq!(scan.regions[0], Region::Prose);
    assert_eq!(scan.regions[2], Region::CodeSpan);
    assert!(scan.is_fence_at(12));
    assert!(scan.open_span.is_some());
    assert!(scan.open_fence.is_none());
}
