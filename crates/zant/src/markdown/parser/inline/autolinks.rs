//! Bare URL and email autolink detection.

use crate::markdown::scan;

/// Bare autolink (GFM): http://, https://, www., or an email address.
pub(super) fn bare_autolink_end(text: &str, i: usize) -> Option<usize> {
    let rest = &text[i..];
    let is_url =
        rest.starts_with("http://") || rest.starts_with("https://") || rest.starts_with("www.");
    if !is_url {
        return None;
    }
    if i > 0 && scan::is_word_cp(scan::cp_before(text, i)) {
        return None;
    }
    let mut end = i;
    let mut parens: isize = 0;
    while end < text.len() {
        let c = text.as_bytes()[end];
        if c <= b' ' || c == b'<' || c == b'>' || c == b'"' || c == b'`' {
            break;
        }
        // CJK punctuation ends an autolink (Streamdown CJK plugin)
        if c >= 0x80 {
            let cp = scan::cp_at(text, end);
            if scan::is_unicode_punct(cp) || scan::is_cjk(cp) {
                break;
            }
            end += scan::utf8_byte_sequence_length_or_1(c);
            continue;
        }
        if c == b'(' {
            parens += 1;
        }
        if c == b')' {
            if parens == 0 {
                break;
            }
            parens -= 1;
        }
        end += 1;
    }
    while end > i && b".,:;!?*_~'".contains(&text.as_bytes()[end - 1]) {
        end -= 1;
    }
    let min_len: usize = if rest.starts_with("www.") { 5 } else { 9 };
    if end - i < min_len {
        return None;
    }
    Some(end)
}

pub(super) fn email_autolink(text: &str, at_index: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut start = at_index;
    while start > 0 {
        let c = bytes[start - 1];
        if c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-' || c == b'+' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == at_index {
        return None;
    }
    let mut end = at_index + 1;
    let mut dot = false;
    while end < text.len() {
        let c = bytes[end];
        if c == b'.' {
            dot = true;
        } else if !(c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
            break;
        }
        end += 1;
    }
    while end > at_index + 1 && bytes[end - 1] == b'.' {
        end -= 1;
    }
    if !dot || end <= at_index + 1 || bytes[end - 1] == b'-' {
        return None;
    }
    Some((start, end))
}
