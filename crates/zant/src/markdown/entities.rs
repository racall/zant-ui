//! Port of `src/markdown/entities.zig` — WP-3.2.
//!
//! HTML character reference decoding (`&amp;`, `&#169;`, `&#x1F600;`).

use std::borrow::Cow;

const NAMED: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{00A0}"),
    ("copy", "\u{00A9}"),
    ("reg", "\u{00AE}"),
    ("trade", "\u{2122}"),
    ("hellip", "\u{2026}"),
    ("mdash", "\u{2014}"),
    ("ndash", "\u{2013}"),
    ("lsquo", "\u{2018}"),
    ("rsquo", "\u{2019}"),
    ("ldquo", "\u{201C}"),
    ("rdquo", "\u{201D}"),
    ("laquo", "\u{00AB}"),
    ("raquo", "\u{00BB}"),
    ("middot", "\u{00B7}"),
    ("bull", "\u{2022}"),
    ("deg", "\u{00B0}"),
    ("plusmn", "\u{00B1}"),
    ("times", "\u{00D7}"),
    ("divide", "\u{00F7}"),
    ("ne", "\u{2260}"),
    ("le", "\u{2264}"),
    ("ge", "\u{2265}"),
    ("larr", "\u{2190}"),
    ("rarr", "\u{2192}"),
    ("uarr", "\u{2191}"),
    ("darr", "\u{2193}"),
    ("harr", "\u{2194}"),
    ("euro", "\u{20AC}"),
    ("pound", "\u{00A3}"),
    ("yen", "\u{00A5}"),
    ("cent", "\u{00A2}"),
    ("sect", "\u{00A7}"),
    ("para", "\u{00B6}"),
    ("frac12", "\u{00BD}"),
    ("frac14", "\u{00BC}"),
    ("frac34", "\u{00BE}"),
    ("alpha", "\u{03B1}"),
    ("beta", "\u{03B2}"),
    ("gamma", "\u{03B3}"),
    ("delta", "\u{03B4}"),
    ("pi", "\u{03C0}"),
    ("sigma", "\u{03C3}"),
    ("mu", "\u{03BC}"),
    ("infin", "\u{221E}"),
    ("check", "\u{2713}"),
    ("zwj", "\u{200D}"),
    ("zwnj", "\u{200C}"),
    ("emsp", "\u{2003}"),
    ("ensp", "\u{2002}"),
    ("thinsp", "\u{2009}"),
];

pub fn lookup(name: &str) -> Option<&'static str> {
    NAMED
        .iter()
        .find(|entry| entry.0 == name)
        .map(|entry| entry.1)
}

/// Decodes one reference starting at `text[0] == '&'`. Returns the decoded
/// UTF-8 bytes (written to `buf`) and the consumed length.
pub fn decode_at<'b>(text: &str, buf: &'b mut [u8; 4]) -> Option<(&'b str, usize)> {
    if text.len() < 3 || text.as_bytes()[0] != b'&' {
        return None;
    }
    let limit = text.len().min(34);
    let semi = text.as_bytes()[..limit].iter().position(|c| *c == b';')?;
    let body = &text.as_bytes()[1..semi];
    if body.is_empty() {
        return None;
    }
    if body[0] == b'#' {
        let hex = body.len() > 1 && (body[1] == b'x' || body[1] == b'X');
        let digits = if hex { &body[2..] } else { &body[1..] };
        if digits.is_empty() || digits.len() > 7 {
            return None;
        }
        let digits = std::str::from_utf8(digits).ok()?;
        let code = u32::from_str_radix(digits, if hex { 16 } else { 10 }).ok()?;
        // Zig parses into `u21`; values above its max are a parse error.
        if code > 0x1F_FFFF {
            return None;
        }
        let cp: u32 = if code == 0 || code > 0x10FFFF || (0xD800..=0xDFFF).contains(&code) {
            0xFFFD
        } else {
            code
        };
        let c = char::from_u32(cp).unwrap_or('\u{FFFD}');
        let n = c.encode_utf8(buf).len();
        let value = std::str::from_utf8(&buf[..n]).expect("utf8Encode wrote valid UTF-8");
        return Some((value, semi + 1));
    }
    if !body.iter().all(u8::is_ascii_alphanumeric) {
        return None;
    }
    let value = lookup(std::str::from_utf8(body).ok()?)?;
    Some((value, semi + 1))
}

/// Returns `text` with all character references decoded. Returns `text`
/// itself when nothing needs decoding.
pub fn decode(text: &str) -> Cow<'_, str> {
    if !text.as_bytes().contains(&b'&') {
        return Cow::Borrowed(text);
    }
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4];
    let mut i: usize = 0;
    while i < text.len() {
        if bytes[i] == b'&'
            && let Some((value, len)) = decode_at(&text[i..], &mut buf)
        {
            out.extend_from_slice(value.as_bytes());
            i += len;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    Cow::Owned(String::from_utf8(out).expect("character references preserve UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_and_numeric_references() {
        let out = decode("a &amp; b &#169; &#x4E2D; &bogus;");
        assert_eq!(out, "a & b \u{00A9} \u{4E2D} &bogus;");
    }
}
