//! mhchem: chemical equations (`\ce{...}`) and physical units (`\pu{...}`).
//!
//! A faithful Rust port of the mhchem parser and texifier shipped with KaTeX
//! (`katex/contrib/mhchem/mhchem.js`, KaTeX's adaptation of mhchem 3.3.0):
//!
//!   mhchem — Copyright (c) 2015-2018 Martin Hensel,
//!            Copyright (c) 2011-2015 The MathJax Consortium.
//!            Licensed under the Apache License, Version 2.0
//!            (http://www.apache.org/licenses/LICENSE-2.0).
//!   KaTeX  — Copyright (c) 2013-2020 Khan Academy and other contributors
//!            (MIT); KaTeX's changes: \mathrlap/\mathllap instead of
//!            \rlap/\llap, \raisebox instead of \raise, extensible reaction
//!            arrows, adjusted \tripledash.
//!
//! `to_tex(input, Machine::Ce)` returns byte-for-byte the string that
//! `texify.go(mhchemParser.go(input, "ce"))` returns upstream (likewise for
//! `Machine::Pu`); upstream exceptions become `MhchemError`. The result still
//! needs KaTeX's `\tripledash` macro (defined by mhchem.js) when bonds of
//! the `~` family are used.
//!
//! The structure follows upstream so fixes can be diffed against it:
//!   * `Pat` + `match_pattern` = `mhchemParser.patterns.patterns` /
//!     `patterns.match_`; every regexp is a hand-written matcher that
//!     reproduces the JS (sticky, backtracking) semantics, named exactly
//!     like the upstream pattern.
//!   * `find_observe_groups` = `patterns.findObserveGroups`.
//!   * the `*_TRANSITIONS` tables = `mhchemParser.stateMachines[...]`
//!     (same pattern/state keys, same order, same actions/options).
//!   * `run_action` = `mhchemParser.actions` + the per-machine
//!     `actions` (looked up machine first, like upstream).
//!   * `go` = `mhchemParser.go`, `texify`/`go2` = `texify.go`/`texify._go2`.
//!
//! Port of `src/math/mhchem.zig` — WP-3.7.

mod actions;
mod chars;
mod enums;
mod observe;
mod outputs;
mod patterns;
mod patterns_group;
mod patterns_token;
mod patterns_word;
mod regexes;
mod tables;
mod tables_ce;
mod texify;
mod types;

#[cfg(test)]
mod tests;

use types::{Buffer, Item, Items, Mv, Ret, Sm, Str};

/// `\ce` / `\pu` selector.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    Ce,
    Pu,
}

impl Machine {
    fn sm(self) -> Sm {
        match self {
            Machine::Ce => Sm::Ce,
            Machine::Pu => Sm::Pu,
        }
    }
}

/// `error.MhchemError`: upstream mhchem throws (and malformed output).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MhchemError;

impl std::fmt::Display for MhchemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("mhchem error")
    }
}

impl std::error::Error for MhchemError {}

/// TeX for `\ce{input}` (`Machine::Ce`) or `\pu{input}` (`Machine::Pu`).
///
/// Rust returns an owned `String`, so output bytes that are not valid UTF-8
/// (only possible when a byte-level split lands inside a multi-byte character,
/// in which case the Zig version returns the raw bytes) become `MhchemError`.
pub fn to_tex(input: &str, machine: Machine) -> Result<String, MhchemError> {
    let parsed = go(Some(input.as_bytes().to_vec()), machine.sm())?;
    let out = texify::texify(&parsed, false)?;
    String::from_utf8(out).map_err(|_| MhchemError)
}

/// The fixture (input, machine, expected TeX) for integration tests that lay
/// every case out.
pub const FIXTURE_JSON: &str = include_str!("../mhchem_cases.json");

// ---------------------------------------------------------------------------
// mhchemParser.go
// ---------------------------------------------------------------------------

fn go(input0: Option<Str>, sm: Sm) -> Result<Items, MhchemError> {
    let raw = match input0 {
        Some(raw) => raw,
        None => return Ok(Vec::new()),
    };
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut input = normalize(&raw);
    let mut state: &'static [u8] = b"0";
    let mut buffer = Buffer::default();
    let mut output: Items = Vec::new();
    let transitions = tables::machine_transitions(sm);
    let mut last_len: Option<usize> = None;
    let mut watchdog: i32 = 10;
    loop {
        if last_len != Some(input.len()) {
            watchdog = 10;
            last_len = Some(input.len());
        } else {
            watchdog -= 1;
        }
        // `machine.transitions[state] || machine.transitions['*']`
        let key: &[u8] = if tables::state_exists(transitions, state) {
            state
        } else {
            b"*"
        };
        let mut broke = false;
        for t in transitions {
            if !tables::state_in(t.states, key) && !tables::state_in(t.states, b"*") {
                continue;
            }
            for pat in t.patterns {
                let Some(matches) = patterns::match_pattern(*pat, &input)? else {
                    continue;
                };
                for action in t.task.action_ {
                    let ret = actions::run_action(sm, &mut buffer, matches.m.clone(), *action)?;
                    concat_array(&mut output, ret);
                }
                if let Some(next) = t.task.next_state {
                    state = next.as_bytes();
                }
                if !input.is_empty() {
                    if !t.task.revisit {
                        input = matches.rem;
                    }
                    if !t.task.to_continue {
                        broke = true;
                        break;
                    }
                } else {
                    return Ok(output);
                }
            }
            if broke {
                break;
            }
        }
        // Prevent infinite loop
        if watchdog <= 0 {
            return Err(MhchemError); // "MhchemBugU"
        }
    }
}

/// `input.replace(/\n/g, " ").replace(/[−–—‐]/g, "-").replace(/[…]/g, "...")`
fn normalize(s: &[u8]) -> Str {
    let needs = s.contains(&b'\n')
        || ["−", "–", "—", "‐", "…"]
            .iter()
            .any(|needle| contains(s, needle.as_bytes()));
    if !needs {
        return s.to_vec();
    }
    let mut out: Str = Vec::new();
    let mut i: usize = 0;
    while i < s.len() {
        if s[i] == b'\n' {
            out.push(b' ');
            i += 1;
            continue;
        }
        let c = chars::decode(s, i);
        match c.c {
            0x2212 | 0x2013 | 0x2014 | 0x2010 => out.push(b'-'),
            0x2026 => out.extend_from_slice(b"..."),
            _ => out.extend_from_slice(&s[i..i + c.n]),
        }
        i += c.n;
    }
    out
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------------------
// `mhchemParser.concatArray` and the small `Parser` helpers
// ---------------------------------------------------------------------------

fn concat_array(a: &mut Items, b: Ret) {
    match b {
        Ret::None => {}
        Ret::One(item) => {
            if matches!(&item, Item::Str(s) if s.is_empty()) {
                return; // falsy ""
            }
            a.push(item);
        }
        Ret::Many(items) => a.extend(items),
    }
}

fn cat(parts: &[&[u8]]) -> Str {
    chars::concat(parts)
}

/// JS `String(m)` of a match (arrays join with ",").
fn mstr(m: &Mv) -> Str {
    match m {
        Mv::S(s) => s.clone(),
        Mv::Arr(arr) => {
            let mut out: Str = Vec::new();
            for (i, part) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(part);
            }
            out
        }
    }
}

/// `(buffer.x || "") + m`
fn app(x: Option<&[u8]>, m: &Mv) -> Str {
    let matched = mstr(m);
    cat(&[x.unwrap_or(b""), &matched])
}

/// `[ "{", ...items, "}" ]`
fn braced(items: Items) -> Ret {
    let mut ret: Items = Vec::with_capacity(items.len() + 2);
    ret.push(Item::Str(b"{".to_vec()));
    ret.extend(items);
    ret.push(Item::Str(b"}".to_vec()));
    Ret::Many(ret)
}
