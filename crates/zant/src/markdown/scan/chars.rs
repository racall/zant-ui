//! Code point helpers: UTF-8 decoding around an index and CommonMark character classes.

use super::*;

/// UTF-8 sequence length implied by a leading byte, `None` when the byte is
/// not a valid leading byte.
pub fn utf8_seq_len(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7F => Some(1),
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
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

/// Decodes a slice that must be exactly one codepoint.
pub(super) fn decode_exact(bytes: &[u8]) -> Option<u32> {
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
