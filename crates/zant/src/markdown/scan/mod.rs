//! Line- and character-level recognizers shared by the block parser, the
//! streaming healer and the editor commands.

use std::borrow::Cow;

use super::ast::Align;

// ── characters ──────────────────────────────────────────────────────────

mod chars;
mod tables;
#[cfg(test)]
mod tests;

pub use self::chars::*;
pub use self::tables::*;

pub fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

pub fn is_ascii_punct(c: u8) -> bool {
    matches!(c, b'!'..=b'/' | b':'..=b'@' | b'['..=b'`' | b'{'..=b'~')
}

/// UTF-8 sequence length implied by a leading byte, 1 for an invalid one.
pub fn utf8_byte_sequence_length_or_1(byte: u8) -> usize {
    utf8_seq_len(byte).unwrap_or(1)
}

/// Trims spaces and tabs from both ends.
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
