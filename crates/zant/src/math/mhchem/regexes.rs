//! Hand-written matchers for every regexp of `mhchemParser.patterns.patterns`
//! that is not a trivial literal. Each one reproduces the JS sticky regexp's
//! boolean/full-match semantics by hand.

use super::chars::{
    PosSet, any_at, char_at, decode, digit_at, digits_end, eql, is_alpha, is_close_paren, is_digit,
    is_lower, is_open_paren, is_roman, is_sign, is_upper, lower_at, skip_spaces, space_len,
    starts_at,
};
use super::types::Str;

static GREEK_LOWER: [&[u8]; 24] = [
    b"alpha", b"beta", b"gamma", b"delta", b"epsilon", b"zeta", b"eta", b"theta", b"iota",
    b"kappa", b"lambda", b"mu", b"nu", b"xi", b"omicron", b"pi", b"rho", b"sigma", b"tau",
    b"upsilon", b"phi", b"chi", b"psi", b"omega",
];
static GREEK_UPPER: [&[u8]; 11] = [
    b"Gamma", b"Delta", b"Theta", b"Lambda", b"Xi", b"Pi", b"Sigma", b"Upsilon", b"Phi", b"Psi",
    b"Omega",
];

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
