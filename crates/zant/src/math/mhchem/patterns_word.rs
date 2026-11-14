//! Word/number patterns (`patterns.rs` split): letters, numbers, states,
//! oxidation numbers and arrows.

use super::chars::*;
use super::patterns::Pat;
use super::regexes::*;
use super::types::*;

pub fn match_word(pat: Pat, s: &[u8]) -> Option<Match> {
    match pat {
        Pat::Letters => opt0(s, letters_end(s)),
        // /^\\(?:alpha|...|Omega)(?:\s+|\{\}|(?![a-zA-Z]))/
        Pat::Greek => match greek_macro_end(s, 0, true) {
            Some(j) => opt0(s, greek_suffix_end(s, j)),
            None => None,
        },
        // /^(?:([a-z])(?:$|[^a-zA-Z]))$/
        Pat::OneLowercaseLatinLetterEnd => {
            if !lower_at(s, 0) {
                None
            } else if s.len() == 1 {
                Some(group1(s, 1, &s[0..1]))
            } else {
                let c = decode(s, 1);
                if 1 + c.n == s.len() && c.c <= 0xFFFF && !(c.c < 128 && is_alpha(c.c as u8)) {
                    Some(group1(s, s.len(), &s[0..1]))
                } else {
                    None
                }
            }
        }
        // /^\$(?:([a-z])(?:$|[^a-zA-Z]))\$$/
        Pat::DollarOneLowercaseLatinLetterDollarEnd => {
            if !char_at(s, 0, b'$') || !lower_at(s, 1) || s.len() < 3 {
                None
            } else {
                let c = decode(s, 2);
                if 2 + c.n + 1 == s.len()
                    && s[s.len() - 1] == b'$'
                    && c.c <= 0xFFFF
                    && !(c.c < 128 && is_alpha(c.c as u8))
                {
                    Some(group1(s, s.len(), &s[1..2]))
                } else {
                    None
                }
            }
        }
        Pat::OneLowercaseGreekLetterEnd => {
            if one_lowercase_greek_letter(s) {
                Some(m0(s, s.len()))
            } else {
                None
            }
        }
        // /^[0-9]+/
        Pat::Digits => {
            if digit_at(s, 0) {
                Some(m0(s, digits_end(s, 0)))
            } else {
                None
            }
        }
        // /^[+\-]?(?:[0-9]+(?:[,.][0-9]+)?|[0-9]*(?:\.[0-9]+))/
        Pat::N9DotComma9 => {
            let i: usize = if !s.is_empty() && is_sign(s[0]) { 1 } else { 0 };
            if digit_at(s, i) {
                let mut j = digits_end(s, i);
                if any_at(s, j, b",.") && digit_at(s, j + 1) {
                    j = digits_end(s, j + 1);
                }
                Some(m0(s, j))
            } else if char_at(s, i, b'.') && digit_at(s, i + 1) {
                Some(m0(s, digits_end(s, i + 1)))
            } else {
                None
            }
        }
        // /^[+\-]?[0-9]+(?:[.,][0-9]+)?/
        Pat::N9DotComma9NoMissing0 => {
            let i: usize = if !s.is_empty() && is_sign(s[0]) { 1 } else { 0 };
            if !digit_at(s, i) {
                None
            } else {
                let mut j = digits_end(s, i);
                if any_at(s, j, b",.") && digit_at(s, j + 1) {
                    j = digits_end(s, j + 1);
                }
                Some(m0(s, j))
            }
        }
        Pat::Enumber => {
            let mut g: [Str; 6] = std::array::from_fn(|_| Vec::new());
            let n = enumber_re(s, &mut g);
            if n == 0 {
                None
            } else {
                Some(Match {
                    m: Mv::Arr(g.into()),
                    rem: s[n..].to_vec(),
                })
            }
        }
        Pat::NumberPow => {
            let mut g: [Str; 3] = std::array::from_fn(|_| Vec::new());
            number_pow_re(s, &mut g).map(|n| Match {
                m: Mv::Arr(g.into()),
                rem: s[n..].to_vec(),
            })
        }
        // /^_\{(\([a-z]{1,3}\))\}/
        Pat::StateOfAggregationSubscriptEnd => {
            if !starts_at(s, 0, b"_{(") {
                None
            } else {
                let mut k: usize = 0;
                while lower_at(s, 3 + k) {
                    k += 1;
                }
                if !(1..=3).contains(&k) || !starts_at(s, 3 + k, b")}") {
                    None
                } else {
                    Some(group1(s, 3 + k + 2, &s[2..3 + k + 1]))
                }
            }
        }
        // /^(?:v|\(v\)|\^|\(\^\))(?=$|[\s,;\)\]\}])/
        Pat::ArrowUpDown => {
            let mut found = None;
            for alt in [&b"v"[..], &b"(v)"[..], &b"^"[..], &b"(^)"[..]] {
                if starts_at(s, 0, alt) && delimiter_or_end(s, alt.len()) {
                    found = Some(m0(s, alt.len()));
                    break;
                }
            }
            found
        }
        // /^(?:<->|<-->|->|<-|<=>>|<<=>|<=>|[→⟶⇌])/
        Pat::Arrow => {
            let mut found = None;
            for alt in [
                &b"<->"[..],
                &b"<-->"[..],
                &b"->"[..],
                &b"<-"[..],
                &b"<=>>"[..],
                &b"<<=>"[..],
                &b"<=>"[..],
                "→".as_bytes(),
                "⟶".as_bytes(),
                "⇌".as_bytes(),
            ] {
                if starts_at(s, 0, alt) {
                    found = Some(m0(s, alt.len()));
                    break;
                }
            }
            found
        }
        // /^[CMT](?=\[)/
        Pat::Cmt => {
            if any_at(s, 0, b"CMT") && char_at(s, 1, b'[') {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^(&|\\\\|\\hline)\s*/
        Pat::FirstLevelEscape => {
            let mut found = None;
            for alt in [&b"&"[..], &b"\\\\"[..], &b"\\hline"[..]] {
                if starts_at(s, 0, alt) {
                    found = Some(group1(s, skip_spaces(s, alt.len()), alt));
                    break;
                }
            }
            found
        }
        // /^(?:\\[,\ ;:])/  \\x - but output no space before
        Pat::BackslashComma => {
            if char_at(s, 0, b'\\') && any_at(s, 1, b", ;:") {
                Some(m0(s, 2))
            } else {
                None
            }
        }
        // /^\\ca(?:\s+|(?![a-zA-Z]))/
        Pat::BackslashCa => {
            if !starts_at(s, 0, b"\\ca") {
                None
            } else if space_len(s, 3) > 0 {
                Some(m0(s, skip_spaces(s, 3)))
            } else if not_letter_or_end(s, 3) {
                Some(m0(s, 3))
            } else {
                None
            }
        }
        // /^(?:\\[a-zA-Z]+\s*|\\[_&{}%])/
        Pat::Macro => {
            if !char_at(s, 0, b'\\') {
                None
            } else {
                let mut i: usize = 1;
                while i < s.len() && is_alpha(s[i]) {
                    i += 1;
                }
                if i > 1 {
                    Some(m0(s, skip_spaces(s, i)))
                } else if any_at(s, 1, b"_&{}%") {
                    Some(m0(s, 2))
                } else {
                    None
                }
            }
        }
        // only those with numbers in front, because the others will be formatted correctly anyway
        Pat::Orbital => opt0(s, orbital_end(s)),
        // /^[\/~|]/
        Pat::Others => {
            if any_at(s, 0, b"/~|") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        Pat::OxidationEnd => {
            if is_oxidation(s, false) {
                Some(m0(s, s.len()))
            } else {
                None
            }
        }
        // 0 could be oxidation or charge
        Pat::DOxidationEnd => {
            if is_oxidation(s, true) {
                Some(m0(s, s.len()))
            } else {
                None
            }
        }
        // /^[IVX]+/
        Pat::RomanNumeral => {
            let mut i: usize = 0;
            while i < s.len() && is_roman(s[i]) {
                i += 1;
            }
            if i == 0 { None } else { Some(m0(s, i)) }
        }
        Pat::HalfEnd => {
            if is_half(s) {
                Some(m0(s, s.len()))
            } else {
                None
            }
        }
        // /^(?:pH|pOH|pC|pK|iPr|iBu)(?=$|[^a-zA-Z])/
        Pat::UprightEntities => {
            let mut found = None;
            for alt in [
                &b"pH"[..],
                &b"pOH"[..],
                &b"pC"[..],
                &b"pK"[..],
                &b"iPr"[..],
                &b"iBu"[..],
            ] {
                if starts_at(s, 0, alt) && not_letter_or_end(s, alt.len()) {
                    found = Some(m0(s, alt.len()));
                    break;
                }
            }
            found
        }
        // /^\s*(\/)\s*/
        Pat::Slash => {
            let i = skip_spaces(s, 0);
            if !char_at(s, i, b'/') {
                None
            } else {
                Some(group1(s, skip_spaces(s, i + 1), &s[i..i + 1]))
            }
        }
        // /^\s*(\/\/)\s*/
        Pat::DoubleSlash => {
            let i = skip_spaces(s, 0);
            if !starts_at(s, i, b"//") {
                None
            } else {
                Some(group1(s, skip_spaces(s, i + 2), &s[i..i + 2]))
            }
        }
        // /^\s*[*.]\s*/
        Pat::Star => {
            let i = skip_spaces(s, 0);
            if !any_at(s, i, b"*.") {
                None
            } else {
                Some(m0(s, skip_spaces(s, i + 1)))
            }
        }

        _ => None,
    }
}
