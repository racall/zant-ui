//! Character helpers reproducing JS regexp semantics over UTF-8 bytes, plus
//! the small position-set simulator used by `amount_math`.

use super::types::{Match, Mv, Str};

/// One decoded code point: `c` is the code point, `n` its UTF-8 length.
pub struct Cp {
    pub c: u32,
    pub n: usize,
}

pub fn decode(s: &[u8], i: usize) -> Cp {
    let b = s[i];
    let n = match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => {
            return Cp {
                c: u32::from(b),
                n: 1,
            };
        }
    };
    if i + n > s.len() {
        return Cp {
            c: u32::from(b),
            n: 1,
        };
    }
    match std::str::from_utf8(&s[i..i + n]) {
        Ok(text) => Cp {
            c: text.chars().next().map_or(u32::from(b), |c| c as u32),
            n,
        },
        Err(_) => Cp {
            c: u32::from(b),
            n: 1,
        },
    }
}

/// JS `\s`.
pub fn is_js_space(c: u32) -> bool {
    matches!(
        c,
        0x09..=0x0D
            | 0x20
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

/// JS line terminators (not matched by `.`).
pub fn is_line_terminator(c: u32) -> bool {
    c == 0x0A || c == 0x0D || c == 0x2028 || c == 0x2029
}

/// Byte length of the `\s` character at `i`, 0 if none.
pub fn space_len(s: &[u8], i: usize) -> usize {
    if i >= s.len() {
        return 0;
    }
    let c = decode(s, i);
    if is_js_space(c.c) { c.n } else { 0 }
}

/// Index after `\s*` starting at `i`.
pub fn skip_spaces(s: &[u8], i: usize) -> usize {
    let mut j = i;
    loop {
        let n = space_len(s, j);
        if n == 0 {
            return j;
        }
        j += n;
    }
}

pub fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

pub fn is_lower(c: u8) -> bool {
    c.is_ascii_lowercase()
}

pub fn is_upper(c: u8) -> bool {
    c.is_ascii_uppercase()
}

pub fn is_alpha(c: u8) -> bool {
    is_lower(c) || is_upper(c)
}

pub fn is_roman(c: u8) -> bool {
    c == b'I' || c == b'V' || c == b'X'
}

pub fn is_sign(c: u8) -> bool {
    c == b'+' || c == b'-'
}

pub fn digit_at(s: &[u8], i: usize) -> bool {
    i < s.len() && is_digit(s[i])
}

pub fn lower_at(s: &[u8], i: usize) -> bool {
    i < s.len() && is_lower(s[i])
}

pub fn char_at(s: &[u8], i: usize, c: u8) -> bool {
    i < s.len() && s[i] == c
}

pub fn any_at(s: &[u8], i: usize, set: &[u8]) -> bool {
    i < s.len() && set.contains(&s[i])
}

pub fn starts_at(s: &[u8], i: usize, prefix: &[u8]) -> bool {
    i <= s.len() && s[i..].starts_with(prefix)
}

/// Index after `[0-9]*` starting at `i`.
pub fn digits_end(s: &[u8], i: usize) -> usize {
    let mut j = i;
    while digit_at(s, j) {
        j += 1;
    }
    j
}

pub fn eql(a: &[u8], b: &[u8]) -> bool {
    a == b
}

/// JS truthiness of an optional string (`undefined` and `""` are falsy).
pub fn truthy(s: &Option<Str>) -> bool {
    s.as_ref().is_some_and(|x| !x.is_empty())
}

pub fn opt_eql(s: &Option<Str>, b: &[u8]) -> bool {
    s.as_deref().is_some_and(|x| x == b)
}

pub fn is_open_paren(c: u8) -> bool {
    c == b'('
}

pub fn is_close_paren(c: u8) -> bool {
    c == b')'
}

/// Position-set simulation for small anchored regexps (exact for boolean
/// full matches, independent of backtracking order).
pub struct PosSet<'a> {
    s: &'a [u8],
    set: Vec<bool>,
}

impl<'a> PosSet<'a> {
    pub fn init(s: &'a [u8]) -> PosSet<'a> {
        let mut set = vec![false; s.len() + 1];
        set[0] = true;
        PosSet { s, set }
    }

    pub fn clone_set(&self) -> PosSet<'a> {
        PosSet {
            s: self.s,
            set: self.set.clone(),
        }
    }

    /// `set := one(class)(set)`.
    pub fn one(&mut self, pred: fn(u8) -> bool) {
        for i in (1..=self.s.len()).rev() {
            let prev = self.set[i - 1] && pred(self.s[i - 1]);
            self.set[i] = prev;
        }
        self.set[0] = false;
    }

    /// `set := set ∪ one(class)(set)`.
    pub fn opt(&mut self, pred: fn(u8) -> bool) {
        for i in (1..=self.s.len()).rev() {
            if self.set[i - 1] && pred(self.s[i - 1]) {
                self.set[i] = true;
            }
        }
    }

    /// `set := star(class)(set)`.
    pub fn star(&mut self, pred: fn(u8) -> bool) {
        for i in 0..self.s.len() {
            if self.set[i] && pred(self.s[i]) {
                self.set[i + 1] = true;
            }
        }
    }

    pub fn union_with(&mut self, other: &PosSet<'_>) {
        for (a, b) in self.set.iter_mut().zip(other.set.iter()) {
            *a = *a || *b;
        }
    }

    /// Whether the whole string is matched (the set contains `s.len`).
    pub fn full_match(&self) -> bool {
        self.set[self.s.len()]
    }
}

/// Replaces every occurrence of any of `alts` (leftmost, first alternative
/// first, like a global JS regexp of literal alternatives) by `repl`.
pub fn replace_alternatives(s: &[u8], alts: &[&[u8]], repl: &[u8]) -> Str {
    let mut out: Str = Vec::new();
    let mut i: usize = 0;
    'outer: while i < s.len() {
        for a in alts {
            if starts_at(s, i, a) {
                out.extend_from_slice(repl);
                i += a.len();
                continue 'outer;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

/// JS `String.prototype.replace(string, string)`: first occurrence only.
pub fn replace_first(s: &[u8], needle: &[u8], repl: &[u8]) -> Str {
    let Some(i) = find_subslice(s, needle) else {
        return s.to_vec();
    };
    let mut out = Vec::with_capacity(s.len() + repl.len());
    out.extend_from_slice(&s[..i]);
    out.extend_from_slice(repl);
    out.extend_from_slice(&s[i + needle.len()..]);
    out
}

/// `String.prototype.replace(/<literal>/g, repl)`: all occurrences.
pub fn replace_all(s: &[u8], needle: &[u8], repl: &[u8]) -> Str {
    let mut out: Str = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if starts_at(s, i, needle) {
            out.extend_from_slice(repl);
            i += needle.len();
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Trims the bytes of `set` from both ends.
pub fn trim_bytes<'a>(s: &'a [u8], set: &[u8]) -> &'a [u8] {
    let mut start = 0;
    let mut end = s.len();
    while start < end && set.contains(&s[start]) {
        start += 1;
    }
    while end > start && set.contains(&s[end - 1]) {
        end -= 1;
    }
    &s[start..end]
}

pub fn m0(s: &[u8], n: usize) -> Match {
    Match {
        m: Mv::S(s[..n].to_vec()),
        rem: s[n..].to_vec(),
    }
}

pub fn opt0(s: &[u8], n: Option<usize>) -> Option<Match> {
    n.map(|k| m0(s, k))
}

/// `patterns.match_` for a regexp with one group: group 1 if truthy, else the
/// whole match.
pub fn group1(s: &[u8], n: usize, g: &[u8]) -> Match {
    Match {
        m: Mv::S(if !g.is_empty() {
            g.to_vec()
        } else {
            s[..n].to_vec()
        }),
        rem: s[n..].to_vec(),
    }
}

/// `m[0]` in 'color0-output' (on a string this is its first character).
pub fn first_element(m: &Mv) -> Str {
    match m {
        Mv::Arr(a) => {
            if let Some(first) = a.first() {
                first.clone()
            } else {
                b"undefined".to_vec()
            }
        }
        Mv::S(s) => {
            if s.is_empty() {
                b"undefined".to_vec()
            } else {
                s[..decode(s, 0).n].to_vec()
            }
        }
    }
}

/// `/^[0-9]+$/`
pub fn is_all_digits(s: &[u8]) -> bool {
    !s.is_empty() && digits_end(s, 0) == s.len()
}

pub fn concat(parts: &[&[u8]]) -> Str {
    let mut out: Str = Vec::new();
    for p in parts {
        out.extend_from_slice(p);
    }
    out
}
