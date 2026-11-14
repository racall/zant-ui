//! Port of `src/markdown/blocks.zig` — WP-3.2.
//!
//! Port of Streamdown `lib/parse-blocks.tsx`: splits a document into
//! top-level block strings so each block can be parsed, cached and rendered
//! independently.
//!
//! Merge rules follow Streamdown: an unclosed HTML block swallows the
//! following blocks until its tag closes, a block with an odd number of `$$`
//! swallows the next one, and documents with footnotes or link reference
//! definitions stay one block (references must see their definitions).
//!
//! `Splitter` is the incremental cache (`lastParse`): while a document only
//! grows (streaming), blocks before the last stable boundary are reused and
//! only the tail is split again. It is caller-owned — no global state.

use crate::markdown::parser;
use crate::markdown::parser::block as block_parser;
use crate::markdown::scan;

fn has_footnotes(markdown: &str) -> bool {
    // /\[\^[\w-]{1,200}\](?!:)/ or /\[\^[\w-]{1,200}\]:/ — either way "[^label]"
    let bytes = markdown.as_bytes();
    let mut i: usize = 0;
    while let Some(rel) = markdown[i..].find("[^") {
        let start = i + rel;
        let mut j = start + 2;
        while j < markdown.len()
            && j - start - 2 < 200
            && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'-')
        {
            j += 1;
        }
        if j > start + 2 && j < markdown.len() && bytes[j] == b']' {
            return true;
        }
        i = start + 2;
    }
    false
}

fn has_link_definitions(markdown: &str) -> bool {
    let mut fence: Option<scan::Fence> = None;
    for raw in markdown.split('\n') {
        let mut line = raw.trim_end_matches('\r');
        // definitions may sit inside block quotes / list items
        loop {
            let t = line.trim_start_matches(' ');
            if !t.is_empty() && t.as_bytes()[0] == b'>' {
                line = &t[1..];
            } else {
                break;
            }
        }
        if let Some(f) = fence.as_ref() {
            if scan::is_fence_close(line, f) {
                fence = None;
            }
            continue;
        }
        if let Some(f) = scan::fence_open(line) {
            fence = Some(f);
            continue;
        }
        if block_parser::Parser::parse_definition(line.trim_start_matches(' ')).is_some() {
            return true;
        }
    }
    false
}

const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn is_void(tag: &str) -> bool {
    VOID_ELEMENTS.iter().any(|v| v.eq_ignore_ascii_case(tag))
}

/// `<tag(?=[\s>/])[^>]*>` occurrences that do not end in `/>`.
fn count_open_tags(block: &str, tag: &str) -> usize {
    if is_void(tag) {
        return 0;
    }
    let bytes = block.as_bytes();
    let mut count: usize = 0;
    let mut i: usize = 0;
    while let Some(rel) = block[i..].find('<') {
        let lt = i + rel;
        let name_end = lt + 1 + tag.len();
        if name_end >= block.len() {
            break;
        }
        if !block[lt + 1..name_end].eq_ignore_ascii_case(tag) {
            i = lt + 1;
            continue;
        }
        let after = bytes[name_end];
        if !(after.is_ascii_whitespace() || after == b'>' || after == b'/') {
            i = lt + 1;
            continue;
        }
        let Some(rel_close) = block[name_end..].find('>') else {
            i = lt + 1;
            continue;
        };
        let close = name_end + rel_close;
        let trimmed = block[lt..close + 1].trim_end_matches([' ', '\t']);
        if !trimmed.ends_with("/>") {
            count += 1;
        }
        i = lt + 1;
    }
    count
}

/// `</tag(?=[\s>])[^>]*>` occurrences.
fn count_close_tags(block: &str, tag: &str) -> usize {
    let bytes = block.as_bytes();
    let mut count: usize = 0;
    let mut i: usize = 0;
    while let Some(rel) = block[i..].find("</") {
        let lt = i + rel;
        let name_end = lt + 2 + tag.len();
        if name_end >= block.len() {
            break;
        }
        if !block[lt + 2..name_end].eq_ignore_ascii_case(tag) {
            i = lt + 2;
            continue;
        }
        let after = bytes[name_end];
        if !(after.is_ascii_whitespace() || after == b'>') {
            i = lt + 2;
            continue;
        }
        if block[name_end..].contains('>') {
            count += 1;
        }
        i = lt + 2;
    }
    count
}

/// `<([A-Za-z][\w:-]*)[\s>/]` — first opening tag name.
fn opening_tag_name(block: &str) -> Option<&str> {
    let bytes = block.as_bytes();
    let mut i: usize = 0;
    while let Some(rel) = block[i..].find('<') {
        let lt = i + rel;
        let mut j = lt + 1;
        if j >= block.len() || !bytes[j].is_ascii_alphabetic() {
            i = lt + 1;
            continue;
        }
        while j < block.len()
            && (bytes[j].is_ascii_alphanumeric()
                || bytes[j] == b'_'
                || bytes[j] == b':'
                || bytes[j] == b'-')
        {
            j += 1;
        }
        if j < block.len()
            && (bytes[j].is_ascii_whitespace() || bytes[j] == b'>' || bytes[j] == b'/')
        {
            return Some(&block[lt + 1..j]);
        }
        i = lt + 1;
    }
    None
}

fn count_double_dollars(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count: usize = 0;
    let mut i: usize = 0;
    while i + 1 < text.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'$' {
            count += 1;
            i += 1;
        }
        i += 1;
    }
    count
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Html,
    Code,
    Other,
}

fn classify(raw: &str) -> Kind {
    let first = raw.split('\n').next().unwrap_or(raw);
    if scan::fence_open(first).is_some() || scan::indent_of(first) >= 4 {
        return Kind::Code;
    }
    if scan::is_html_block_start(first) {
        return Kind::Html;
    }
    Kind::Other
}

/// Splits `input` into raw block strings, applying Streamdown's merge rules.
/// Each block keeps its trailing blank lines, so the blocks concatenate back
/// to `input`.
pub fn split_raw(input: &str) -> Vec<String> {
    split_raw_with(input, parser::Options::default())
}

/// `split_raw` with the document's parse options: block boundaries depend on
/// them (with `latex_delimiters`, a multi-line `\[ … \]` formula is one
/// block even when a line inside it looks like a heading underline `=` or a
/// list item `+`).
pub fn split_raw_with(input: &str, options: parser::Options) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }
    let ranges = parser::top_level_ranges(input, options);
    let starts = block_parser::line_starts(input);
    let mut tokens: Vec<&str> = Vec::new();
    for (k, range) in ranges.iter().enumerate() {
        let begin = if k == 0 {
            0
        } else {
            starts[range.start.min(starts.len() - 1)]
        };
        let end = if k + 1 < ranges.len() {
            starts[ranges[k + 1].start.min(starts.len() - 1)]
        } else {
            input.len()
        };
        if end > begin {
            tokens.push(&input[begin..end]);
        }
    }
    if tokens.is_empty() {
        tokens.push(input);
    }
    merge_tokens_into_blocks(&tokens)
}

fn merge_tokens_into_blocks(tokens: &[&str]) -> Vec<String> {
    let mut merged: Vec<String> = Vec::new();
    let mut html_stack: Vec<&str> = Vec::new();
    let mut previous_was_code = false;
    for current in tokens {
        if let Some(tracked) = html_stack.last().copied() {
            merge_extend(&mut merged, current);
            let opens = count_open_tags(current, tracked);
            let closes = count_close_tags(current, tracked);
            for _ in 0..opens {
                html_stack.push(tracked);
            }
            for _ in 0..closes {
                if html_stack.last() == Some(&tracked) {
                    let _ = html_stack.pop();
                }
            }
            continue;
        }
        let kind = classify(current);
        if kind == Kind::Html
            && let Some(tag) = opening_tag_name(current)
            && count_open_tags(current, tag) > count_close_tags(current, tag)
        {
            html_stack.push(tag);
        }
        if !merged.is_empty()
            && !previous_was_code
            && count_double_dollars(merged.last().expect("merged is not empty")) % 2 == 1
        {
            merge_extend(&mut merged, current);
            continue;
        }
        merged.push(current.to_string());
        previous_was_code = kind == Kind::Code;
    }
    merged
}

/// Grows the last merged block to also cover `next` (they are adjacent
/// slices of the input).
fn merge_extend(merged: &mut [String], next: &str) {
    let last = merged.last_mut().expect("merged is not empty");
    last.push_str(next);
}

/// Number of leading blocks that cannot change when text is appended.
fn count_stable_blocks(blocks: &[String]) -> usize {
    if blocks.len() < 3 {
        return 0;
    }
    let mut i = blocks.len() - 3;
    loop {
        if blocks[i].ends_with("\n\n") {
            return i + 1;
        }
        if i == 0 {
            return 0;
        }
        i -= 1;
    }
}

/// Incremental splitter (Streamdown's single-entry `ParseCache`).
pub struct Splitter {
    input: String,
    /// Owned copies of the block strings of the last split.
    blocks: Vec<String>,
    /// Statistics for tests/profiling: blocks reused by the last split.
    pub reused: usize,
    /// Parse options the blocks were split with (`set_options`).
    pub options: parser::Options,
}

impl Default for Splitter {
    fn default() -> Self {
        Splitter::new()
    }
}

impl Splitter {
    pub fn new() -> Splitter {
        Splitter {
            input: String::new(),
            blocks: Vec::new(),
            reused: 0,
            options: parser::Options::default(),
        }
    }

    /// Parse options for the following splits; a change drops the cache.
    pub fn set_options(&mut self, options: parser::Options) {
        if self.options == options {
            return;
        }
        self.options = options;
        self.blocks.clear();
        self.input.clear();
    }

    /// Returns the blocks of `markdown` (owned copies; the Zig version returns
    /// slices valid until the next call).
    pub fn split(&mut self, markdown: &str) -> Vec<String> {
        let normalized;
        let input: &str = if markdown.as_bytes().contains(&b'\r') {
            normalized = normalize_newlines(markdown);
            normalized.as_str()
        } else {
            markdown
        };
        if input == self.input.as_str() && (!self.blocks.is_empty() || input.is_empty()) {
            self.reused = self.blocks.len();
            return self.blocks.clone();
        }

        // footnotes and link definitions: the whole document is one block
        if has_footnotes(input) || has_link_definitions(input) {
            self.replace_all(input);
            self.reused = 0;
            return self.blocks.clone();
        }

        let mut stable: usize = 0;
        let mut stable_len: usize = 0;
        if input.len() > self.input.len() && input.starts_with(self.input.as_str()) {
            stable = count_stable_blocks(&self.blocks);
            for b in &self.blocks[..stable] {
                stable_len += b.len();
            }
        }
        let tail = split_raw_with(&input[stable_len..], self.options);
        self.blocks.truncate(stable);
        self.blocks.extend(tail);
        self.input.clear();
        self.input.push_str(input);
        self.reused = stable;
        self.blocks.clone()
    }

    fn replace_all(&mut self, input: &str) {
        self.blocks.clear();
        self.blocks.push(input.to_string());
        self.input.clear();
        self.input.push_str(input);
    }
}

fn normalize_newlines(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    let mut i: usize = 0;
    while i < text.len() {
        if bytes[i] == b'\r' {
            out.push(b'\n');
            if i + 1 < text.len() && bytes[i + 1] == b'\n' {
                i += 1;
            }
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(out).expect("normalizing newlines preserves UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_concatenate_back_to_the_input() {
        let src = "# T\n\npara\n\n```js\nx\n\ny\n```\n\n$$\na\n\n= b\n$$\n\n<div>\n\ninner\n\n</div>\n\nend";
        let blocks = split_raw(src);
        let mut joined = String::new();
        for b in &blocks {
            joined.push_str(b);
        }
        assert_eq!(src, joined);
        // heading, para, code, math (merged), html (merged), end
        assert_eq!(blocks.len(), 6);
    }

    #[test]
    fn splitter_reuses_stable_blocks_while_streaming() {
        let mut s = Splitter::new();
        let _ = s.split("# A\n\none\n\ntwo\n\nthr");
        let blocks = s.split("# A\n\none\n\ntwo\n\nthree and more\n\nfour");
        assert!(s.reused >= 2);
        assert_eq!(blocks[0], "# A\n\n");
        assert_eq!(blocks[blocks.len() - 1], "four");
    }
}
