//! ECMAScript RegExp runtime. All match ranges and lastIndex use UTF-16 units.
//!
//! `Str` cannot represent isolated surrogates: extracted halves become U+FFFD.
//! Replacement output is assembled in UTF-16 first, preserving intact pairs.
//! `split` maps unmatched captures to "" because its result is `List<Str>`.
mod errors;
mod match_array;
mod methods;
mod substitution;
#[cfg(test)]
mod tests;

use super::{List, Thrown};
use crate::Str;
pub use match_array::{MatchArray, exec, match_all_arrays, match_array};
pub use methods::{
    match_, match_all, match_all_global, replace, replace_all, replace_all_with, replace_with,
    search, split,
};
use std::{cell::Cell, rc::Rc};

/// A JavaScript reference: clones share the same lastIndex cell.
#[derive(Clone, Debug)]
pub struct RegExp(Rc<Inner>);
#[derive(Debug)]
struct Inner {
    /// The compiled pattern, shared by every object made from it
    /// ([`RegExp::fresh`]).
    program: Rc<Program>,
    last_index: Cell<f64>,
}
#[derive(Debug)]
struct Program {
    regex: regress::Regex,
    source: Str,
    flags: Str,
}

/// A cheap shared snapshot of a match, its captures, and the original input.
#[derive(Clone, Debug)]
pub struct RegExpMatch(Rc<MatchData>);
#[derive(Debug)]
struct MatchData {
    found: regress::Match,
    input: Input,
}
#[derive(Clone, Debug)]
struct Input {
    text: Str,
    units: Rc<[u16]>,
}
impl Input {
    fn new(s: &Str) -> Self {
        Self {
            text: s.clone(),
            units: s.encode_utf16().collect(),
        }
    }
}
fn string(units: &[u16]) -> Str {
    Str::from(String::from_utf16_lossy(units))
}
fn to_length(n: f64) -> usize {
    if n.is_nan() || n <= 0.0 {
        0
    } else {
        n.trunc().min(9007199254740991.0) as usize
    }
}
fn advance(units: &[u16], i: usize, unicode: bool) -> usize {
    if unicode
        && units.get(i).is_some_and(|u| (0xd800..=0xdbff).contains(u))
        && units
            .get(i + 1)
            .is_some_and(|u| (0xdc00..=0xdfff).contains(u))
    {
        i + 2
    } else {
        i + 1
    }
}
impl RegExp {
    pub fn new(source: &str, flags: &str) -> Result<Self, Thrown> {
        let mut seen = String::new();
        for c in flags.chars() {
            if !"dgimsuvy".contains(c) || seen.contains(c) {
                return Err(errors::flags(flags));
            }
            seen.push(c);
        }
        if seen.contains('u') && seen.contains('v') {
            return Err(errors::flags(flags));
        }
        let full_unicode = flags.contains('u') || flags.contains('v');
        let engine_flags = regress::Flags {
            unicode: full_unicode,
            ..flags.into()
        };
        // Legacy patterns operate on code units, including literal astral characters.
        let pattern: Vec<u32> = if full_unicode {
            source.chars().map(u32::from).collect()
        } else {
            source.encode_utf16().map(u32::from).collect()
        };
        let regex = regress::Regex::from_unicode(pattern.into_iter(), engine_flags)
            .map_err(|e| errors::pattern(source, flags, &e.to_string()))?;
        Ok(Self(Rc::new(Inner {
            program: Rc::new(Program {
                regex,
                source: errors::escape_source(source),
                flags: Str::from(
                    "dgimsuvy"
                        .chars()
                        .filter(|c| flags.contains(*c))
                        .collect::<String>(),
                ),
            }),
            last_index: Cell::new(0.0),
        })))
    }
    /// A new object of the same pattern (lastIndex 0) without compiling it
    /// again: what a regex literal evaluates to each time (the compiled
    /// program is cached per literal, like JS engines do).
    pub fn fresh(&self) -> Self {
        Self(Rc::new(Inner {
            program: Rc::clone(&self.0.program),
            last_index: Cell::new(0.0),
        }))
    }
    pub fn source(&self) -> Str {
        self.0.program.source.clone()
    }
    pub fn flags(&self) -> Str {
        self.0.program.flags.clone()
    }
    pub fn global(&self) -> bool {
        self.0.program.flags.contains('g')
    }
    pub fn ignore_case(&self) -> bool {
        self.0.program.flags.contains('i')
    }
    pub fn multiline(&self) -> bool {
        self.0.program.flags.contains('m')
    }
    pub fn sticky(&self) -> bool {
        self.0.program.flags.contains('y')
    }
    pub fn unicode(&self) -> bool {
        self.0.program.flags.contains('u')
    }
    pub fn dot_all(&self) -> bool {
        self.0.program.flags.contains('s')
    }
    pub fn has_indices(&self) -> bool {
        self.0.program.flags.contains('d')
    }
    pub fn last_index(&self) -> f64 {
        self.0.last_index.get()
    }
    pub fn set_last_index(&self, value: f64) {
        self.0.last_index.set(value);
    }
    pub fn to_str(&self) -> Str {
        Str::from(format!(
            "/{}/{}",
            self.0.program.source, self.0.program.flags
        ))
    }
    pub fn exec(&self, s: &Str) -> Option<RegExpMatch> {
        self.exec_input(&Input::new(s))
    }
    pub fn test(&self, s: &Str) -> bool {
        self.exec(s).is_some()
    }
    fn full_unicode(&self) -> bool {
        self.unicode() || self.0.program.flags.contains('v')
    }
    // Unlike Clone, this creates an independent lastIndex cell for matchAll.
    fn independent(&self) -> Self {
        Self(Rc::new(Inner {
            program: Rc::clone(&self.0.program),
            last_index: Cell::new(self.last_index()),
        }))
    }
    fn find(&self, input: &Input, mut start: usize, sticky: bool) -> Option<RegExpMatch> {
        if start > input.units.len() {
            return None;
        }
        // V8 starts at the containing code point when lastIndex bisects a pair.
        if self.full_unicode()
            && start > 0
            && input
                .units
                .get(start)
                .is_some_and(|u| (0xdc00..=0xdfff).contains(u))
            && (0xd800..=0xdbff).contains(&input.units[start - 1])
        {
            start -= 1;
        }
        let found = if self.full_unicode() {
            self.0
                .program
                .regex
                .find_from_utf16(&input.units, start)
                .next()
        } else {
            self.0
                .program
                .regex
                .find_from_ucs2(&input.units, start)
                .next()
        }?;
        if sticky && found.start() != start {
            return None;
        }
        Some(RegExpMatch(Rc::new(MatchData {
            found,
            input: input.clone(),
        })))
    }
    fn exec_input(&self, input: &Input) -> Option<RegExpMatch> {
        let stateful = self.global() || self.sticky();
        let start = if stateful {
            to_length(self.last_index())
        } else {
            0
        };
        let result = self.find(input, start, self.sticky());
        if stateful {
            self.set_last_index(result.as_ref().map_or(0.0, |m| m.end() as f64));
        }
        result
    }
}
impl RegExpMatch {
    pub fn get(&self, i: f64) -> Option<Str> {
        if !i.is_finite() || i < 0.0 || i.fract() != 0.0 {
            return None;
        }
        self.0
            .found
            .group(i as usize)
            .map(|r| string(&self.0.input.units[r]))
    }
    pub fn len(&self) -> f64 {
        (self.0.found.captures.len() + 1) as f64
    }
    pub fn index(&self) -> f64 {
        self.start() as f64
    }
    pub fn input(&self) -> Str {
        self.0.input.text.clone()
    }
    pub fn group(&self, name: &str) -> Option<Str> {
        self.named_range(name)
            .map(|r| string(&self.0.input.units[r]))
    }
    pub fn has_groups(&self) -> bool {
        self.0.found.named_groups().next().is_some()
    }
    /// `match.groups`: `undefined` without named groups; a group that did
    /// not take part in the match is absent (it reads as `undefined`).
    pub fn groups(&self) -> Option<super::Dict<Str, Str>> {
        if !self.has_groups() {
            return None;
        }
        Some(super::Dict::from_entries(
            self.0.found.named_groups().filter_map(|(name, range)| {
                range.map(|range| {
                    (
                        Str::from(name.to_owned()),
                        string(&self.0.input.units[range]),
                    )
                })
            }),
        ))
    }
    fn named_range(&self, name: &str) -> Option<regress::Range> {
        // named_groups merges duplicate names across disjoint alternatives.
        self.0
            .found
            .named_groups()
            .find(|(key, _)| *key == name)
            .and_then(|(_, range)| range)
    }
    fn start(&self) -> usize {
        self.0.found.start()
    }
    fn end(&self) -> usize {
        self.0.found.end()
    }
}
