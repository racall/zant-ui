//! JSX text: React's whitespace rule and HTML entities.
//!
//! Inputs and outputs are bytes, not `String`: decoded entities may be WTF-8
//! (lone surrogates and `\xNN` escapes), which Rust's UTF-8 strings cannot
//! hold.

use crate::jsx_entities;

/// React's JSX text rule (Babel `cleanJSXElementLiteralChild`): lines are
/// trimmed except at the outer edges, whitespace-only lines vanish and the
/// remaining lines join with one space. Null when nothing is left.
pub fn clean_jsx_text(value: &[u8]) -> Option<Vec<u8>> {
    let mut lines: Vec<&[u8]> = Vec::new();
    for line in value.split(|&b| b == b'\n') {
        lines.push(trim_end_set(line, b"\r"));
    }
    let mut last_non_empty: usize = 0; // Babel's default
    for (i, line) in lines.iter().enumerate() {
        if line.iter().any(|&b| b != b' ' && b != b'\t') {
            last_non_empty = i;
        }
    }
    let mut out: Vec<u8> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut trimmed: &[u8] = line;
        if i != 0 {
            trimmed = trim_start_set(trimmed, b" \t");
        }
        if i != lines.len() - 1 {
            trimmed = trim_end_set(trimmed, b" \t");
        }
        if trimmed.is_empty() {
            continue;
        }
        for &c in trimmed {
            out.push(if c == b'\t' { b' ' } else { c });
        }
        if i != last_non_empty {
            out.push(b' ');
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn trim_start_set<'a>(mut value: &'a [u8], set: &[u8]) -> &'a [u8] {
    while let [first, rest @ ..] = value {
        if !set.contains(first) {
            break;
        }
        value = rest;
    }
    value
}

fn trim_end_set<'a>(mut value: &'a [u8], set: &[u8]) -> &'a [u8] {
    while let [rest @ .., last] = value {
        if !set.contains(last) {
            break;
        }
        value = rest;
    }
    value
}

struct Entity {
    cp: u32,
    len: usize,
}

/// Decode one reference at the start, following SWC's `decode_jsx_entity`.
fn entity(raw: &[u8]) -> Option<Entity> {
    if raw.len() < 3 || raw[0] != b'&' {
        return None;
    }
    let mut i: usize = 1;
    if raw[i] == b'#' {
        i += 1;
        let radix: u32 = if i < raw.len() && raw[i] == b'x' {
            i += 1;
            16
        } else {
            10
        };
        let start = i;
        let mut value: u32 = 0;
        while i < raw.len() {
            let ch = raw[i];
            if ch == b';' && i > start {
                return Some(Entity {
                    cp: value,
                    len: i + 1,
                });
            }
            let digit: u32 = match ch {
                b'0'..=b'9' => (ch - b'0') as u32,
                b'a'..=b'f' if radix == 16 => (ch - b'a' + 10) as u32,
                b'A'..=b'F' if radix == 16 => (ch - b'A' + 10) as u32,
                _ => return None,
            };
            // Values cannot shrink: this also prevents integer overflow on
            // arbitrarily long inputs.
            if value > (0x10ffff - digit) / radix {
                return None;
            }
            value = value * radix + digit;
            i += 1;
        }
        return None;
    }
    while i < raw.len() && raw[i].is_ascii_alphanumeric() {
        i += 1;
    }
    if i >= raw.len() || raw[i] != b';' {
        return None;
    }
    let cp = jsx_entities::get(&raw[1..i])?;
    Some(Entity {
        cp: cp as u32,
        len: i + 1,
    })
}

/// XHTML entities in JSX text and attributes. Invalid references stay literal.
/// Like SWC's JS strings, lone surrogates use WTF-8; adjacent pairs combine.
// The nested `if`s keep one condition per step; clippy would fold them.
#[allow(clippy::collapsible_if)]
pub fn decode_entities(raw: &[u8]) -> Vec<u8> {
    if !raw.contains(&b'&') {
        return raw.to_vec();
    }
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut i: usize = 0;
    while i < raw.len() {
        if let Some(decoded) = entity(&raw[i..]) {
            i += decoded.len;
            let mut cp = decoded.cp;
            if (0xd800..=0xdbff).contains(&cp) {
                if let Some(next) = entity(&raw[i..]) {
                    if (0xdc00..=0xdfff).contains(&next.cp) {
                        cp = 0x10000 + ((cp - 0xd800) << 10) + (next.cp - 0xdc00);
                        i += next.len;
                    }
                }
            }
            push_wtf8(&mut out, cp);
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    out
}

/// `std.unicode.wtf8Encode`: UTF-8 that also encodes lone surrogates.
fn push_wtf8(out: &mut Vec<u8>, cp: u32) {
    if cp < 0x80 {
        out.push(cp as u8);
    } else if cp < 0x800 {
        out.push(0xC0 | (cp >> 6) as u8);
        out.push(0x80 | (cp & 0x3F) as u8);
    } else if cp < 0x10000 {
        out.push(0xE0 | (cp >> 12) as u8);
        out.push(0x80 | ((cp >> 6) & 0x3F) as u8);
        out.push(0x80 | (cp & 0x3F) as u8);
    } else {
        out.push(0xF0 | (cp >> 18) as u8);
        out.push(0x80 | ((cp >> 12) & 0x3F) as u8);
        out.push(0x80 | ((cp >> 6) & 0x3F) as u8);
        out.push(0x80 | (cp & 0x3F) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_decoded(raw: &[u8], expected: &[u8]) {
        assert_eq!(decode_entities(raw).as_slice(), expected);
    }

    #[test]
    fn jsx_text_whitespace_follows_react() {
        assert_eq!(clean_jsx_text(b"Count: ").unwrap().as_slice(), b"Count: ");
        assert_eq!(
            clean_jsx_text(b"\n    Hello\n    world\n  ")
                .unwrap()
                .as_slice(),
            b"Hello world"
        );
        assert_eq!(clean_jsx_text(b"\n    \n  "), None);
        assert_eq!(clean_jsx_text(b" a b ").unwrap().as_slice(), b" a b ");
        assert_eq!(
            decode_entities(b"a &copy; b").as_slice(),
            "a © b".as_bytes()
        );
        assert_eq!(decode_entities(b"&lt;&amp;&#35;&gt;").as_slice(), b"<&#>");
        assert_eq!(
            decode_entities(b"&#123;x&#125;").as_slice(),
            b"{x}" // what the `}` error suggests
        );
    }

    #[test]
    fn xhtml_named_references_cover_latin_greek_symbols_and_special_characters() {
        expect_decoded(
            b"&copy; 2026 &mdash; &Alpha; &alpha; &euro; &hearts; &OElig; &AElig;",
            "© 2026 — Α α € ♥ Œ Æ".as_bytes(),
        );
        expect_decoded(
            b"&amp;&lt;&gt;&quot;&apos;&nbsp;&ensp;&zwnj;&forall;&frac12;",
            "&<>\"'\u{a0}\u{2002}\u{200c}∀½".as_bytes(),
        );
        expect_decoded(
            b"&AMP; &Copy; &unknown; &lbrace; &rbrace;",
            b"&AMP; &Copy; &unknown; &lbrace; &rbrace;",
        );
    }

    #[test]
    fn decimal_and_lowercase_x_hexadecimal_numeric_references() {
        expect_decoded(b"&#169; 2026 &#8212; &#128512;", "© 2026 — 😀".as_bytes());
        expect_decoded(
            b"&#xA9; &#x00a9; &#x1F600; &#x10FFFF;",
            "© © 😀 \u{10ffff}".as_bytes(),
        );
        expect_decoded(
            b"&#0;|&#x0;|&#1114111;",
            "\u{0}|\u{0}|\u{10ffff}".as_bytes(),
        );
        expect_decoded(
            b"&#000000000000000000000000000000065; &#x00000000000000041;",
            b"A A",
        );
    }

    #[test]
    fn invalid_numeric_and_non_alphanumeric_named_references_stay_literal() {
        const INVALID: [&[u8]; 19] = [
            b"&#;",
            b"&#x;",
            b"&#X41;",
            b"&#xg;",
            b"&#12a;",
            b"&#-1;",
            b"&#+65;",
            b"&#x+41;",
            b"&# 65;",
            b"&#1_0;",
            b"&#0x41;",
            b"&#x110000;",
            b"&#1114112;",
            b"&#99999999999999999999999999999;",
            b"&co_py;",
            b"&co-py;",
            b"&co py;",
            b"&;",
            b"&\xc3\xa9;",
        ];
        for raw in INVALID {
            expect_decoded(raw, raw);
        }
        expect_decoded(
            b"&bad&copy; &#x&copy; &amp;&copy;",
            "&bad© &#x© &©".as_bytes(),
        );
    }

    #[test]
    fn unterminated_references_and_ampersands_stay_literal() {
        const CASES: [&[u8]; 8] = [
            b"&",
            b"&amp",
            b"&#",
            b"&#12",
            b"&#x",
            b"&#x41",
            b"a & b",
            b"&copy rest;",
        ];
        for raw in CASES {
            expect_decoded(raw, raw);
        }
        // one pass, never recursively decode
        expect_decoded(b"&amp;copy;", b"&copy;");
    }

    #[test]
    fn surrogates_follow_swc_javascript_code_points_pairing_only_adjacent_halves() {
        expect_decoded(
            b"&#xD800;|&#55296;|&#xDFFF;|&#57343;",
            b"\xed\xa0\x80|\xed\xa0\x80|\xed\xbf\xbf|\xed\xbf\xbf",
        );
        expect_decoded(
            b"&#xD83D;&#56832; &#55296;&#xDC00; &#xDBFF;&#xDFFF;",
            "😀 \u{10000} \u{10ffff}".as_bytes(),
        );
        expect_decoded(b"&#xD800; &#xDC00;", b"\xed\xa0\x80 \xed\xb0\x80");
        expect_decoded(b"&#xDC00;&#xD800;", b"\xed\xb0\x80\xed\xa0\x80");
        expect_decoded(
            b"&#xD800;&#xD83D;&#xDE00;",
            &[b"\xed\xa0\x80".as_slice(), "😀".as_bytes()].concat(),
        );
    }
}
