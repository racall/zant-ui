//! Number patterns: literals, exponents, `\pu` numbers, amounts and fractions.

use super::*;

/// Full match of `[0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+)` (`optional_frac`
/// selects the `(?:\.[0-9]+)?` variant of the second alternative).
pub fn is_number_literal(s: &[u8], optional_frac: bool) -> bool {
    // alternative 1
    if !s.is_empty() && is_digit(s[0]) {
        let mut i = digits_end(s, 0);
        if i == s.len() {
            return true;
        }
        if (s[i] == b',' || s[i] == b'.') && digit_at(s, i + 1) {
            i = digits_end(s, i + 1);
            if i == s.len() {
                return true;
            }
        }
    }
    // alternative 2
    let i = digits_end(s, 0);
    if i == s.len() {
        return optional_frac;
    }
    if s[i] == b'.' && digit_at(s, i + 1) {
        return digits_end(s, i + 1) == s.len();
    }
    false
}

/// `[+\-]?[0-9]+|\{[+\-]?[0-9]+\}` at `i`; returns the end index.
pub fn exponent_end(s: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    if j < s.len() && is_sign(s[j]) {
        j += 1;
    }
    if digit_at(s, j) {
        return Some(digits_end(s, j));
    }
    if char_at(s, i, b'{') {
        j = i + 1;
        if j < s.len() && is_sign(s[j]) {
            j += 1;
        }
        if !digit_at(s, j) {
            return None;
        }
        j = digits_end(s, j);
        if char_at(s, j, b'}') {
            return Some(j + 1);
        }
    }
    None
}

/// `'(-)(9.,9)(e)(99)'` regexp:
/// `/^(\+\-|\+\/\-|\+|\-|\\pm\s?)?([0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+))?(\((?:[0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+))\))?(?:([eE]|\s*(\*|x|\\times|×)\s*10\^)([+\-]?[0-9]+|\{[+\-]?[0-9]+\}))?/`
/// Returns the end index and groups 1..6 ("" for groups that did not participate).
pub fn enumber_re(s: &[u8], g: &mut [Str; 6]) -> usize {
    for x in g.iter_mut() {
        x.clear();
    }
    let mut i: usize = 0;
    // group 1
    {
        let mut matched = false;
        for alt in [&b"+-"[..], &b"+/-"[..], &b"+"[..], &b"-"[..]] {
            if starts_at(s, 0, alt) {
                i = alt.len();
                matched = true;
                break;
            }
        }
        if !matched && starts_at(s, 0, b"\\pm") {
            i = 3 + space_len(s, 3);
        }
    }
    g[0] = s[..i].to_vec();
    // group 2
    {
        let mut j = i;
        if digit_at(s, j) {
            j = digits_end(s, j);
            if any_at(s, j, b",.") && digit_at(s, j + 1) {
                j = digits_end(s, j + 1);
            }
        } else if char_at(s, j, b'.') && digit_at(s, j + 1) {
            j = digits_end(s, j + 1);
        }
        g[1] = s[i..j].to_vec();
        i = j;
    }
    // group 3
    if char_at(s, i, b'(') {
        let mut k = i + 1;
        while any_at(s, k, b"0123456789.,") {
            k += 1;
        }
        if char_at(s, k, b')') && is_number_literal(&s[i + 1..k], false) {
            g[2] = s[i..k + 1].to_vec();
            i = k + 1;
        }
    }
    // groups 4, 5, 6
    if any_at(s, i, b"eE")
        && let Some(e) = exponent_end(s, i + 1)
    {
        g[3] = s[i..i + 1].to_vec();
        g[5] = s[i + 1..e].to_vec();
        return e;
    }
    {
        let j = skip_spaces(s, i);
        let mut op: &[u8] = b"";
        for alt in [&b"*"[..], &b"x"[..], &b"\\times"[..], "×".as_bytes()] {
            if starts_at(s, j, alt) {
                op = alt;
                break;
            }
        }
        if !op.is_empty() {
            let k = skip_spaces(s, j + op.len());
            if starts_at(s, k, b"10^")
                && let Some(e) = exponent_end(s, k + 3)
            {
                g[3] = s[i..k + 3].to_vec();
                g[4] = op.to_vec();
                g[5] = s[k + 3..e].to_vec();
                return e;
            }
        }
    }
    i
}

/// `'(-)(9)^(-9)'` regexp:
/// `/^(\+\-|\+\/\-|\+|\-|\\pm\s?)?([0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+)?)\^([+\-]?[0-9]+|\{[+\-]?[0-9]+\})/`
pub fn number_pow_re(s: &[u8], g: &mut [Str; 3]) -> Option<usize> {
    // group 1 alternatives in backtracking order
    let mut cands: [Option<usize>; 7] = [None; 7];
    if starts_at(s, 0, b"+-") {
        cands[0] = Some(2);
    }
    if starts_at(s, 0, b"+/-") {
        cands[1] = Some(3);
    }
    if starts_at(s, 0, b"+") {
        cands[2] = Some(1);
    }
    if starts_at(s, 0, b"-") {
        cands[3] = Some(1);
    }
    if starts_at(s, 0, b"\\pm") {
        let n = space_len(s, 3);
        if n > 0 {
            cands[4] = Some(3 + n);
        }
        cands[5] = Some(3);
    }
    cands[6] = Some(0);
    for cand in cands {
        let Some(i) = cand else { continue };
        let mut k = i;
        while any_at(s, k, b"0123456789.,") {
            k += 1;
        }
        if !char_at(s, k, b'^') {
            continue;
        }
        if !is_number_literal(&s[i..k], true) {
            continue;
        }
        let Some(e) = exponent_end(s, k + 1) else {
            continue;
        };
        g[0] = s[..i].to_vec();
        g[1] = s[i..k].to_vec();
        g[2] = s[k + 1..e].to_vec();
        return Some(e);
    }
    None
}

/// First regexp of `'amount'`:
/// `/^(?:(?:(?:\([+\-]?[0-9]+\/[0-9]+\)|[+\-]?(?:[0-9]+|\$[a-z]\$|[a-z])\/[0-9]+|[+\-]?[0-9]+[.,][0-9]+|[+\-]?\.[0-9]+|[+\-]?[0-9]+)(?:[a-z](?=\s*[A-Z]))?)|[+\-]?[a-z](?=\s*[A-Z])|\+(?!\s))/`
pub fn amount_re(s: &[u8]) -> Option<usize> {
    if let Some(n) = amount_number(s) {
        if lower_at(s, n) && upper_after_spaces(s, n + 1) {
            return Some(n + 1);
        }
        return Some(n);
    }
    {
        let i: usize = if !s.is_empty() && is_sign(s[0]) { 1 } else { 0 };
        if lower_at(s, i) && upper_after_spaces(s, i + 1) {
            return Some(i + 1);
        }
    }
    if char_at(s, 0, b'+') && space_len(s, 1) == 0 {
        return Some(1);
    }
    None
}

/// `(?=\s*[A-Z])` at `i`.
pub fn upper_after_spaces(s: &[u8], i: usize) -> bool {
    let k = skip_spaces(s, i);
    k < s.len() && is_upper(s[k])
}

/// The five number alternatives of `amount_re`.
pub fn amount_number(s: &[u8]) -> Option<usize> {
    // \([+\-]?[0-9]+\/[0-9]+\)
    if char_at(s, 0, b'(') {
        let mut i: usize = 1;
        if i < s.len() && is_sign(s[i]) {
            i += 1;
        }
        if digit_at(s, i) {
            i = digits_end(s, i);
            if char_at(s, i, b'/') && digit_at(s, i + 1) {
                i = digits_end(s, i + 1);
                if char_at(s, i, b')') {
                    return Some(i + 1);
                }
            }
        }
    }
    let start: usize = if !s.is_empty() && is_sign(s[0]) { 1 } else { 0 };
    // [+\-]?(?:[0-9]+|\$[a-z]\$|[a-z])\/[0-9]+
    {
        let mut i = start;
        let mut ok = true;
        if digit_at(s, i) {
            i = digits_end(s, i);
        } else if char_at(s, i, b'$') && lower_at(s, i + 1) && char_at(s, i + 2, b'$') {
            i += 3;
        } else if lower_at(s, i) {
            i += 1;
        } else {
            ok = false;
        }
        if ok && char_at(s, i, b'/') && digit_at(s, i + 1) {
            return Some(digits_end(s, i + 1));
        }
    }
    // [+\-]?[0-9]+[.,][0-9]+
    if digit_at(s, start) {
        let i = digits_end(s, start);
        if any_at(s, i, b".,") && digit_at(s, i + 1) {
            return Some(digits_end(s, i + 1));
        }
    }
    // [+\-]?\.[0-9]+
    if char_at(s, start, b'.') && digit_at(s, start + 1) {
        return Some(digits_end(s, start + 1));
    }
    // [+\-]?[0-9]+
    if digit_at(s, start) {
        return Some(digits_end(s, start));
    }
    None
}

/// Second regexp of `'amount'` (full match of a `$...$` group):
/// `/^\$(?:\(?[+\-]?(?:[0-9]*[a-z]?[+\-])?[0-9]*[a-z](?:[+\-][0-9]*[a-z]?)?\)?|\+|-)\$$/`
pub fn amount_math(s: &[u8]) -> bool {
    if s.len() < 2 || s[0] != b'$' || s[s.len() - 1] != b'$' {
        return false;
    }
    let c = &s[1..s.len() - 1];
    let mut set = PosSet::init(c);
    set.opt(is_open_paren);
    set.opt(is_sign);
    {
        let mut t = set.clone_set();
        t.star(is_digit);
        t.opt(is_lower);
        t.one(is_sign);
        set.union_with(&t);
    }
    set.star(is_digit);
    set.one(is_lower);
    {
        let mut t = set.clone_set();
        t.one(is_sign);
        t.star(is_digit);
        t.opt(is_lower);
        set.union_with(&t);
    }
    set.opt(is_close_paren);
    if set.full_match() {
        return true;
    }
    eql(c, b"+") || eql(c, b"-")
}

/// `(?:[0-9]+|\$[a-z]\$|[a-z])` at `i`; returns the end index.
pub fn frac_numerator_end(s: &[u8], i: usize) -> Option<usize> {
    if digit_at(s, i) {
        return Some(digits_end(s, i));
    }
    if char_at(s, i, b'$') && lower_at(s, i + 1) && char_at(s, i + 2, b'$') {
        return Some(i + 3);
    }
    if lower_at(s, i) {
        return Some(i + 1);
    }
    None
}

/// `(?:\$[a-z]\$|[a-z])?$` at `i`; returns the optional group or null if no match.
pub fn frac_suffix(s: &[u8], i: usize) -> Option<Option<Str>> {
    if i == s.len() {
        return Some(None);
    }
    if s.len() - i == 3 && s[i] == b'$' && is_lower(s[i + 1]) && s[i + 2] == b'$' {
        return Some(Some(s[i..].to_vec()));
    }
    if s.len() - i == 1 && is_lower(s[i]) {
        return Some(Some(s[i..].to_vec()));
    }
    None
}

/// `'1/2$'`: `/^[+\-]?(?:[0-9]+|\$[a-z]\$|[a-z])\/[0-9]+(?:\$[a-z]\$|[a-z])?$/`
pub fn is_half(s: &[u8]) -> bool {
    let start: usize = if !s.is_empty() && is_sign(s[0]) { 1 } else { 0 };
    let Some(i) = frac_numerator_end(s, start) else {
        return false;
    };
    if !char_at(s, i, b'/') || !digit_at(s, i + 1) {
        return false;
    }
    frac_suffix(s, digits_end(s, i + 1)).is_some()
}
