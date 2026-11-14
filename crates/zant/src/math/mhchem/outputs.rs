//! Output builders: `ce_output` / `pu_output` / `pu2_output` / `output9`,
//! `enumber`, `half` and the `- after o/d` action.
//!
//! Port of `src/math/mhchem.zig` (the remaining Parser action helpers).

use super::chars::{
    char_at, digit_at, digits_end, eql, is_sign, opt_eql, replace_all, replace_alternatives,
    trim_bytes, truthy,
};
use super::patterns::{Pat, match_pattern};
use super::regexes::{frac_numerator_end, frac_suffix};
use super::types::{Buffer, Item, Items, Mv, Ret, Sm, Str};
use super::{MhchemError, cat, concat_array, go};

/// `ret.push("\\pm ")` / `ret.push(m[0])` in 'enumber' and 'number^'.
pub fn push_sign(ret: &mut Items, sign: &[u8]) {
    if eql(sign, b"+-") || eql(sign, b"+/-") {
        ret.push(Item::Str(b"\\pm ".to_vec()));
    } else if !sign.is_empty() {
        ret.push(Item::Str(sign.to_vec()));
    }
}

/// `mhchemParser.actions['1/2']`
pub fn half(m0_: &[u8]) -> Result<Ret, MhchemError> {
    let mut ret: Items = Vec::new();
    let mut m = m0_;
    if !m.is_empty() && is_sign(m[0]) {
        ret.push(Item::Str(m[..1].to_vec()));
        m = &m[1..];
    }
    // /^([0-9]+|\$[a-z]\$|[a-z])\/([0-9]+)(\$[a-z]\$|[a-z])?$/
    let e1 = frac_numerator_end(m, 0).ok_or(MhchemError)?;
    if !char_at(m, e1, b'/') || !digit_at(m, e1 + 1) {
        return Err(MhchemError);
    }
    let e2 = digits_end(m, e1 + 1);
    let n3 = frac_suffix(m, e2).ok_or(MhchemError)?;
    let n1 = replace_all(&m[..e1], b"$", b"");
    ret.push(Item::Frac([n1, m[e1 + 1..e2].to_vec()]));
    if let Some(x) = n3 {
        ret.push(Item::TexMath(replace_all(&x, b"$", b"")));
    }
    Ok(Ret::Many(ret))
}

/// `stateMachines.ce.actions['output']`
pub fn ce_output(buffer: &mut Buffer, entity_follows: u8) -> Result<Ret, MhchemError> {
    // entityFollows:
    //   undefined = if we have nothing else to output, also ignore the just read space (buffer.sb)
    //   1 = an entity follows, never omit the space if there was one just read before (can only apply to state 1)
    //   2 = 1 + the entity can have an amount, so output a\, instead of converting it to o (can only apply to states a|as)
    let ret: Ret;
    if !truthy(&buffer.r) {
        let mut out: Items = Vec::new();
        {
            let b = &mut *buffer;
            let nothing = !truthy(&b.a)
                && !truthy(&b.b)
                && !truthy(&b.p)
                && !truthy(&b.o)
                && !truthy(&b.q)
                && !truthy(&b.d)
                && entity_follows == 0;
            if !nothing {
                if b.sb {
                    out.push(Item::EntitySkip);
                }
                if !truthy(&b.o)
                    && !truthy(&b.q)
                    && !truthy(&b.d)
                    && !truthy(&b.b)
                    && !truthy(&b.p)
                    && entity_follows != 2
                {
                    b.o = b.a.take();
                } else if !truthy(&b.o)
                    && !truthy(&b.q)
                    && !truthy(&b.d)
                    && (truthy(&b.b) || truthy(&b.p))
                {
                    b.o = b.a.take();
                    b.d = b.b.take();
                    b.q = b.p.take();
                } else if truthy(&b.o)
                    && opt_eql(&b.d_type, b"kv")
                    && match_pattern(Pat::DOxidationEnd, b.d.as_deref().unwrap_or(b""))?.is_some()
                {
                    b.d_type = Some(b"oxidation".to_vec());
                } else if truthy(&b.o) && opt_eql(&b.d_type, b"kv") && !truthy(&b.q) {
                    b.d_type = None;
                }
                let cf = super::types::Chemfive {
                    a: go(b.a.take(), Sm::A)?,
                    b: go(b.b.take(), Sm::Bd)?,
                    p: go(b.p.take(), Sm::Pq)?,
                    o: go(b.o.take(), Sm::O)?,
                    q: go(b.q.take(), Sm::Pq)?,
                    d: go(
                        b.d.take(),
                        if opt_eql(&b.d_type, b"oxidation") {
                            Sm::Oxidation
                        } else {
                            Sm::Bd
                        },
                    )?,
                    d_type: b.d_type.clone(),
                };
                out.push(Item::Chemfive(Box::new(cf)));
            }
        }
        ret = Ret::Many(out);
    } else {
        // r
        let rd: Items = if opt_eql(&buffer.rdt, b"M") {
            go(buffer.rd.take(), Sm::TexMath)?
        } else if opt_eql(&buffer.rdt, b"T") {
            vec![Item::Text(buffer.rd.clone().unwrap_or_default())]
        } else {
            go(buffer.rd.take(), Sm::Ce)?
        };
        let rq: Items = if opt_eql(&buffer.rqt, b"M") {
            go(buffer.rq.take(), Sm::TexMath)?
        } else if opt_eql(&buffer.rqt, b"T") {
            vec![Item::Text(buffer.rq.clone().unwrap_or_default())]
        } else {
            go(buffer.rq.take(), Sm::Ce)?
        };
        ret = Ret::One(Item::Arrow {
            r: buffer.r.clone().unwrap_or_default(),
            rd,
            rq,
        });
    }
    let level = buffer.parenthesis_level;
    let bond = buffer.begins_with_bond;
    *buffer = Buffer {
        parenthesis_level: level,
        begins_with_bond: bond,
        ..Buffer::default()
    };
    Ok(ret)
}

/// `stateMachines.ce.actions['- after o/d']`
pub fn after_od(buffer: &mut Buffer, m: &[u8], is_after_d: bool) -> Result<Ret, MhchemError> {
    let o = buffer.o.clone().unwrap_or_default();
    let mut c1 = match_pattern(Pat::Orbital, &o)?;
    let c2 = match_pattern(Pat::OneLowercaseGreekLetterEnd, &o)?;
    let c3 = match_pattern(Pat::OneLowercaseLatinLetterEnd, &o)?;
    let c4 = match_pattern(Pat::DollarOneLowercaseLatinLetterDollarEnd, &o)?;
    let hyphen_follows = eql(m, b"-")
        && ((c1.as_ref().is_some_and(|c| c.rem.is_empty()))
            || c2.is_some()
            || c3.is_some()
            || c4.is_some());
    if hyphen_follows
        && !truthy(&buffer.a)
        && !truthy(&buffer.b)
        && !truthy(&buffer.p)
        && !truthy(&buffer.d)
        && !truthy(&buffer.q)
        && c1.is_none()
        && c3.is_some()
    {
        buffer.o = Some(cat(&[
            b"$",
            buffer.o.as_deref().unwrap_or(b"undefined"),
            b"$",
        ]));
    }
    let mut ret: Items = Vec::new();
    if hyphen_follows {
        concat_array(&mut ret, ce_output(buffer, 0)?);
        ret.push(Item::Hyphen);
    } else {
        c1 = match_pattern(Pat::Digits, buffer.d.as_deref().unwrap_or(b""))?;
        if is_after_d && c1.as_ref().is_some_and(|c| c.rem.is_empty()) {
            buffer.d = Some(cat(&[buffer.d.as_deref().unwrap_or(b""), m]));
            concat_array(&mut ret, ce_output(buffer, 0)?);
        } else {
            concat_array(&mut ret, ce_output(buffer, 0)?);
            ret.push(Item::Bond(b"-".to_vec()));
        }
    }
    Ok(Ret::Many(ret))
}

/// `stateMachines.pu.actions['enumber']`
pub fn enumber(m: Mv) -> Result<Ret, MhchemError> {
    let arr = match &m {
        Mv::Arr(arr) => arr,
        Mv::S(_) => return Err(MhchemError),
    };
    if arr.len() < 6 {
        return Err(MhchemError);
    }
    let mut g: [Str; 6] = std::array::from_fn(|i| arr[i].clone());
    let mut ret: Items = Vec::new();
    push_sign(&mut ret, &g[0]);
    if !g[1].is_empty() {
        concat_array(&mut ret, Ret::Many(go(Some(g[1].clone()), Sm::Pu99)?));
        if !g[2].is_empty() {
            if g[2].contains(&b',') || g[2].contains(&b'.') {
                concat_array(&mut ret, Ret::Many(go(Some(g[2].clone()), Sm::Pu99)?));
            } else {
                ret.push(Item::Str(g[2].clone()));
            }
        }
        g[3] = if !g[4].is_empty() {
            g[4].clone()
        } else {
            g[3].clone()
        };
        if !g[3].is_empty() {
            g[3] = trim_bytes(&g[3], b" \t\n\r\x0B\x0C").to_vec();
            if eql(&g[3], b"e") || (!g[3].is_empty() && g[3][0] == b'*') {
                ret.push(Item::Cdot);
            } else {
                ret.push(Item::Times);
            }
        }
    }
    if !g[3].is_empty() {
        ret.push(Item::Str(cat(&[b"10^{", &g[5], b"}"])));
    }
    Ok(Ret::Many(ret))
}

/// `stateMachines.pu.actions['output']`
pub fn pu_output(buffer: &mut Buffer) -> Result<Ret, MhchemError> {
    if let Some(md) = match_pattern(Pat::BracedParen, buffer.d.as_deref().unwrap_or(b""))?
        && md.rem.is_empty()
    {
        buffer.d = Some(md.m.as_str().unwrap_or_default().to_vec());
    }
    if let Some(mq) = match_pattern(Pat::BracedParen, buffer.q.as_deref().unwrap_or(b""))?
        && mq.rem.is_empty()
    {
        buffer.q = Some(mq.m.as_str().unwrap_or_default().to_vec());
    }
    const CELSIUS: [&[u8]; 3] = [b"\xC2\xB0C", b"^oC", b"^{o}C"];
    const FAHRENHEIT: [&[u8]; 3] = [b"\xC2\xB0F", b"^oF", b"^{o}F"];
    if truthy(&buffer.d) {
        let d = replace_alternatives(
            buffer.d.as_deref().unwrap_or(b""),
            &CELSIUS,
            b"{}^{\\circ}C",
        );
        buffer.d = Some(replace_alternatives(&d, &FAHRENHEIT, b"{}^{\\circ}F"));
    }
    let ret: Ret;
    if truthy(&buffer.q) {
        // fraction
        let q = replace_alternatives(
            buffer.q.as_deref().unwrap_or(b""),
            &CELSIUS,
            b"{}^{\\circ}C",
        );
        buffer.q = Some(replace_alternatives(&q, &FAHRENHEIT, b"{}^{\\circ}F"));
        let b5d = go(buffer.d.take(), Sm::Pu)?;
        let b5q = go(buffer.q.take(), Sm::Pu)?;
        if opt_eql(&buffer.o, b"//") {
            ret = Ret::One(Item::PuFrac([b5d, b5q]));
        } else {
            let many = b5d.len() > 1 || b5q.len() > 1;
            let mut out: Items = Vec::new();
            out.extend(b5d);
            if many {
                out.push(Item::SpacedSlash);
            } else {
                out.push(Item::Slash);
            }
            out.extend(b5q);
            ret = Ret::Many(out);
        }
    } else {
        // no fraction
        ret = Ret::Many(go(buffer.d.take(), Sm::Pu2)?);
    }
    *buffer = Buffer::default();
    Ok(ret)
}

/// `stateMachines['pu-2'].actions['output']`
pub fn pu2_output(buffer: &mut Buffer) -> Result<Ret, MhchemError> {
    let mut ret: Ret = Ret::Many(Vec::new());
    if truthy(&buffer.rm) {
        let mrm = match_pattern(Pat::BracedParen, buffer.rm.as_deref().unwrap_or(b""))?;
        if let Some(md) = mrm {
            if md.rem.is_empty() {
                ret = Ret::Many(go(
                    Some(md.m.as_str().unwrap_or_default().to_vec()),
                    Sm::Pu,
                )?);
            } else {
                ret = Ret::One(Item::Rm(buffer.rm.clone().unwrap_or_default()));
            }
        } else {
            ret = Ret::One(Item::Rm(buffer.rm.clone().unwrap_or_default()));
        }
    }
    *buffer = Buffer::default();
    Ok(ret)
}

/// `stateMachines['pu-9,9'].actions['output-0']` / `['output-o']`
pub fn output9(buffer: &mut Buffer, is_o: bool) -> Ret {
    let mut ret: Items = Vec::new();
    let t = buffer.text.take().unwrap_or_default();
    if t.len() > 4 {
        if !is_o {
            let mut a = t.len() % 3;
            if a == 0 {
                a = 3;
            }
            // pushed in reverse: substr(i,3), separator, ..., substr(0,a); then reversed
            let mut tmp: Items = Vec::new();
            let mut i: isize = t.len() as isize - 3;
            while i > 0 {
                let u = i as usize;
                tmp.push(Item::Str(t[u..usize::min(u + 3, t.len())].to_vec()));
                tmp.push(Item::ThousandSeparator);
                i -= 3;
            }
            tmp.push(Item::Str(t[0..a].to_vec()));
            tmp.reverse();
            ret = tmp;
        } else {
            let a = t.len() - 3;
            let mut i: usize = 0;
            while i < a {
                ret.push(Item::Str(t[i..usize::min(i + 3, t.len())].to_vec()));
                ret.push(Item::ThousandSeparator);
                i += 3;
            }
            ret.push(Item::Str(t[i..].to_vec()));
        }
    } else {
        ret.push(Item::Str(t));
    }
    *buffer = Buffer::default();
    Ret::Many(ret)
}
