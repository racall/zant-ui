use super::*;

/// Non-global String.prototype.match; global callers use match_all_global.
pub fn match_(s: &Str, re: &RegExp) -> Option<RegExpMatch> {
    re.exec(s)
}

/// Global String.prototype.match (no capture metadata).
pub fn match_all_global(s: &Str, re: &RegExp) -> Option<List<Str>> {
    let matches = collect(&Input::new(s), re);
    if matches.is_empty() {
        None
    } else {
        Some(matches.iter().map(|m| m.get(0.0).unwrap()).collect())
    }
}

/// Eager String.prototype.matchAll; the original lastIndex is preserved.
pub fn match_all(s: &Str, re: &RegExp) -> Result<List<RegExpMatch>, Thrown> {
    require_global(re, "matchAll")?;
    let copy = re.independent();
    Ok(collect_from(&Input::new(s), &copy).into())
}
fn require_global(re: &RegExp, method: &str) -> Result<(), Thrown> {
    if re.global() {
        Ok(())
    } else {
        Err(errors::error(
            "TypeError",
            format!("String.prototype.{method} called with a non-global RegExp argument"),
        ))
    }
}
fn collect(input: &Input, re: &RegExp) -> Vec<RegExpMatch> {
    if re.global() {
        re.set_last_index(0.0);
    }
    collect_from(input, re)
}
fn collect_from(input: &Input, re: &RegExp) -> Vec<RegExpMatch> {
    let mut result = Vec::new();
    while let Some(m) = re.exec_input(input) {
        let empty = m.start() == m.end();
        result.push(m);
        if !re.global() {
            break;
        }
        if empty {
            re.set_last_index(
                advance(&input.units, to_length(re.last_index()), re.full_unicode()) as f64,
            );
        }
    }
    result
}
/// String.prototype.replace with full GetSubstitution expansion.
pub fn replace(s: &Str, re: &RegExp, replacement: &str) -> Str {
    replaced(s, re, |m| substitution::expand(m, replacement))
}
/// String.prototype.replaceAll; requires the global flag.
pub fn replace_all(s: &Str, re: &RegExp, replacement: &str) -> Result<Str, Thrown> {
    require_global(re, "replaceAll")?;
    Ok(replace(s, re, replacement))
}
/// Matches are collected before callbacks, as in RegExp.prototype[@@replace].
pub fn replace_with(s: &Str, re: &RegExp, mut f: impl FnMut(&RegExpMatch) -> Str) -> Str {
    replaced(s, re, |m| f(m).encode_utf16().collect())
}
pub fn replace_all_with(
    s: &Str,
    re: &RegExp,
    f: impl FnMut(&RegExpMatch) -> Str,
) -> Result<Str, Thrown> {
    require_global(re, "replaceAll")?;
    Ok(replace_with(s, re, f))
}
fn replaced(s: &Str, re: &RegExp, mut f: impl FnMut(&RegExpMatch) -> Vec<u16>) -> Str {
    let input = Input::new(s);
    let matches = collect(&input, re);
    let mut out = Vec::new();
    let mut end = 0;
    for m in matches {
        let replacement = f(&m);
        if m.start() >= end {
            out.extend_from_slice(&input.units[end..m.start()]);
            out.extend(replacement);
            end = m.end();
        }
    }
    out.extend_from_slice(&input.units[end..]);
    string(&out)
}
/// Search starts at zero and restores lastIndex, including NaN and negative zero.
pub fn search(s: &Str, re: &RegExp) -> f64 {
    let old = re.last_index();
    re.set_last_index(0.0);
    let index = re.exec(s).map_or(-1.0, |m| m.index());
    re.set_last_index(old);
    index
}
/// RegExp split, using a private sticky matcher. Unmatched captures become "".
pub fn split(s: &Str, re: &RegExp, limit: Option<f64>) -> List<Str> {
    let limit = limit.map_or(u32::MAX as usize, |n| {
        if !n.is_finite() || n == 0.0 {
            0
        } else {
            n.trunc().rem_euclid(4294967296.0) as usize
        }
    });
    let mut out = Vec::new();
    if limit == 0 {
        return out.into();
    }
    let input = Input::new(s);
    let size = input.units.len();
    if size == 0 {
        if re.find(&input, 0, true).is_none() {
            out.push(s.clone());
        }
        return out.into();
    }
    let mut p = 0;
    let mut q = 0;
    while q < size {
        let Some(m) = re.find(&input, q, true) else {
            q = advance(&input.units, q, re.full_unicode());
            continue;
        };
        let end = m.end();
        if end == p {
            q = advance(&input.units, q, re.full_unicode());
            continue;
        }
        out.push(string(&input.units[p..q]));
        if out.len() == limit {
            return out.into();
        }
        p = end;
        for capture in &m.0.found.captures {
            out.push(
                capture
                    .clone()
                    .map_or(Str::Static(""), |r| string(&input.units[r])),
            );
            if out.len() == limit {
                return out.into();
            }
        }
        q = p;
    }
    out.push(string(&input.units[p..]));
    out.into()
}
