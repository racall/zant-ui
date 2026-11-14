//! Pattern matchers that use `findObserveGroups` (and the state/amount
//! matchers built on it). Split out of `patterns.rs` to keep files small.

use super::MhchemError;
use super::chars::*;
use super::observe::*;
use super::patterns::Pat;
use super::regexes::*;
use super::types::*;

pub fn match_group(pat: Pat, s: &[u8]) -> Result<Option<Match>, MhchemError> {
    match pat {
        // ... or crystal system
        Pat::StateOfAggregationEnd => {
            // (aq), (aq,$\infty$), (aq, sat)
            if let Some(a) = find_observe_groups(
                s,
                lit(b""),
                Fm::Re(re_aggregation_begin),
                lit(b")"),
                lit(b""),
            )? {
                // AND end of 'phrase': /^($|[\s,;\)\]\}])/
                if delimiter_or_end(&a.rem, 0) {
                    return Ok(Some(a));
                }
            }
            // OR crystal system ($o$) (\ca$c$): /^(?:\((?:\\ca\s?)?\$[amothc]\$\))/
            if !char_at(s, 0, b'(') {
                return Ok(None);
            }
            let mut i: usize = 1;
            if starts_at(s, 1, b"\\ca") {
                i = 4 + space_len(s, 4);
            }
            if char_at(s, i, b'$')
                && any_at(s, i + 1, b"amothc")
                && char_at(s, i + 2, b'$')
                && char_at(s, i + 3, b')')
            {
                return Ok(Some(m0(s, i + 4)));
            }
            Ok(None)
        }
        Pat::CaretBraced => find_observe_groups(s, lit(b"^{"), lit(b""), lit(b""), lit(b"}")),
        Pat::CaretDollar => find_observe_groups(s, lit(b"^"), lit(b"$"), lit(b"$"), lit(b"")),
        Pat::CaretMacroBracedBraced => find_observe_groups2(
            s,
            lit(b"^"),
            Fm::Re(re_macro_brace),
            lit(b"}"),
            lit(b""),
            lit(b""),
            lit(b"{"),
            lit(b"}"),
            lit(b""),
            true,
        ),
        Pat::CaretMacroBraced => {
            find_observe_groups(s, lit(b"^"), Fm::Re(re_macro_brace), lit(b"}"), lit(b""))
        }
        Pat::UnderscoreBraced => find_observe_groups(s, lit(b"_{"), lit(b""), lit(b""), lit(b"}")),
        Pat::UnderscoreDollar => find_observe_groups(s, lit(b"_"), lit(b"$"), lit(b"$"), lit(b"")),
        Pat::UnderscoreMacroBracedBraced => find_observe_groups2(
            s,
            lit(b"_"),
            Fm::Re(re_macro_brace),
            lit(b"}"),
            lit(b""),
            lit(b""),
            lit(b"{"),
            lit(b"}"),
            lit(b""),
            true,
        ),
        Pat::UnderscoreMacroBraced => {
            find_observe_groups(s, lit(b"_"), Fm::Re(re_macro_brace), lit(b"}"), lit(b""))
        }
        Pat::Braced => find_observe_groups(s, lit(b""), lit(b"{"), lit(b"}"), lit(b"")),
        Pat::BracedParen => find_observe_groups(s, lit(b"{"), lit(b""), lit(b""), lit(b"}")),
        Pat::DollarText => find_observe_groups(s, lit(b""), lit(b"$"), lit(b"$"), lit(b"")),
        Pat::DollarBracedDollar => {
            find_observe_groups(s, lit(b"${"), lit(b""), lit(b""), lit(b"}$"))
        }
        Pat::DollarParenDollar => find_observe_groups(s, lit(b"$"), lit(b""), lit(b""), lit(b"$")),
        Pat::BondBraced => find_observe_groups(s, lit(b"\\bond{"), lit(b""), lit(b""), lit(b"}")),
        Pat::BracketParen => find_observe_groups(s, lit(b"["), lit(b""), lit(b""), lit(b"]")),
        Pat::MacroBracedBraced => find_observe_groups2(
            s,
            lit(b""),
            Fm::Re(re_macro_brace),
            lit(b"}"),
            lit(b""),
            lit(b""),
            lit(b"{"),
            lit(b"}"),
            lit(b""),
            true,
        ),
        Pat::MacroBraced => {
            find_observe_groups(s, lit(b""), Fm::Re(re_macro_brace), lit(b"}"), lit(b""))
        }
        Pat::FracBraced => find_observe_groups2(
            s,
            lit(b"\\frac{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            lit(b"{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            false,
        ),
        Pat::OversetBraced => find_observe_groups2(
            s,
            lit(b"\\overset{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            lit(b"{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            false,
        ),
        Pat::UndersetBraced => find_observe_groups2(
            s,
            lit(b"\\underset{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            lit(b"{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            false,
        ),
        Pat::UnderbraceBraced => find_observe_groups2(
            s,
            lit(b"\\underbrace{"),
            lit(b""),
            lit(b""),
            lit(b"}_"),
            lit(b"{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            false,
        ),
        Pat::ColorBraced0 => {
            find_observe_groups(s, lit(b"\\color{"), lit(b""), lit(b""), lit(b"}"))
        }
        Pat::ColorBracedBraced1 => find_observe_groups2(
            s,
            lit(b"\\color{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            lit(b"{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            false,
        ),
        Pat::ColorParenBraced2 => find_observe_groups2(
            s,
            lit(b"\\color"),
            lit(b"\\"),
            lit(b""),
            Fm::Re(re_before_brace),
            lit(b"{"),
            lit(b""),
            lit(b""),
            lit(b"}"),
            false,
        ),
        Pat::CeBraced => find_observe_groups(s, lit(b"\\ce{"), lit(b""), lit(b""), lit(b"}")),
        // e.g. 2, 0.5, 1/2, -2, n/2, +;  $a$ could be added later in parsing
        Pat::Amount | Pat::Amount2 => {
            if let Some(n) = amount_re(s) {
                return Ok(Some(m0(s, n)));
            }
            let Some(a) = find_observe_groups(s, lit(b""), lit(b"$"), lit(b"$"), lit(b""))? else {
                return Ok(None);
            };
            // e.g. $2n-1$, $-$
            let text = a.m.as_str().unwrap_or_default();
            if amount_math(text) {
                return Ok(Some(m0(s, text.len())));
            }
            Ok(None)
        }
        // /^(?:[A-Z][a-z]{0,2}|i)(?=,)/
        Pat::KvLettersComma => {
            if !s.is_empty() && is_upper(s[0]) {
                let mut k: usize = 0;
                while k < 2 && lower_at(s, 1 + k) {
                    k += 1;
                }
                if char_at(s, 1 + k, b',') {
                    return Ok(Some(m0(s, 1 + k)));
                }
            }
            if char_at(s, 0, b'i') && char_at(s, 1, b',') {
                return Ok(Some(m0(s, 1)));
            }
            Ok(None)
        }
        Pat::FormulaEnd => {
            // state of aggregation = no formula: /^\([a-z]+\)$/
            if s.len() >= 3 && s[0] == b'(' && s[s.len() - 1] == b')' {
                let mut all_lower = true;
                for &c in &s[1..s.len() - 1] {
                    if !is_lower(c) {
                        all_lower = false;
                        break;
                    }
                }
                if all_lower {
                    return Ok(None);
                }
            }
            Ok(if is_formula(s) {
                Some(m0(s, s.len()))
            } else {
                None
            })
        }
        _ => Ok(None),
    }
}
