//! What TypeScript calls `RegExpMatchArray` / `RegExpExecArray`: the result
//! of `exec`, `match` and each `matchAll` item. A global `match` returns
//! only the matched strings (no `index` / `groups`), the other forms one
//! match with its captures; JavaScript gives both the same array type, and
//! so does this one value.

use super::{RegExp, RegExpMatch};
use crate::Str;
use crate::tsx::{Dict, List};

#[derive(Clone, Debug)]
pub enum MatchArray {
    /// One match: the whole match, then the captures.
    Exec(RegExpMatch),
    /// A global `match`: every matched string.
    All(List<Str>),
}

impl From<RegExpMatch> for MatchArray {
    fn from(found: RegExpMatch) -> Self {
        MatchArray::Exec(found)
    }
}

impl Default for MatchArray {
    fn default() -> Self {
        MatchArray::All(List::default())
    }
}

impl PartialEq for MatchArray {
    /// Arrays compare by identity in JavaScript; two results are equal
    /// here when they hold the same strings at the same index.
    fn eq(&self, other: &Self) -> bool {
        self.to_list() == other.to_list() && self.index().to_bits() == other.index().to_bits()
    }
}

impl MatchArray {
    /// `m[i]`; a capture that did not take part is `undefined`.
    pub fn get(&self, i: f64) -> Option<Str> {
        match self {
            MatchArray::Exec(found) => found.get(i),
            MatchArray::All(list) => {
                if i.is_finite() && i >= 0.0 && i.fract() == 0.0 {
                    list.as_slice().get(i as usize).cloned()
                } else {
                    None
                }
            }
        }
    }

    /// `m.length`.
    pub fn len(&self) -> f64 {
        match self {
            MatchArray::Exec(found) => found.len(),
            MatchArray::All(list) => list.len() as f64,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0.0
    }

    /// `m.index` (UTF-16); `undefined` (NaN here) on a global match.
    pub fn index(&self) -> f64 {
        match self {
            MatchArray::Exec(found) => found.index(),
            MatchArray::All(_) => f64::NAN,
        }
    }

    /// `m.input`; empty on a global match (JavaScript: `undefined`).
    pub fn input(&self) -> Str {
        match self {
            MatchArray::Exec(found) => found.input(),
            MatchArray::All(_) => Str::Static(""),
        }
    }

    /// `m.groups`.
    pub fn groups(&self) -> Option<Dict<Str, Str>> {
        match self {
            MatchArray::Exec(found) => found.groups(),
            MatchArray::All(_) => None,
        }
    }

    /// The array itself (`m.map(…)`, `[...m]`); an unmatched capture is "".
    pub fn to_list(&self) -> List<Str> {
        match self {
            MatchArray::Exec(found) => (0..found.len() as usize)
                .map(|i| found.get(i as f64).unwrap_or_default())
                .collect(),
            MatchArray::All(list) => list.clone(),
        }
    }
}

/// `s.match(re)`: every matched string with `g`, else the first match.
pub fn match_array(s: &Str, re: &RegExp) -> Option<MatchArray> {
    if re.global() {
        super::match_all_global(s, re).map(MatchArray::All)
    } else {
        super::match_(s, re).map(MatchArray::Exec)
    }
}

/// `re.exec(s)`.
pub fn exec(re: &RegExp, s: &Str) -> Option<MatchArray> {
    re.exec(s).map(MatchArray::Exec)
}

/// `s.matchAll(re)` (requires `g`; a TypeError otherwise, as in JavaScript).
pub fn match_all_arrays(s: &Str, re: &RegExp) -> Result<List<MatchArray>, crate::tsx::Thrown> {
    Ok(super::match_all(s, re)?
        .as_slice()
        .iter()
        .cloned()
        .map(MatchArray::Exec)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Str {
        Str::from(text.to_owned())
    }

    #[test]
    fn global_and_single_matches_share_one_array_type() {
        // "a1b22c333".match(/\d+/g) → ["1", "22", "333"]
        let re = RegExp::new(r"\d+", "g").unwrap();
        let all = match_array(&s("a1b22c333"), &re).unwrap();
        assert_eq!(all.len(), 3.0);
        assert_eq!(all.get(2.0).as_deref(), Some("333"));
        assert!(all.index().is_nan());
        assert!(all.groups().is_none());
        // "2026-11-16".match(/(?<y>\d+)-(?<m>\d+)/) → index 0, groups.y "2026"
        let re = RegExp::new(r"(?<y>\d+)-(?<m>\d+)(x)?", "").unwrap();
        let one = match_array(&s("2026-11-16"), &re).unwrap();
        assert_eq!(one.index(), 0.0);
        assert_eq!(one.get(1.0).as_deref(), Some("2026"));
        assert_eq!(one.get(3.0), None);
        assert_eq!(one.len(), 4.0);
        let groups = one.groups().unwrap();
        assert_eq!(groups.get(&s("m")).as_deref(), Some("11"));
        assert_eq!(
            one.to_list().as_slice(),
            &[s("2026-11"), s("2026"), s("11"), s("")]
        );
        assert!(match_array(&s("none"), &re).is_none());
    }

    #[test]
    fn match_all_needs_the_global_flag() {
        let re = RegExp::new("a", "").unwrap();
        assert!(match_all_arrays(&s("aa"), &re).is_err());
        let re = RegExp::new("a(.)", "g").unwrap();
        let items = match_all_arrays(&s("abac"), &re).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items.as_slice()[1].get(1.0).as_deref(), Some("c"));
        assert_eq!(items.as_slice()[1].index(), 2.0);
    }
}
