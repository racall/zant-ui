//! Grapheme-cluster movement, UTF-8 ↔ UTF-16 offset conversion and word
//! boundaries for the text widgets.
//!
//! Decoding falls back to the raw first byte when a byte sequence is invalid
//! or truncated. Every value produced here is a valid Unicode scalar value
//! (the invalid-byte fallback is always `< 0x100`), so codepoints are `char`s.

/// One decoded codepoint at a byte offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The codepoint, or the raw first byte when the sequence is invalid.
    pub codepoint: char,
    /// Number of bytes consumed; 0 at (or past) the end of the text.
    pub len: usize,
}

/// The sequence length implied by a
/// lead byte, or `None` when the byte cannot start a sequence.
fn utf8_byte_sequence_length(first_byte: u8) -> Option<usize> {
    match first_byte {
        0x00..=0x7F => Some(1),
        0xC0..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF7 => Some(4),
        _ => None,
    }
}

/// Decodes one codepoint at `at`, never splitting a sequence and
/// falling back to the raw byte at `at` for malformed input.
pub fn decode_at(text: &str, at: usize) -> Decoded {
    let bytes = text.as_bytes();
    if at >= bytes.len() {
        return Decoded {
            codepoint: '\0',
            len: 0,
        };
    }
    let Some(len) = utf8_byte_sequence_length(bytes[at]) else {
        return Decoded {
            codepoint: bytes[at] as char,
            len: 1,
        };
    };
    if at + len > bytes.len() {
        return Decoded {
            codepoint: bytes[at] as char,
            len: 1,
        };
    }
    match std::str::from_utf8(&bytes[at..at + len]) {
        Ok(sequence) => Decoded {
            codepoint: sequence
                .chars()
                .next()
                .expect("a valid UTF-8 sequence decodes one codepoint"),
            len,
        },
        Err(_) => Decoded {
            codepoint: bytes[at] as char,
            len: 1,
        },
    }
}

/// Byte offset of the next codepoint (`text.len()` at the end).
pub fn next_codepoint(text: &str, at: usize) -> usize {
    let d = decode_at(text, at);
    text.len().min(at + d.len.max(1))
}

/// Byte offset of the codepoint before `at`.
pub fn prev_codepoint(text: &str, at: usize) -> usize {
    if at == 0 {
        return 0;
    }
    let bytes = text.as_bytes();
    let mut i = at - 1;
    while i > 0 && (bytes[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    i
}

fn is_combining(cp: char) -> bool {
    let cp = cp as u32;
    (0x0300..=0x036F).contains(&cp)
        || (0x1AB0..=0x1AFF).contains(&cp)
        || (0x1DC0..=0x1DFF).contains(&cp)
        || (0x20D0..=0x20FF).contains(&cp)
        || (0xFE00..=0xFE0F).contains(&cp)
        || (0xFE20..=0xFE2F).contains(&cp)
        || (0xE0100..=0xE01EF).contains(&cp)
        || cp == 0x200D
}

fn is_regional_indicator(cp: char) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&(cp as u32))
}

fn is_emoji_modifier(cp: char) -> bool {
    (0x1F3FB..=0x1F3FF).contains(&(cp as u32))
}

fn is_extend(cp: char) -> bool {
    is_combining(cp) || is_emoji_modifier(cp) || cp == '\u{20E3}'
}

/// A pragmatic grapheme-cluster iterator. It handles combining marks, emoji
/// variation selectors, ZWJ sequences, keycaps and regional-indicator flags.
/// It is intentionally small rather than a full UAX #29 implementation, but
/// never splits a UTF-8 code point.
pub fn next_grapheme(text: &str, at: usize) -> usize {
    let len = text.len();
    if at >= len {
        return len;
    }
    let mut i = next_codepoint(text, at);
    let mut last_cp = '\u{10FFFF}';
    let mut ri_count: usize = 0;
    while i < len {
        let d = decode_at(text, i);
        if d.len == 0 {
            break;
        }
        let cp = d.codepoint;
        if is_extend(cp) {
            i += d.len;
            last_cp = cp;
            continue;
        }
        if last_cp == '\u{200D}' {
            i += d.len;
            last_cp = cp;
            continue;
        }
        if is_regional_indicator(cp) {
            if ri_count == 1 {
                i += d.len;
                ri_count = 0;
                last_cp = cp;
                continue;
            }
            if ri_count == 0 {
                ri_count = 1;
                i += d.len;
                last_cp = cp;
                continue;
            }
        }
        break;
    }
    i
}

/// Byte offset of the start of the grapheme before `at`.
pub fn prev_grapheme(text: &str, at: usize) -> usize {
    if at == 0 {
        return 0;
    }
    let mut i: usize = 0;
    let mut prev: usize = 0;
    while i < at {
        prev = i;
        i = next_grapheme(text, i);
    }
    prev
}

/// UTF-16 code units before `byte_offset`.
pub fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    let limit = byte_offset.min(text.len());
    let mut i: usize = 0;
    let mut units: usize = 0;
    while i < limit {
        let d = decode_at(text, i);
        if d.len == 0 || i + d.len > limit {
            break;
        }
        units += if (d.codepoint as u32) > 0xFFFF { 2 } else { 1 };
        i += d.len;
    }
    units
}

/// Byte offset for a UTF-16 code-unit offset.
pub fn utf16_to_byte(text: &str, unit_offset: usize) -> usize {
    let mut i: usize = 0;
    let mut units: usize = 0;
    while i < text.len() && units < unit_offset {
        let d = decode_at(text, i);
        if d.len == 0 {
            break;
        }
        let add: usize = if (d.codepoint as u32) > 0xFFFF { 2 } else { 1 };
        if units + add > unit_offset {
            break;
        }
        units += add;
        i += d.len;
    }
    i
}

/// Whether a codepoint belongs to a word for double-click / ⌥-arrow movement.
pub fn is_word_codepoint(cp: char) -> bool {
    let n = cp as u32;
    if n < 128 {
        return cp.is_ascii_alphanumeric() || n == '_' as u32;
    }
    !(n == 0x3000
        || (0x3001..=0x303F).contains(&n)
        || (0xFF01..=0xFF0F).contains(&n)
        || (0xFF1A..=0xFF20).contains(&n))
}

/// Start of the word around `at`.
pub fn word_start(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    while i > 0 {
        let p = prev_codepoint(text, i);
        let d = decode_at(text, p);
        if !is_word_codepoint(d.codepoint) {
            break;
        }
        i = p;
    }
    i
}

/// End of the word around `at`.
pub fn word_end(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    while i < text.len() {
        let d = decode_at(text, i);
        if !is_word_codepoint(d.codepoint) {
            break;
        }
        i += d.len;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_movement_never_splits_utf8() {
        let s = "a中e\u{0301}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let mut i: usize = 0;
        i = next_grapheme(s, i);
        assert_eq!(1, i);
        i = next_grapheme(s, i);
        assert_eq!(4, i);
        i = next_grapheme(s, i);
        assert_eq!(7, i);
        i = next_grapheme(s, i);
        assert_eq!(s.len(), i);
        assert_eq!(7, prev_grapheme(s, s.len()));
    }

    #[test]
    fn utf16_conversion() {
        let s = "A中😀";
        assert_eq!(1, byte_to_utf16(s, 1));
        assert_eq!(2, byte_to_utf16(s, 4));
        assert_eq!(4, byte_to_utf16(s, s.len()));
        assert_eq!(4, utf16_to_byte(s, 2));
        assert_eq!(s.len(), utf16_to_byte(s, 4));
    }

    /// An offset inside a sequence yields the raw continuation byte.
    #[test]
    fn decode_at_falls_back_to_the_raw_byte() {
        let s = "é"; // c3 a9
        let mid = decode_at(s, 1);
        assert_eq!('\u{00A9}', mid.codepoint);
        assert_eq!(1, mid.len);
        let end = decode_at(s, s.len());
        assert_eq!('\0', end.codepoint);
        assert_eq!(0, end.len);
    }

    /// Regression values for word boundaries in mixed Latin / CJK text.
    #[test]
    fn word_helpers_find_word_boundaries() {
        let s = "hello, 世界";
        assert_eq!(0, word_start(s, 3));
        assert_eq!(5, word_end(s, 0));
        assert!(!is_word_codepoint(' '));
        assert!(is_word_codepoint('世'));
    }
}
