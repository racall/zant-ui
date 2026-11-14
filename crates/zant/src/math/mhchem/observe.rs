//! `mhchemParser.patterns.findObserveGroups` and the small anchored regexps
//! it uses. Port of `src/math/mhchem.zig` (findObserveGroups section).

use super::MhchemError;
use super::chars::{any_at, char_at, is_alpha, lower_at};
use super::types::{Match, Mv, Str};

#[derive(Clone, Copy)]
pub enum Fm {
    Str(&'static [u8]),
    Re(fn(&[u8]) -> Option<usize>),
}

impl Fm {
    /// `_match`: length of the match at the start of `s`.
    pub fn at(&self, s: &[u8]) -> Option<usize> {
        match self {
            Fm::Str(p) => {
                if s.starts_with(p) {
                    Some(p.len())
                } else {
                    None
                }
            }
            Fm::Re(f) => f(s),
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            Fm::Str(p) => !p.is_empty(),
            Fm::Re(_) => true,
        }
    }
}

pub fn lit(s: &'static [u8]) -> Fm {
    Fm::Str(s)
}

/// `/^\([a-z]{1,3}(?=[\),])/`
pub fn re_aggregation_begin(s: &[u8]) -> Option<usize> {
    if !char_at(s, 0, b'(') {
        return None;
    }
    let mut k: usize = 0;
    while k < 3 && lower_at(s, 1 + k) {
        k += 1;
    }
    if k == 0 {
        return None;
    }
    if !any_at(s, 1 + k, b"),") {
        return None;
    }
    Some(1 + k)
}

/// `/^\\[a-zA-Z]+\{/`
pub fn re_macro_brace(s: &[u8]) -> Option<usize> {
    if !char_at(s, 0, b'\\') {
        return None;
    }
    let mut i: usize = 1;
    while i < s.len() && is_alpha(s[i]) {
        i += 1;
    }
    if i == 1 || !char_at(s, i, b'{') {
        return None;
    }
    Some(i + 1)
}

/// `/^(?=\{)/`
pub fn re_before_brace(s: &[u8]) -> Option<usize> {
    if char_at(s, 0, b'{') { Some(0) } else { None }
}

pub struct ObserveEnd {
    pub begin: usize,
    pub end: usize,
}

/// `_findObserveGroups`
pub fn find_observe_end(
    input: &[u8],
    start: usize,
    end_chars: Fm,
) -> Result<Option<ObserveEnd>, MhchemError> {
    let mut braces: usize = 0;
    let mut i = start;
    while i < input.len() {
        let a = input[i];
        // Upstream: `if (mm != null and braces == 0) ... else if (a == '{') ...
        // else if (a == '}') ...` — a matched end char inside braces must still
        // fall through to the brace bookkeeping below.
        if let Some(m) = end_chars.at(&input[i..])
            && braces == 0
        {
            return Ok(Some(ObserveEnd {
                begin: i,
                end: i + m,
            }));
        }
        if a == b'{' {
            braces += 1;
        } else if a == b'}' {
            // upstream throws ["ExtraCloseMissingOpen", ...]
            if braces == 0 {
                return Err(MhchemError);
            }
            braces -= 1;
        }
        i += 1;
    }
    Ok(None)
}

/// `findObserveGroups(input, begExcl, begIncl, endIncl, endExcl)`
pub fn find_observe_groups(
    s: &[u8],
    beg_excl: Fm,
    beg_incl: Fm,
    end_incl: Fm,
    end_excl: Fm,
) -> Result<Option<Match>, MhchemError> {
    let Some(n1) = beg_excl.at(s) else {
        return Ok(None);
    };
    let input = &s[n1..];
    let Some(n2) = beg_incl.at(input) else {
        return Ok(None);
    };
    let end_chars = if end_incl.truthy() {
        end_incl
    } else {
        end_excl
    };
    let Some(e) = find_observe_end(input, n2, end_chars)? else {
        return Ok(None);
    };
    let match1 = input[..if end_incl.truthy() { e.end } else { e.begin }].to_vec();
    Ok(Some(Match {
        m: Mv::S(match1),
        rem: input[e.end..].to_vec(),
    }))
}

/// `findObserveGroups` with the second group (beg2Excl/beg2Incl given).
#[allow(clippy::too_many_arguments)]
pub fn find_observe_groups2(
    s: &[u8],
    beg_excl: Fm,
    beg_incl: Fm,
    end_incl: Fm,
    end_excl: Fm,
    beg2_excl: Fm,
    beg2_incl: Fm,
    end2_incl: Fm,
    end2_excl: Fm,
    combine: bool,
) -> Result<Option<Match>, MhchemError> {
    let Some(g1) = find_observe_groups(s, beg_excl, beg_incl, end_incl, end_excl)? else {
        return Ok(None);
    };
    let Some(g2) = find_observe_groups(&g1.rem, beg2_excl, beg2_incl, end2_incl, end2_excl)? else {
        return Ok(None);
    };
    if combine {
        let first = g1.m.as_str().unwrap_or_default();
        let second = g2.m.as_str().unwrap_or_default();
        return Ok(Some(Match {
            m: Mv::S(super::chars::concat(&[first, second])),
            rem: g2.rem,
        }));
    }
    let arr: Vec<Str> = vec![
        g1.m.as_str().map(<[u8]>::to_vec).unwrap_or_default(),
        g2.m.as_str().map(<[u8]>::to_vec).unwrap_or_default(),
    ];
    Ok(Some(Match {
        m: Mv::Arr(arr),
        rem: g2.rem,
    }))
}
