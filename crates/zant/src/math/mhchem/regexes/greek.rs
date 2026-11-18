//! Greek letter and letter patterns.

use super::*;

pub(super) static GREEK_LOWER: [&[u8]; 24] = [
    b"alpha", b"beta", b"gamma", b"delta", b"epsilon", b"zeta", b"eta", b"theta", b"iota",
    b"kappa", b"lambda", b"mu", b"nu", b"xi", b"omicron", b"pi", b"rho", b"sigma", b"tau",
    b"upsilon", b"phi", b"chi", b"psi", b"omega",
];

pub(super) static GREEK_UPPER: [&[u8]; 11] = [
    b"Gamma", b"Delta", b"Theta", b"Lambda", b"Xi", b"Pi", b"Sigma", b"Upsilon", b"Phi", b"Psi",
    b"Omega",
];

/// `\\(?:alpha|beta|...)` at `i` (no name is a prefix of another, so the
/// first hit is the only one); returns the end index.
pub fn greek_macro_end(s: &[u8], i: usize, with_upper: bool) -> Option<usize> {
    if !char_at(s, i, b'\\') {
        return None;
    }
    for name in GREEK_LOWER {
        if starts_at(s, i + 1, name) {
            return Some(i + 1 + name.len());
        }
    }
    if with_upper {
        for name in GREEK_UPPER {
            if starts_at(s, i + 1, name) {
                return Some(i + 1 + name.len());
            }
        }
    }
    None
}

/// `(?:\s+|\{\}|(?![a-zA-Z]))` at `j`.
pub fn greek_suffix_end(s: &[u8], j: usize) -> Option<usize> {
    if space_len(s, j) > 0 {
        return Some(skip_spaces(s, j));
    }
    if starts_at(s, j, b"{}") {
        return Some(j + 2);
    }
    if j < s.len() && is_alpha(s[j]) {
        return None;
    }
    Some(j)
}

/// `(?:\s+|\{\}|(?![a-zA-Z]))$` at `j`.
pub fn greek_suffix_to_end(s: &[u8], j: usize) -> bool {
    if j == s.len() {
        return true;
    }
    if eql(&s[j..], b"{}") {
        return true;
    }
    skip_spaces(s, j) == s.len()
}

/// Character class of `letters`: `[a-zA-Zα-ωΑ-Ω?@]`.
pub fn is_letter_char(c: u32) -> bool {
    (c < 128 && (is_alpha(c as u8) || c == u32::from(b'?') || c == u32::from(b'@')))
        || (0x3B1..=0x3C9).contains(&c)
        || (0x391..=0x3A9).contains(&c)
}

/// `'letters'`: `/^(?:[a-zA-Zα-ωΑ-Ω?@]|(?:\\(?:alpha|...|Omega)(?:\s+|\{\}|(?![a-zA-Z]))))+/`
pub fn letters_end(s: &[u8]) -> Option<usize> {
    let mut i: usize = 0;
    while i < s.len() {
        let c = decode(s, i);
        if is_letter_char(c.c) {
            i += c.n;
            continue;
        }
        if let Some(j) = greek_macro_end(s, i, true)
            && let Some(k) = greek_suffix_end(s, j)
        {
            i = k;
            continue;
        }
        break;
    }
    if i == 0 { None } else { Some(i) }
}

/// `'one lowercase greek letter $'`:
/// `/^(?:\$?[α-ω]\$?|\$?\\(?:alpha|...|omega)\s*\$?)(?:\s+|\{\}|(?![a-zA-Z]))$/`
pub fn one_lowercase_greek_letter(s: &[u8]) -> bool {
    let mut d1: usize = 0;
    while d1 <= 1 {
        if d1 != 1 || char_at(s, 0, b'$') {
            if d1 < s.len() {
                let c = decode(s, d1);
                if (0x3B1..=0x3C9).contains(&c.c) {
                    let p = d1 + c.n;
                    if char_at(s, p, b'$') && greek_suffix_to_end(s, p + 1) {
                        return true;
                    }
                    if greek_suffix_to_end(s, p) {
                        return true;
                    }
                }
            }
            if let Some(j) = greek_macro_end(s, d1, false) {
                let mut e = j;
                loop {
                    if char_at(s, e, b'$') && greek_suffix_to_end(s, e + 1) {
                        return true;
                    }
                    if greek_suffix_to_end(s, e) {
                        return true;
                    }
                    let n = space_len(s, e);
                    if n == 0 {
                        break;
                    }
                    e += n;
                }
            }
        }
        d1 += 1;
    }
    false
}
