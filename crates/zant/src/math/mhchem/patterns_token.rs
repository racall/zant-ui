//! Short token patterns (`patterns.rs` split): whitespace, punctuation,
//! operators and simple script markers.

use super::chars::*;
use super::patterns::Pat;
use super::regexes::delimiter_or_end;
use super::types::*;

pub fn match_token(pat: Pat, s: &[u8]) -> Option<Match> {
    match pat {
        Pat::Empty => {
            if s.is_empty() {
                Some(m0(s, 0))
            } else {
                None
            }
        }
        // /^./
        Pat::Else | Pat::Else2 => {
            if s.is_empty() {
                None
            } else {
                let c = decode(s, 0);
                if is_line_terminator(c.c) {
                    None
                } else {
                    Some(m0(s, c.n))
                }
            }
        }
        // /^\s/
        Pat::Space => opt0(
            s,
            if space_len(s, 0) > 0 {
                Some(space_len(s, 0))
            } else {
                None
            },
        ),
        // /^\s(?=[A-Z\\$])/
        Pat::SpaceA => {
            let n = space_len(s, 0);
            if n == 0 {
                None
            } else if n < s.len() && (is_upper(s[n]) || s[n] == b'\\' || s[n] == b'$') {
                Some(m0(s, n))
            } else {
                None
            }
        }
        // /^\s$/
        Pat::SpaceEnd => {
            if !s.is_empty() && space_len(s, 0) == s.len() {
                Some(m0(s, s.len()))
            } else {
                None
            }
        }
        // /^[a-z]/
        Pat::AZ => {
            if lower_at(s, 0) {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^x/
        Pat::X => {
            if char_at(s, 0, b'x') {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^x$/
        Pat::XEnd => {
            if eql(s, b"x") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^i$/
        Pat::IEnd => {
            if eql(s, b"i") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^(?:\\\{|\[|\()/
        Pat::OpenGroup => {
            if starts_at(s, 0, b"\\{") {
                Some(m0(s, 2))
            } else if any_at(s, 0, b"[(") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^(?:\)|\]|\\\})/
        Pat::CloseGroup => {
            if any_at(s, 0, b")]") {
                Some(m0(s, 1))
            } else if starts_at(s, 0, b"\\}") {
                Some(m0(s, 2))
            } else {
                None
            }
        }
        // /^[,;]\s*/
        Pat::CommaSpace => {
            if any_at(s, 0, b",;") {
                Some(m0(s, skip_spaces(s, 1)))
            } else {
                None
            }
        }
        // /^[,;]/
        Pat::Comma => {
            if any_at(s, 0, b",;") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^[.]/
        Pat::Dot => {
            if char_at(s, 0, b'.') {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^([.⋅·•])\s*/
        Pat::DotSpace => {
            if s.is_empty() {
                None
            } else {
                let c = decode(s, 0);
                if c.c != 0x2E && c.c != 0x22C5 && c.c != 0xB7 && c.c != 0x2022 {
                    None
                } else {
                    Some(group1(s, skip_spaces(s, c.n), &s[0..c.n]))
                }
            }
        }
        // /^\.\.\.(?=$|[^.])/
        Pat::Ellipsis => {
            if starts_at(s, 0, b"...") && !char_at(s, 3, b'.') {
                Some(m0(s, 3))
            } else {
                None
            }
        }
        // /^([*])\s*/
        Pat::StarSpace => {
            if char_at(s, 0, b'*') {
                Some(group1(s, skip_spaces(s, 1), &s[0..1]))
            } else {
                None
            }
        }
        // /^\^([0-9]+|[^\\_])/
        Pat::CaretA => {
            if !char_at(s, 0, b'^') || s.len() < 2 {
                None
            } else if digit_at(s, 1) {
                let e = digits_end(s, 1);
                Some(group1(s, e, &s[1..e]))
            } else if s[1] == b'\\' || s[1] == b'_' {
                None
            } else {
                let c = decode(s, 1);
                Some(group1(s, 1 + c.n, &s[1..1 + c.n]))
            }
        }
        // /^\^(\\[a-zA-Z]+)\s*/
        Pat::CaretMacro => {
            if !starts_at(s, 0, b"^\\") {
                None
            } else {
                let mut i: usize = 2;
                while i < s.len() && is_alpha(s[i]) {
                    i += 1;
                }
                if i == 2 {
                    None
                } else {
                    Some(group1(s, skip_spaces(s, i), &s[1..i]))
                }
            }
        }
        // /^\^(-?\d+)/
        Pat::CaretNegativeOne => {
            if !char_at(s, 0, b'^') {
                None
            } else {
                let i: usize = if char_at(s, 1, b'-') { 2 } else { 1 };
                if !digit_at(s, i) {
                    None
                } else {
                    let e = digits_end(s, i);
                    Some(group1(s, e, &s[1..e]))
                }
            }
        }
        // /^'/
        Pat::Prime => {
            if char_at(s, 0, b'\'') {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^_([+\-]?[0-9]+|[^\\])/
        Pat::Underscore9 => {
            if !char_at(s, 0, b'_') || s.len() < 2 {
                None
            } else {
                let i: usize = if is_sign(s[1]) { 2 } else { 1 };
                if digit_at(s, i) {
                    let e = digits_end(s, i);
                    Some(group1(s, e, &s[1..e]))
                } else if s[1] == b'\\' {
                    None
                } else {
                    let c = decode(s, 1);
                    Some(group1(s, 1 + c.n, &s[1..1 + c.n]))
                }
            }
        }
        // /^_(\\[a-zA-Z]+)\s*/
        Pat::UnderscoreMacro => {
            if !starts_at(s, 0, b"_\\") {
                None
            } else {
                let mut i: usize = 2;
                while i < s.len() && is_alpha(s[i]) {
                    i += 1;
                }
                if i == 2 {
                    None
                } else {
                    Some(group1(s, skip_spaces(s, i), &s[1..i]))
                }
            }
        }
        // /^(?:\^(?=_)|\_(?=\^)|[\^_]$)/
        Pat::CaretUnderscore => {
            if starts_at(s, 0, b"^_") || starts_at(s, 0, b"_^") || eql(s, b"^") || eql(s, b"_") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^\{\}/
        Pat::EmptyBraces => {
            if starts_at(s, 0, b"{}") {
                Some(m0(s, 2))
            } else {
                None
            }
        }
        // /^[=<>]/
        Pat::EqLtGt => {
            if any_at(s, 0, b"=<>") {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^[#≡]/
        Pat::Hash => {
            if char_at(s, 0, b'#') {
                Some(m0(s, 1))
            } else if starts_at(s, 0, "≡".as_bytes()) {
                Some(m0(s, 3))
            } else {
                None
            }
        }
        // /^\+/
        Pat::Plus => {
            if char_at(s, 0, b'+') {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^-(?=[\s_},;\]/]|$|\([a-z]+\))/  -space -, -; -] -/ -$ -state-of-aggregation
        Pat::MinusEnd => {
            if !char_at(s, 0, b'-') {
                None
            } else if s.len() == 1 || space_len(s, 1) > 0 || any_at(s, 1, b"_},;]/") {
                Some(m0(s, 1))
            } else if char_at(s, 1, b'(') && lower_at(s, 2) {
                let mut i: usize = 2;
                while lower_at(s, i) {
                    i += 1;
                }
                if char_at(s, i, b')') {
                    Some(m0(s, 1))
                } else {
                    None
                }
            } else {
                None
            }
        }
        // /^-(?=[0-9])/
        Pat::Minus9 => {
            if char_at(s, 0, b'-') && digit_at(s, 1) {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^-(?=(?:[spd]|sp)(?:$|[\s,;\)\]\}]))/
        Pat::MinusOrbitalOverlap => {
            if !char_at(s, 0, b'-') {
                None
            } else if (any_at(s, 1, b"spd") && delimiter_or_end(s, 2))
                || (starts_at(s, 1, b"sp") && delimiter_or_end(s, 3))
            {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^-/
        Pat::Minus => {
            if char_at(s, 0, b'-') {
                Some(m0(s, 1))
            } else {
                None
            }
        }
        // /^(?:\\pm|\$\\pm\$|\+-|\+\/-)/
        Pat::PmOperator => {
            let mut found = None;
            for alt in [&b"\\pm"[..], &b"$\\pm$"[..], &b"+-"[..], &b"+/-"[..]] {
                if starts_at(s, 0, alt) {
                    found = Some(m0(s, alt.len()));
                    break;
                }
            }
            found
        }
        // /^(?:\+|(?:[\-=<>]|<<|>>|\\approx|\$\\approx\$)(?=\s|$|-?[0-9]))/
        Pat::Operator => {
            if char_at(s, 0, b'+') {
                Some(m0(s, 1))
            } else {
                let ok = |j: usize| {
                    j == s.len()
                        || space_len(s, j) > 0
                        || digit_at(s, j)
                        || (char_at(s, j, b'-') && digit_at(s, j + 1))
                };
                if any_at(s, 0, b"-=<>") && ok(1) {
                    Some(m0(s, 1))
                } else {
                    let mut found = None;
                    for alt in [&b"<<"[..], &b">>"[..], &b"\\approx"[..], &b"$\\approx$"[..]] {
                        if starts_at(s, 0, alt) && ok(alt.len()) {
                            found = Some(m0(s, alt.len()));
                            break;
                        }
                    }
                    found
                }
            }
        }
        _ => None,
    }
}
