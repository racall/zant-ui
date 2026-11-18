//! Hand-written matchers for every regexp of `mhchemParser.patterns.patterns`
//! that is not a trivial literal. Each one reproduces the JS sticky regexp's
//! boolean/full-match semantics by hand.

use super::chars::{
    PosSet, any_at, char_at, decode, digit_at, digits_end, eql, is_alpha, is_close_paren, is_digit,
    is_lower, is_open_paren, is_roman, is_sign, is_upper, lower_at, skip_spaces, space_len,
    starts_at,
};
use super::types::Str;

mod greek;
mod numbers;

pub use self::greek::*;
pub use self::numbers::*;

/// `'formula$'` regexp `/^(?:[a-z]|(?:[0-9\ \+\-\,\.\(\)]+[a-z])+[0-9\ \+\-\,\.\(\)]*|(?:[a-z][0-9\ \+\-\,\.\(\)]+)+[a-z]?)$/`
/// is exactly: only `[a-z]` and `[0-9 +\-,.()]`, at least one letter, no
/// two letters adjacent.
pub fn is_formula(s: &[u8]) -> bool {
    let mut letters: usize = 0;
    let mut prev_letter = false;
    for &c in s {
        if is_lower(c) {
            if prev_letter {
                return false;
            }
            prev_letter = true;
            letters += 1;
        } else if is_digit(c) || b" +-,.()".contains(&c) {
            prev_letter = false;
        } else {
            return false;
        }
    }
    letters > 0
}

/// `'orbital'`: `/^(?:[0-9]{1,2}[spdfgh]|[0-9]{0,2}sp)(?=$|[^a-zA-Z])/`
pub fn orbital_end(s: &[u8]) -> Option<usize> {
    let run: usize = usize::min(digits_end(s, 0), 2);
    let mut k = run;
    while k >= 1 {
        if any_at(s, k, b"spdfgh") && not_letter_or_end(s, k + 1) {
            return Some(k + 1);
        }
        k -= 1;
    }
    k = run;
    loop {
        if starts_at(s, k, b"sp") && not_letter_or_end(s, k + 2) {
            return Some(k + 2);
        }
        if k == 0 {
            break;
        }
        k -= 1;
    }
    None
}

/// `(?=$|[^a-zA-Z])` at `i`.
pub fn not_letter_or_end(s: &[u8], i: usize) -> bool {
    i >= s.len() || !is_alpha(s[i])
}

/// `(?=$|[\s,;\)\]\}])` at `i`.
pub fn delimiter_or_end(s: &[u8], i: usize) -> bool {
    i >= s.len() || space_len(s, i) > 0 || any_at(s, i, b",;)]}")
}

/// `'oxidation$'` (`signed`) / `'d-oxidation$'`:
/// `/^(?:[+-][IVX]+|\\pm\s*0|\$\\pm\$\s*0)$/`, `/^(?:[+-]?\s?[IVX]+|\\pm\s*0|\$\\pm\$\s*0)$/`
pub fn is_oxidation(s: &[u8], d_variant: bool) -> bool {
    'roman: {
        let mut i: usize = 0;
        if !s.is_empty() && is_sign(s[0]) {
            i = 1;
        } else if !d_variant {
            break 'roman;
        }
        if d_variant {
            i += space_len(s, i);
        }
        if i >= s.len() {
            break 'roman;
        }
        for &c in &s[i..] {
            if !is_roman(c) {
                break 'roman;
            }
        }
        return true;
    }
    for pm in [&b"\\pm"[..], &b"$\\pm$"[..]] {
        if starts_at(s, 0, pm) {
            let j = skip_spaces(s, pm.len());
            if j + 1 == s.len() && s[j] == b'0' {
                return true;
            }
        }
    }
    false
}
