//! Port of `src/markdown/scan.zig` — WP-3.2.
//!
//! Line- and character-level recognizers shared by the block parser, the
//! streaming healer and the editor commands.

use std::borrow::Cow;

use super::ast::Align;

// ── characters ──────────────────────────────────────────────────────────

pub fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

pub fn is_ascii_punct(c: u8) -> bool {
    matches!(c, b'!'..=b'/' | b':'..=b'@' | b'['..=b'`' | b'{'..=b'~')
}

/// UTF-8 sequence length implied by a leading byte, `None` when the byte is
/// not a valid leading byte (Zig `std.unicode.utf8ByteSequenceLength`).
pub fn utf8_seq_len(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7F => Some(1),
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

/// `std.unicode.utf8ByteSequenceLength(c) catch 1`.
pub fn utf8_byte_sequence_length_or_1(byte: u8) -> usize {
    utf8_seq_len(byte).unwrap_or(1)
}

/// Decodes the code point that ends right before `index` (0 when none).
pub fn cp_before(text: &str, index: usize) -> u32 {
    if index == 0 {
        return 0;
    }
    let bytes = text.as_bytes();
    let mut start = index - 1;
    while start > 0 && (bytes[start] & 0xC0) == 0x80 {
        start -= 1;
    }
    decode_exact(&bytes[start..index]).unwrap_or(0xFFFD)
}

/// Decodes the code point starting at `index` (0 when none).
pub fn cp_at(text: &str, index: usize) -> u32 {
    if index >= text.len() {
        return 0;
    }
    let bytes = text.as_bytes();
    let Some(len) = utf8_seq_len(bytes[index]) else {
        return 0xFFFD;
    };
    if index + len > text.len() {
        return 0xFFFD;
    }
    decode_exact(&bytes[index..index + len]).unwrap_or(0xFFFD)
}

/// Zig `std.unicode.utf8Decode`: the whole slice must be exactly one codepoint.
fn decode_exact(bytes: &[u8]) -> Option<u32> {
    let s = std::str::from_utf8(bytes).ok()?;
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_some() || c.len_utf8() != s.len() {
        return None;
    }
    Some(c as u32)
}

pub fn is_cjk(cp: u32) -> bool {
    (0x2E80..=0x9FFF).contains(&cp) || // radicals, kana, CJK unified
        (0xAC00..=0xD7AF).contains(&cp) || // hangul
        (0xF900..=0xFAFF).contains(&cp) ||
        (0xFF00..=0xFFEF).contains(&cp) || // full-width forms
        (0x20000..=0x2FA1F).contains(&cp)
}

/// CJK and general punctuation (。，、！？「」…).
pub fn is_unicode_punct(cp: u32) -> bool {
    if cp < 0x80 {
        return cp != 0 && is_ascii_punct(cp as u8);
    }
    (0x2000..=0x206F).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0xFF01..=0xFF0F).contains(&cp)
        || (0xFF1A..=0xFF20).contains(&cp)
        || (0xFF3B..=0xFF40).contains(&cp)
        || (0xFF5B..=0xFF65).contains(&cp)
}

pub fn is_whitespace_cp(cp: u32) -> bool {
    cp == 0
        || cp == u32::from(b' ')
        || cp == u32::from(b'\t')
        || cp == u32::from(b'\n')
        || cp == u32::from(b'\r')
        || cp == 0xA0
        || cp == 0x3000
}

pub fn is_word_cp(cp: u32) -> bool {
    if cp < 0x80 {
        return (cp as u8).is_ascii_alphanumeric();
    }
    !is_whitespace_cp(cp) && !is_unicode_punct(cp) && !is_cjk(cp)
}

// ── lines ───────────────────────────────────────────────────────────────

/// `str::trim_matches` for the space / tab set Zig trims with `" \t"`.
pub fn trim_space_tab(text: &str) -> &str {
    text.trim_matches([' ', '\t'])
}

pub fn is_blank(line: &str) -> bool {
    line.bytes().all(|c| is_space(c) || c == b'\r')
}

/// Number of leading spaces (tabs are expanded before parsing).
pub fn indent_of(line: &str) -> usize {
    line.bytes().take_while(|c| *c == b' ').count()
}

pub fn strip_indent(line: &str, max: usize) -> &str {
    let mut n = 0;
    let bytes = line.as_bytes();
    while n < line.len() && n < max && bytes[n] == b' ' {
        n += 1;
    }
    &line[n..]
}

pub fn is_thematic_break(line: &str) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let mut count: usize = 0;
    let mut marker: u8 = 0;
    for c in line.bytes() {
        if is_space(c) {
            continue;
        }
        if c != b'-' && c != b'*' && c != b'_' {
            return false;
        }
        if marker == 0 {
            marker = c;
        } else if c != marker {
            return false;
        }
        count += 1;
    }
    count >= 3
}

pub struct AtxHeading<'a> {
    pub level: u8,
    pub text: &'a str,
}

pub fn atx_heading(line: &str) -> Option<AtxHeading<'_>> {
    if indent_of(line) > 3 {
        return None;
    }
    let t = line.trim_start_matches(' ');
    let mut level: u8 = 0;
    let bytes = t.as_bytes();
    while (level as usize) < t.len() && bytes[level as usize] == b'#' {
        level += 1;
    }
    if level == 0 || level > 6 {
        return None;
    }
    if (level as usize) < t.len() && !is_space(bytes[level as usize]) {
        return None;
    }
    let mut body = trim_space_tab(&t[level as usize..]);
    // optional closing sequence: "## Title ##"
    let mut end = body.len();
    while end > 0 && body.as_bytes()[end - 1] == b'#' {
        end -= 1;
    }
    if end == 0 {
        body = "";
    } else if end < body.len() && is_space(body.as_bytes()[end - 1]) {
        body = trim_space_tab(&body[..end]);
    }
    Some(AtxHeading { level, text: body })
}

/// `=` → 1, `-` → 2 for a setext underline, else `None`.
pub fn setext_level(line: &str) -> Option<u8> {
    if indent_of(line) > 3 {
        return None;
    }
    let t = trim_space_tab(line);
    if t.is_empty() {
        return None;
    }
    let c = t.as_bytes()[0];
    if c != b'=' && c != b'-' {
        return None;
    }
    if t.bytes().any(|x| x != c) {
        return None;
    }
    Some(if c == b'=' { 1 } else { 2 })
}

pub struct Fence<'a> {
    pub char: u8,
    pub len: usize,
    pub indent: usize,
    pub info: &'a str,
}

pub fn fence_open(line: &str) -> Option<Fence<'_>> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let t = &line[indent..];
    if t.len() < 3 {
        return None;
    }
    let c = t.as_bytes()[0];
    if c != b'`' && c != b'~' {
        return None;
    }
    let mut n = 0;
    let bytes = t.as_bytes();
    while n < t.len() && bytes[n] == c {
        n += 1;
    }
    if n < 3 {
        return None;
    }
    let info = trim_space_tab(&t[n..]);
    if c == b'`' && info.as_bytes().contains(&b'`') {
        return None;
    }
    Some(Fence {
        char: c,
        len: n,
        indent,
        info,
    })
}

pub fn is_fence_close(line: &str, open: &Fence<'_>) -> bool {
    let indent = indent_of(line);
    if indent > 3 {
        return false;
    }
    let t = line[indent..].trim_end_matches([' ', '\t']);
    if t.len() < open.len {
        return false;
    }
    t.bytes().all(|c| c == open.char)
}

pub struct ListMarker<'a> {
    pub ordered: bool,
    pub bullet: u8,
    pub delimiter: u8,
    pub number: usize,
    /// Columns from line start to the item content.
    pub content_indent: usize,
    /// Item text after the marker (may be empty).
    pub rest: &'a str,
}

pub fn list_marker(line: &str) -> Option<ListMarker<'_>> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let t = &line[indent..];
    if t.is_empty() {
        return None;
    }
    let bytes = t.as_bytes();
    let marker_len: usize;
    let mut result = ListMarker {
        ordered: false,
        bullet: 0,
        delimiter: 0,
        number: 1,
        content_indent: 0,
        rest: "",
    };
    if bytes[0] == b'-' || bytes[0] == b'*' || bytes[0] == b'+' {
        marker_len = 1;
        result.bullet = bytes[0];
    } else if bytes[0].is_ascii_digit() {
        let mut d: usize = 0;
        while d < t.len() && d < 9 && bytes[d].is_ascii_digit() {
            d += 1;
        }
        if d >= t.len() || (bytes[d] != b'.' && bytes[d] != b')') {
            return None;
        }
        result.ordered = true;
        result.delimiter = bytes[d];
        result.number = t[..d].parse::<usize>().unwrap_or(1);
        marker_len = d + 1;
    } else {
        return None;
    }
    let after = &t[marker_len..];
    if !after.is_empty() && !is_space(after.as_bytes()[0]) {
        return None;
    }
    let mut spaces: usize = 0;
    while spaces < after.len() && after.as_bytes()[spaces] == b' ' {
        spaces += 1;
    }
    // 5+ spaces: content is an indented code block; content starts after 1.
    if spaces > 4 || spaces == after.len() {
        spaces = spaces.min(1);
    }
    result.content_indent = indent + marker_len + spaces.max(1);
    result.rest = if spaces < after.len() {
        &after[spaces..]
    } else {
        ""
    };
    Some(result)
}

pub fn is_html_block_start(line: &str) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let t = line.trim_start_matches(' ');
    if t.len() < 2 || t.as_bytes()[0] != b'<' {
        return false;
    }
    if t.starts_with("<!--") {
        return true;
    }
    if t.starts_with("<http") || t.starts_with("<mailto:") {
        return false;
    }
    let bytes = t.as_bytes();
    let mut i: usize = 1;
    if bytes[i] == b'/' {
        i += 1;
    }
    if i >= t.len() || !bytes[i].is_ascii_alphabetic() {
        return false;
    }
    let mut j = i;
    while j < t.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
        j += 1;
    }
    // must look like a tag: <div>, <div ...>, <br/>, </div>
    if j < t.len() && bytes[j] != b'>' && bytes[j] != b' ' && bytes[j] != b'/' && bytes[j] != b'\t'
    {
        return false;
    }
    // autolink-ish "<foo@bar>" is not a block
    let prefix_end = t.len().min(j + 1);
    if t.as_bytes()[..prefix_end].contains(&b'@') {
        return false;
    }
    true
}

pub fn is_math_fence(line: &str) -> bool {
    let t = trim_space_tab(line);
    t.starts_with("$$") || t == "\\["
}

/// Splits a GFM table row into trimmed cells, honouring `\|` escapes and
/// pipes inside code spans.
pub fn split_table_row(line: &str) -> Vec<Cow<'_, str>> {
    let mut t = trim_space_tab(line);
    if t.starts_with('|') {
        t = &t[1..];
    }
    if t.ends_with('|') && !(t.len() > 1 && t.as_bytes()[t.len() - 2] == b'\\') {
        t = &t[..t.len() - 1];
    }
    let mut cells: Vec<Cow<'_, str>> = Vec::new();
    let mut owned: Option<String> = None;
    let mut start = 0usize;
    let mut in_code = false;
    let mut i: usize = 0;
    let bytes = t.as_bytes();
    while i < t.len() {
        let c = bytes[i];
        if c == b'\\' && i + 1 < t.len() && bytes[i + 1] == b'|' {
            let buffer = owned.get_or_insert_with(String::new);
            buffer.push_str(&t[start..i]);
            buffer.push('|');
            i += 1;
            i += 1;
            start = i;
            continue;
        }
        if c == b'`' {
            in_code = !in_code;
        }
        if c == b'|' && !in_code {
            cells.push(cell_from(t, &mut owned, start, i));
            start = i + 1;
        }
        i += 1;
    }
    cells.push(cell_from(t, &mut owned, start, t.len()));
    cells
}

fn cell_from<'a>(t: &'a str, owned: &mut Option<String>, start: usize, end: usize) -> Cow<'a, str> {
    match owned.take() {
        Some(mut buffer) => {
            buffer.push_str(&t[start..end]);
            Cow::Owned(trim_space_tab(&buffer).to_string())
        }
        None => Cow::Borrowed(trim_space_tab(&t[start..end])),
    }
}

/// Parses a delimiter row (`| :--- | ---: |`). Returns `None` if invalid.
pub fn table_delimiter(line: &str) -> Option<Vec<Align>> {
    if !line.as_bytes().contains(&b'-') {
        return None;
    }
    let t = trim_space_tab(line);
    if t.is_empty() {
        return None;
    }
    let cells = split_table_row(t);
    if cells.is_empty() {
        return None;
    }
    // a single column needs an explicit pipe to avoid matching setext/rules
    if cells.len() == 1 && !t.as_bytes().contains(&b'|') {
        return None;
    }
    let mut out = Vec::with_capacity(cells.len());
    for cell in &cells {
        if cell.is_empty() {
            return None;
        }
        let cell_bytes = cell.as_bytes();
        let left = cell_bytes[0] == b':';
        let right = cell_bytes[cell.len() - 1] == b':';
        let body = &cell[usize::from(left)..cell.len() - usize::from(right && cell.len() > 1)];
        if body.is_empty() {
            return None;
        }
        if body.bytes().any(|c| c != b'-') {
            return None;
        }
        out.push(if left && right {
            Align::Center
        } else if right {
            Align::Right
        } else if left {
            Align::Left
        } else {
            Align::None
        });
    }
    Some(out)
}

/// Expands tabs in the leading whitespace to 4-column stops.
pub fn expand_leading_tabs(line: &str) -> Cow<'_, str> {
    let bytes = line.as_bytes();
    let mut i: usize = 0;
    let mut has_tab = false;
    while i < line.len() && is_space(bytes[i]) {
        if bytes[i] == b'\t' {
            has_tab = true;
        }
        i += 1;
    }
    if !has_tab {
        return Cow::Borrowed(line);
    }
    let mut out = String::new();
    let mut col: usize = 0;
    for &c in &bytes[..i] {
        if c == b'\t' {
            let n = 4 - (col % 4);
            out.push_str(&" ".repeat(n));
            col += n;
        } else {
            out.push(' ');
            col += 1;
        }
    }
    out.push_str(&line[i..]);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_markers() {
        let m = list_marker("  12) item").expect("ordered marker");
        assert!(m.ordered);
        assert_eq!(m.number, 12);
        assert_eq!(m.content_indent, 6);
        assert_eq!(m.rest, "item");
        assert!(list_marker("-not").is_none());
        assert_eq!(list_marker("* ").unwrap().rest.len(), 0);
    }

    #[test]
    fn fences_and_headings() {
        let f = fence_open("~~~~ zig title=a").expect("fence");
        assert_eq!(f.len, 4);
        assert!(is_fence_close("~~~~~", &f));
        assert!(!is_fence_close("~~~", &f));
        assert_eq!(atx_heading("## Title ##").unwrap().text, "Title");
        assert!(atx_heading("#Title").is_none());
        assert_eq!(atx_heading("# C#").unwrap().text, "C#");
    }
}
