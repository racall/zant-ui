//! Port of `src/markdown/remend/patterns.zig` (remend `patterns.ts` plus the
//! character helpers of `utils.ts`).
//!
//! The original uses end-anchored regular expressions; each `TrailingPattern`
//! below reproduces one of them with an explicit leftmost-match search.
//!
//! The Zig module reuses the code-point helpers of `src/markdown/scan.zig`
//! (WP-3.2). They are reproduced here so this module does not depend on a
//! module that is ported separately; the logic is identical.

#[derive(Clone, Copy)]
pub enum Body {
    /// `([^D]*?)$` — no delimiter after the marker
    None,
    /// `([^D]*\D?)$` — no delimiter except an optional final one (bold)
    OptionalFinal,
    /// `([^D]+)D$` — half-complete closer: content then one delimiter
    HalfComplete,
}

#[derive(Clone, Copy)]
pub struct TrailingPattern {
    pub delimiter: u8,
    pub marker: &'static str,
    pub body: Body,
}

pub struct Match<'a> {
    /// Full match (marker + content) — `match[0]`
    pub whole: &'a str,
    /// `match[1]`
    pub marker: &'a str,
    /// `match[2]`
    pub content: &'a str,
}

pub const BOLD: TrailingPattern = TrailingPattern {
    delimiter: b'*',
    marker: "**",
    body: Body::OptionalFinal,
};
pub const ITALIC: TrailingPattern = TrailingPattern {
    delimiter: b'_',
    marker: "__",
    body: Body::None,
};
pub const BOLD_ITALIC: TrailingPattern = TrailingPattern {
    delimiter: b'*',
    marker: "***",
    body: Body::None,
};
pub const SINGLE_ASTERISK: TrailingPattern = TrailingPattern {
    delimiter: b'*',
    marker: "*",
    body: Body::None,
};
pub const SINGLE_UNDERSCORE: TrailingPattern = TrailingPattern {
    delimiter: b'_',
    marker: "_",
    body: Body::None,
};
pub const STRIKETHROUGH: TrailingPattern = TrailingPattern {
    delimiter: b'~',
    marker: "~~",
    body: Body::None,
};
pub const HALF_COMPLETE_UNDERSCORE: TrailingPattern = TrailingPattern {
    delimiter: b'_',
    marker: "__",
    body: Body::HalfComplete,
};
pub const HALF_COMPLETE_TILDE: TrailingPattern = TrailingPattern {
    delimiter: b'~',
    marker: "~~",
    body: Body::HalfComplete,
};

fn body_matches(rest: &[u8], delimiter: u8, body: Body) -> bool {
    match body {
        Body::None => !rest.contains(&delimiter),
        Body::OptionalFinal => {
            let inner = if !rest.is_empty() && rest[rest.len() - 1] == delimiter {
                &rest[..rest.len() - 1]
            } else {
                rest
            };
            !inner.contains(&delimiter)
        }
        Body::HalfComplete => {
            if rest.len() < 2 || rest[rest.len() - 1] != delimiter {
                return false;
            }
            !rest[..rest.len() - 1].contains(&delimiter)
        }
    }
}

/// `matchTrailing(text, pattern)`: only the tail starting near the last
/// delimiter (excluding the final character) is examined.
pub fn match_trailing<'a>(text: &'a str, pattern: TrailingPattern) -> Option<Match<'a>> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    // text.lastIndexOf(delimiter, text.length - 2)
    let mut last: Option<usize> = None;
    if bytes.len() >= 2 {
        let mut i = bytes.len() - 1;
        while i > 0 {
            i -= 1;
            if bytes[i] == pattern.delimiter {
                last = Some(i);
                break;
            }
        }
    }
    let marker_end = last.unwrap_or(bytes.len() - 1);
    let begin = if marker_end + 1 >= pattern.marker.len() {
        marker_end + 1 - pattern.marker.len()
    } else {
        0
    };
    let slice = &bytes[begin..];
    let marker = pattern.marker.as_bytes();
    let mut p = 0usize;
    while p + marker.len() <= slice.len() {
        if &slice[p..p + marker.len()] != marker {
            p += 1;
            continue;
        }
        let rest = &slice[p + marker.len()..];
        if body_matches(rest, pattern.delimiter, pattern.body) {
            let abs = begin + p;
            return Some(Match {
                whole: &text[abs..],
                marker: &text[abs..abs + marker.len()],
                content: &text[abs + marker.len()..],
            });
        }
        p += 1;
    }
    None
}

/// `/^[\s_~*`]*$/`
pub fn is_whitespace_or_markers(text: &str) -> bool {
    for &c in text.as_bytes() {
        match c {
            b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C | b'_' | b'~' | b'*' | b'`' => {}
            _ => return false,
        }
    }
    true
}

/// `/^[\s]*[-*+][\s]+$/`
pub fn is_list_item_prefix(text: &str) -> bool {
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= b.len() || (b[i] != b'-' && b[i] != b'*' && b[i] != b'+') {
        return false;
    }
    i += 1;
    if i >= b.len() {
        return false;
    }
    while i < b.len() {
        if !b[i].is_ascii_whitespace() {
            return false;
        }
        i += 1;
    }
    true
}

/// `/^\*{4,}$/`
pub fn is_four_or_more_asterisks(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() < 4 {
        return false;
    }
    for &c in b {
        if c != b'*' {
            return false;
        }
    }
    true
}

/// `isWordChar`: ASCII letters, digits, `_`, or any Unicode letter/number.
/// Takes the code point (0 = no character).
pub fn is_word_char(cp: u32) -> bool {
    if cp == 0 {
        return false;
    }
    if cp < 0x80 {
        return (cp as u8).is_ascii_alphanumeric() || cp == b'_' as u32;
    }
    // \p{L}\p{N}: approximate as "not whitespace, not punctuation/symbol".
    !is_whitespace_cp(cp) && !is_unicode_punct(cp) && !is_symbol(cp)
}

fn is_symbol(cp: u32) -> bool {
    (0x2100..=0x2BFF).contains(&cp) || // letterlike, arrows, math, technical, shapes
        (0x1F000..=0x1FAFF).contains(&cp) || // emoji
        (0xFE00..=0xFE0F).contains(&cp) ||
        cp == 0x00A9 ||
        cp == 0x00AE ||
        cp == 0x00B0 ||
        cp == 0x00D7 ||
        cp == 0x00F7
}

fn is_ascii_punct(c: u8) -> bool {
    matches!(c, b'!'..=b'/' | b':'..=b'@' | b'['..=b'`' | b'{'..=b'~')
}

fn is_unicode_punct(cp: u32) -> bool {
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

fn is_whitespace_cp(cp: u32) -> bool {
    cp == 0
        || cp == b' ' as u32
        || cp == b'\t' as u32
        || cp == b'\n' as u32
        || cp == b'\r' as u32
        || cp == 0xA0
        || cp == 0x3000
}

fn decode_single(bytes: &[u8], start: usize, end: usize) -> u32 {
    if start > end || end > bytes.len() {
        return 0xFFFD;
    }
    match std::str::from_utf8(&bytes[start..end]) {
        Ok(s) => {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => c as u32,
                _ => 0xFFFD,
            }
        }
        Err(_) => 0xFFFD,
    }
}

fn utf8_byte_sequence_length(first_byte: u8) -> Option<usize> {
    match first_byte {
        0x00..=0x7F => Some(1),
        0xC0..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF7 => Some(4),
        _ => None,
    }
}

/// Previous character (code point) before byte `index`, 0 when none.
pub fn prev_char(text: &str, index: usize) -> u32 {
    if index == 0 {
        return 0;
    }
    let b = text.as_bytes();
    let mut start = index - 1;
    while start > 0 && (b[start] & 0xC0) == 0x80 {
        start -= 1;
    }
    decode_single(b, start, index)
}

/// Character starting at byte `index`, 0 when none.
pub fn char_at(text: &str, index: usize) -> u32 {
    let b = text.as_bytes();
    if index >= b.len() {
        return 0;
    }
    let Some(len) = utf8_byte_sequence_length(b[index]) else {
        return 0xFFFD;
    };
    if index + len > b.len() {
        return 0xFFFD;
    }
    decode_single(b, index, index + len)
}

/// `isHorizontalRule(text, markerIndex, marker)`
pub fn is_horizontal_rule(text: &str, marker_index: usize, marker: u8) -> bool {
    let b = text.as_bytes();
    let mut line_start = 0usize;
    let mut i = marker_index;
    while i > 0 {
        i -= 1;
        if b[i] == b'\n' {
            line_start = i + 1;
            break;
        }
    }
    let from = marker_index.min(text.len());
    let line_end = text[from..]
        .find('\n')
        .map(|x| x + from)
        .unwrap_or(text.len());
    let mut count = 0usize;
    for &c in &b[line_start..line_end] {
        if c == marker {
            count += 1;
        } else if c != b' ' && c != b'\t' {
            return false;
        }
    }
    count >= 3
}

pub fn find_matching_opening_bracket(text: &str, close_index: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut depth = 1usize;
    let mut i = close_index;
    while i > 0 {
        i -= 1;
        if b[i] == b']' {
            depth += 1;
        } else if b[i] == b'[' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

pub fn find_matching_closing_bracket(text: &str, open_index: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut depth = 1usize;
    let mut i = open_index + 1;
    while i < b.len() {
        if b[i] == b'[' {
            depth += 1;
        } else if b[i] == b']' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

pub fn last_index_of(text: &str, needle: &str) -> Option<usize> {
    text.rfind(needle)
}

#[test]
fn match_trailing_mirrors_the_regexes() {
    assert_eq!(
        "bold",
        match_trailing("Text **bold", BOLD)
            .expect("bold match")
            .content
    );
    assert_eq!(
        "bold*",
        match_trailing("**bold*", BOLD).expect("bold match").content
    );
    // parity decides later
    assert!(match_trailing("**a** b", BOLD).is_some());
    assert!(match_trailing("a b", BOLD).is_none());
    assert_eq!(
        "x",
        &match_trailing("__x_", HALF_COMPLETE_UNDERSCORE)
            .expect("half-complete underscore match")
            .content[0..1]
    );
    assert!(is_list_item_prefix("  - "));
    assert!(!is_list_item_prefix("-"));
}
