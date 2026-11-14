//! JavaScript string methods for generated TSX code (`src/tsx_strings.zig`).
//!
//! TSX strings are [`zant::Str`](crate::Str) values; positions are UTF-16 code
//! units like in JavaScript, storage is UTF-8. Unlike the Zig version (whose
//! results belonged to the current UI frame), every function here returns an
//! owned `Str`/`List<Str>`, so generated code may store or clone it freely.
//!
//! Semantics kept 1:1 with the Zig port: `slice` / `substring` / `charAt` /
//! `at` round a boundary inside a surrogate pair down, `padStart` / `padEnd`
//! count UTF-16 units, `replace` implements JS's `$&` / `` $` `` / `$'` / `$$`
//! substitution tokens, `split("")` splits by code point, and invalid ranges
//! panic like the Zig `@panic` calls.

use crate::Str;
use crate::core::unicode::{self, decode_at};
use crate::tsx::{List, utf16_length};

/// Allocates a copy of `s` as a `Str` (Zig's frame copy).
fn copy(s: &str) -> Str {
    Str::copy(s)
}

/// `for (const ch of s)`: the code points of `s` as strings (JS iterates a
/// string by code point).
pub fn chars(s: &str) -> List<Str> {
    let mut items = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let next = unicode::next_codepoint(s, i);
        items.push(copy(&s[i..next]));
        i = next;
    }
    List::from_vec(items)
}

/// `String.prototype.includes`.
pub fn includes(s: &str, needle: &str) -> bool {
    s.contains(needle)
}

/// `String.prototype.startsWith`.
pub fn starts_with(s: &str, needle: &str) -> bool {
    s.starts_with(needle)
}

/// `String.prototype.endsWith`.
pub fn ends_with(s: &str, needle: &str) -> bool {
    s.ends_with(needle)
}

/// `String.prototype.indexOf`: the first match in UTF-16 units, or -1 (the
/// same signed type as array `indexOf`).
pub fn index_of(s: &str, needle: &str) -> isize {
    match s.find(needle) {
        Some(byte) => unicode::byte_to_utf16(s, byte) as isize,
        None => -1,
    }
}

/// `String.prototype.lastIndexOf`: the last match in UTF-16 units, or -1 (an
/// empty pattern matches the end).
pub fn last_index_of(s: &str, needle: &str) -> isize {
    match s.rfind(needle) {
        Some(byte) => unicode::byte_to_utf16(s, byte) as isize,
        None => -1,
    }
}

/// ECMAScript WhiteSpace and LineTerminator code points (U+0085 NEL and
/// U+200B zero width space are *not* whitespace).
fn whitespace(cp: char) -> bool {
    matches!(
        cp,
        '\u{09}'..='\u{0d}' | '\u{20}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
            | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

fn trimmed(s: &str, start: bool, end: bool) -> Str {
    let mut lo = 0;
    let mut hi = s.len();
    if start {
        while lo < hi {
            let decoded = decode_at(s, lo);
            if !whitespace(decoded.codepoint) {
                break;
            }
            lo += decoded.len;
        }
    }
    if end {
        while hi > lo {
            let previous = unicode::prev_codepoint(s, hi);
            if !whitespace(decode_at(s, previous).codepoint) {
                break;
            }
            hi = previous;
        }
    }
    copy(&s[lo..hi])
}

/// `String.prototype.trim`.
pub fn trim(s: &str) -> Str {
    trimmed(s, true, true)
}

/// `String.prototype.trimStart`.
pub fn trim_start(s: &str) -> Str {
    trimmed(s, true, false)
}

/// `String.prototype.trimEnd`.
pub fn trim_end(s: &str) -> Str {
    trimmed(s, false, true)
}

fn ascii_case(s: &str, upper: bool) -> Str {
    let bytes = s
        .bytes()
        .map(|byte| {
            if upper {
                byte.to_ascii_uppercase()
            } else {
                byte.to_ascii_lowercase()
            }
        })
        .collect();
    Str::copy(&String::from_utf8(bytes).expect("ASCII case mapping keeps UTF-8"))
}

/// `String.prototype.toLowerCase`: ASCII only, other UTF-8 bytes unchanged
/// (like the Zig port).
pub fn to_lower_case(s: &str) -> Str {
    ascii_case(s, false)
}

/// `String.prototype.toUpperCase`: ASCII only, other UTF-8 bytes unchanged.
pub fn to_upper_case(s: &str) -> Str {
    ascii_case(s, true)
}

/// `String.prototype.slice`'s `ToIntegerOrInfinity` clamping: NaN or a
/// negative value is 0, otherwise the truncation, at most `length`.
fn clamp(value: f64, length: usize) -> usize {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    value.trunc().min(length as f64) as usize
}

fn range(s: &str, lo: usize, hi: usize) -> Str {
    copy(&s[unicode::utf16_to_byte(s, lo)..unicode::utf16_to_byte(s, hi)])
}

/// `String.prototype.slice`: negative offsets count from the end; both
/// endpoints round down inside surrogate pairs.
pub fn slice(s: &str, start: f64, end: Option<f64>) -> Str {
    let len = utf16_length(s);
    let lo = crate::tsx::slice_index(start.trunc(), len);
    let hi = end.map_or(len, |value| crate::tsx::slice_index(value.trunc(), len));
    range(s, lo, lo.max(hi))
}

/// `String.prototype.substring`: negatives clamp to zero, reversed endpoints
/// swap, boundaries round down.
pub fn substring(s: &str, start: f64, end: Option<f64>) -> Str {
    let len = utf16_length(s);
    let a = clamp(start, len);
    let b = end.map_or(len, |value| clamp(value, len));
    range(s, a.min(b), a.max(b))
}

fn checked_length(n: f64) -> usize {
    if !n.is_finite() || n >= usize::MAX as f64 {
        panic!("TSX string length out of range");
    }
    n.trunc().max(0.0) as usize
}

fn multiply(a: usize, b: usize) -> usize {
    a.checked_mul(b).expect("TSX string length out of range")
}

fn pad(s: &str, target: f64, fill: &str, start: bool) -> Str {
    let len = utf16_length(s);
    if target.is_nan() || target <= len as f64 || fill.is_empty() {
        return copy(s);
    }
    let needed = checked_length(target) - len;
    let units = utf16_length(fill);
    let full = needed / units;
    let tail = unicode::utf16_to_byte(fill, needed % units);
    let bytes = full
        .checked_mul(fill.len())
        .expect("TSX string length out of range")
        + tail;
    let mut out = String::with_capacity(s.len() + bytes);
    if start {
        for _ in 0..full {
            out.push_str(fill);
        }
        out.push_str(&fill[..tail]);
        out.push_str(s);
    } else {
        out.push_str(s);
        for _ in 0..full {
            out.push_str(fill);
        }
        out.push_str(&fill[..tail]);
    }
    Str::from(out)
}

/// `String.prototype.padStart`: pad to a UTF-16 length; a fill truncated inside
/// a surrogate pair rounds down.
pub fn pad_start(s: &str, target: f64, fill: &str) -> Str {
    pad(s, target, fill, true)
}

/// `String.prototype.padEnd` (same boundary policy as [`pad_start`]).
pub fn pad_end(s: &str, target: f64, fill: &str) -> Str {
    pad(s, target, fill, false)
}

/// `String.prototype.repeat`: a truncated count; invalid JS RangeError counts
/// panic (like the Zig runtime).
pub fn repeat(s: &str, count: f64) -> Str {
    let n = if count.is_nan() { 0.0 } else { count.trunc() };
    if n < 0.0 || !n.is_finite() {
        panic!("TSX repeat count out of range");
    }
    if s.is_empty() || n == 0.0 {
        return copy("");
    }
    let times = checked_length(n);
    let _ = multiply(s.len(), times); // out-of-range check, like Zig
    Str::from(s.repeat(times))
}

// JS GetSubstitution for string patterns: $$, $&, $`, $'; capture tokens stay
// literal.
fn substitution(out: &mut String, s: &str, lo: usize, hi: usize, replacement: &str) {
    let mut i = 0;
    while i < replacement.len() {
        if replacement.as_bytes()[i] == b'$' && i + 1 < replacement.len() {
            let next = replacement[i + 1..]
                .chars()
                .next()
                .expect("index i + 1 is a char boundary");
            let part: Option<&str> = match next {
                '$' => Some("$"),
                '&' => Some(&s[lo..hi]),
                '`' => Some(&s[..lo]),
                '\'' => Some(&s[hi..]),
                _ => None,
            };
            if let Some(value) = part {
                out.push_str(value);
                i += 1 + next.len_utf8();
                continue;
            }
        }
        let character = replacement[i..]
            .chars()
            .next()
            .expect("i < replacement.len()");
        out.push(character);
        i += character.len_utf8();
    }
}

fn replaced(s: &str, pattern: &str, replacement: &str, all: bool) -> Str {
    let mut out = String::new();
    let mut offset = 0;
    if pattern.is_empty() {
        // Enumerate UTF-16 boundaries, rounding each boundary down like slice.
        substitution(&mut out, s, 0, 0, replacement);
        if all {
            while offset < s.len() {
                let decoded = decode_at(s, offset);
                if decoded.codepoint as u32 > 0xffff {
                    substitution(&mut out, s, offset, offset, replacement);
                }
                out.push_str(&s[offset..offset + decoded.len]);
                offset += decoded.len;
                substitution(&mut out, s, offset, offset, replacement);
            }
        }
    } else {
        while let Some(found) = s[offset..].find(pattern) {
            let at = offset + found;
            out.push_str(&s[offset..at]);
            substitution(&mut out, s, at, at + pattern.len(), replacement);
            offset = at + pattern.len();
            if !all {
                break;
            }
        }
    }
    out.push_str(&s[offset..]);
    Str::from(out)
}

/// `String.prototype.replace` (first match) with JS replacement dollar tokens.
pub fn replace(s: &str, pattern: &str, replacement: &str) -> Str {
    replaced(s, pattern, replacement, false)
}

/// `String.prototype.replaceAll` (non-overlapping matches, including empty
/// patterns).
pub fn replace_all(s: &str, pattern: &str, replacement: &str) -> Str {
    replaced(s, pattern, replacement, true)
}

/// `String.prototype.split` on a literal separator; an empty separator splits
/// by Unicode code point.
pub fn split(s: &str, separator: &str) -> List<Str> {
    let mut parts = Vec::new();
    if separator.is_empty() {
        let mut offset = 0;
        while offset < s.len() {
            let end = unicode::next_codepoint(s, offset);
            parts.push(copy(&s[offset..end]));
            offset = end;
        }
    } else {
        let mut offset = 0;
        while let Some(found) = s[offset..].find(separator) {
            let at = offset + found;
            parts.push(copy(&s[offset..at]));
            offset = at + separator.len();
        }
        parts.push(copy(&s[offset..]));
    }
    List::from_vec(parts)
}

fn character(s: &str, position: f64, negative: bool) -> Option<Str> {
    let n = if position.is_nan() {
        0.0
    } else {
        position.trunc()
    };
    let len = utf16_length(s) as f64;
    let i = if negative && n < 0.0 { len + n } else { n };
    if i < 0.0 || i >= len {
        return None;
    }
    let unit = i as usize;
    Some(range(s, unit, unit + 1))
}

/// `String.prototype.charAt`: one UTF-16 unit using rounded UTF-8 boundaries;
/// absent positions give "".
pub fn char_at(s: &str, position: f64) -> Str {
    character(s, position, false).unwrap_or_else(|| copy(""))
}

/// `String.prototype.at`: negative positions count from the end, absent
/// positions are `None` (TS `undefined`).
pub fn at(s: &str, position: f64) -> Option<Str> {
    character(s, position, true)
}

/// `String.prototype.charCodeAt`: the UTF-16 code unit at `position`, NaN
/// outside the string (like JS).
pub fn char_code_at(s: &str, position: f64) -> f64 {
    match utf16_unit(s, position) {
        Some(unit) => f64::from(unit),
        None => f64::NAN,
    }
}

/// `String.prototype.codePointAt`: the code point starting at UTF-16
/// `position` (a trailing surrogate's own value in the middle of a pair, like
/// JS); `None` outside the string.
pub fn code_point_at(s: &str, position: f64) -> Option<f64> {
    let unit = utf16_unit(s, position)?;
    if !(0xd800..=0xdbff).contains(&unit) {
        return Some(f64::from(unit));
    }
    let low = utf16_unit(s, position.trunc() + 1.0)?;
    if !(0xdc00..=0xdfff).contains(&low) {
        return Some(f64::from(unit));
    }
    Some(f64::from(
        0x10000 + ((u32::from(unit) - 0xd800) << 10) + (u32::from(low) - 0xdc00),
    ))
}

fn utf16_unit(s: &str, position: f64) -> Option<u16> {
    if position.is_nan() {
        return utf16_unit(s, 0.0);
    }
    let p = position.trunc();
    if p < 0.0 {
        return None;
    }
    let mut index: f64 = 0.0;
    for character in s.chars() {
        let cp = character as u32;
        if cp >= 0x10000 {
            let value = cp - 0x10000;
            if index == p {
                return u16::try_from(0xd800 + (value >> 10)).ok();
            }
            if index + 1.0 == p {
                return u16::try_from(0xdc00 + (value & 0x3ff)).ok();
            }
            index += 2.0;
        } else {
            if index == p {
                return u16::try_from(cp).ok();
            }
            index += 1.0;
        }
    }
    None
}

#[cfg(test)]
#[path = "strings_tests.rs"]
mod tests;
