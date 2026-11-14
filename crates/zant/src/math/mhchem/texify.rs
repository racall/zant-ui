//! `texify.go` / `texify._go2`: turn the parser output into TeX.

use super::chars::{eql, is_sign, opt_eql, replace_first};
use super::types::{Item, Items, Str};
use super::{MhchemError, cat};

/// `texify.go(input, isInner)` (recursive, max 4 levels)
pub fn texify(input: &Items, is_inner: bool) -> Result<Str, MhchemError> {
    let mut res: Str = Vec::new();
    let mut cee = false;
    for inputi in input {
        match inputi {
            Item::Str(s) => res.extend_from_slice(s),
            _ => {
                res.extend(go2(inputi)?);
                if matches!(inputi, Item::FirstLevelEscape(_)) {
                    cee = true;
                }
            }
        }
    }
    if !is_inner && !cee && !res.is_empty() {
        return Ok(cat(&[b"{", &res, b"}"]));
    }
    Ok(res)
}

/// `texify._goInner`
fn inner(input: &Items) -> Result<Str, MhchemError> {
    texify(input, true)
}

/// `texify._go2`
fn go2(buf: &Item) -> Result<Str, MhchemError> {
    Ok(match buf {
        Item::Str(s) => s.clone(),
        Item::Chemfive(c) => {
            let mut res: Str = Vec::new();
            let mut a = inner(&c.a)?;
            let b = inner(&c.b)?;
            let p = inner(&c.p)?;
            let mut o = inner(&c.o)?;
            let q = inner(&c.q)?;
            let d = inner(&c.d)?;
            //
            // a
            //
            if !a.is_empty() {
                if is_sign(a[0]) {
                    a = cat(&[b"{", &a, b"}"]);
                }
                res.extend_from_slice(&a);
                res.extend_from_slice(b"\\,");
            }
            //
            // b and p
            //
            if !b.is_empty() || !p.is_empty() {
                res.extend(cat(&[
                    b"{\\vphantom{X}}",
                    b"^{\\hphantom{",
                    &b,
                    b"}}_{\\hphantom{",
                    &p,
                    b"}}",
                    b"{\\vphantom{X}}",
                    b"^{\\smash[t]{\\vphantom{2}}\\mathllap{",
                    &b,
                    b"}}",
                    b"_{\\vphantom{2}\\mathllap{\\smash[t]{",
                    &p,
                    b"}}}",
                ]));
            }
            //
            // o
            //
            if !o.is_empty() {
                if is_sign(o[0]) {
                    o = cat(&[b"{", &o, b"}"]);
                }
                res.extend_from_slice(&o);
            }
            //
            // q and d
            //
            if opt_eql(&c.d_type, b"kv") {
                if !d.is_empty() || !q.is_empty() {
                    res.extend_from_slice(b"{\\vphantom{X}}");
                }
                if !d.is_empty() {
                    res.extend(cat(&[b"^{", &d, b"}"]));
                }
                if !q.is_empty() {
                    res.extend(cat(&[b"_{\\smash[t]{", &q, b"}}"]));
                }
            } else if opt_eql(&c.d_type, b"oxidation") {
                if !d.is_empty() {
                    res.extend(cat(&[b"{\\vphantom{X}}", b"^{", &d, b"}"]));
                }
                if !q.is_empty() {
                    res.extend(cat(&[b"{\\vphantom{X}}", b"_{\\smash[t]{", &q, b"}}"]));
                }
            } else {
                if !q.is_empty() {
                    res.extend(cat(&[b"{\\vphantom{X}}", b"_{\\smash[t]{", &q, b"}}"]));
                }
                if !d.is_empty() {
                    res.extend(cat(&[b"{\\vphantom{X}}", b"^{", &d, b"}"]));
                }
            }
            res
        }
        Item::Rm(p1) => cat(&[b"\\mathrm{", p1, b"}"]),
        Item::Text(p1) => {
            if p1.iter().any(|&c| c == b'^' || c == b'_') {
                let t = replace_first(&replace_first(p1, b" ", b"~"), b"-", b"\\text{-}");
                cat(&[b"\\mathrm{", &t, b"}"])
            } else {
                cat(&[b"\\text{", p1, b"}"])
            }
        }
        Item::RomanNumeral(p1) => cat(&[b"\\mathrm{", p1, b"}"]),
        Item::StateOfAggregation(p1) => cat(&[b"\\mskip2mu ", &inner(p1)?]),
        Item::StateOfAggregationSubscript(p1) => cat(&[b"\\mskip1mu ", &inner(p1)?]),
        Item::Bond(kind) => get_bond(kind).ok_or(MhchemError)?, // "MhchemErrorBond"
        Item::Frac(f) => {
            let c = cat(&[b"\\frac{", &f[0], b"}{", &f[1], b"}"]);
            cat(&[
                b"\\mathchoice{\\textstyle",
                &c,
                b"}{",
                &c,
                b"}{",
                &c,
                b"}{",
                &c,
                b"}",
            ])
        }
        Item::PuFrac(f) => {
            let d = cat(&[b"\\frac{", &inner(&f[0])?, b"}{", &inner(&f[1])?, b"}"]);
            cat(&[
                b"\\mathchoice{\\textstyle",
                &d,
                b"}{",
                &d,
                b"}{",
                &d,
                b"}{",
                &d,
                b"}",
            ])
        }
        Item::TexMath(p1) => cat(&[p1, b" "]),
        Item::FracCe(f) => cat(&[b"\\frac{", &inner(&f[0])?, b"}{", &inner(&f[1])?, b"}"]),
        Item::Overset(f) => cat(&[b"\\overset{", &inner(&f[0])?, b"}{", &inner(&f[1])?, b"}"]),
        Item::Underset(f) => cat(&[b"\\underset{", &inner(&f[0])?, b"}{", &inner(&f[1])?, b"}"]),
        Item::Underbrace(f) => cat(&[
            b"\\underbrace{",
            &inner(&f[0])?,
            b"}_{",
            &inner(&f[1])?,
            b"}",
        ]),
        Item::Color { color1, color2 } => {
            cat(&[b"{\\color{", color1, b"}{", &inner(color2)?, b"}}"])
        }
        Item::Color0(c) => cat(&[b"\\color{", c, b"}"]),
        Item::Arrow { r, rd, rq } => {
            let rd = inner(rd)?;
            let rq = inner(rq)?;
            let name = get_arrow(r).ok_or(MhchemError)?;
            let mut arrow: Str = Vec::new();
            arrow.extend_from_slice(b"\\x");
            arrow.extend_from_slice(&name);
            if !rq.is_empty() {
                arrow.extend(cat(&[b"[{", &rq, b"}]"]));
            }
            if !rd.is_empty() {
                arrow.extend(cat(&[b"{", &rd, b"}"]));
            } else {
                arrow.extend_from_slice(b"{}");
            }
            arrow
        }
        Item::Operator(kind) => get_operator(kind).ok_or(MhchemError)?,
        Item::FirstLevelEscape(p1) => cat(&[p1, b" "]), // &, \\\\, \\hlin
        Item::Space => b" ".to_vec(),
        Item::EntitySkip => b"~".to_vec(),
        Item::PuSpace1 => b"~".to_vec(),
        Item::PuSpace2 => b"\\mkern3mu ".to_vec(),
        Item::ThousandSeparator => b"\\mkern2mu ".to_vec(),
        Item::CommaDecimal => b"{,}".to_vec(),
        Item::CommaEnumerationL(p1) => cat(&[b"{", p1, b"}\\mkern6mu "]),
        Item::CommaEnumerationM(p1) => cat(&[b"{", p1, b"}\\mkern3mu "]),
        Item::CommaEnumerationS(p1) => cat(&[b"{", p1, b"}\\mkern1mu "]),
        Item::Hyphen => b"\\text{-}".to_vec(),
        Item::AdditionCompound => b"\\,{\\cdot}\\,".to_vec(),
        Item::ElectronDot => b"\\mkern1mu \\bullet\\mkern1mu ".to_vec(),
        Item::KvX => b"{\\times}".to_vec(),
        Item::Prime => b"\\prime ".to_vec(),
        Item::Cdot => b"\\cdot ".to_vec(),
        Item::TightCdot => b"\\mkern1mu{\\cdot}\\mkern1mu ".to_vec(),
        Item::Times => b"\\times ".to_vec(),
        Item::Circa => b"{\\sim}".to_vec(),
        Item::Up => b"uparrow".to_vec(),
        Item::V => b"downarrow".to_vec(),
        Item::Ellipsis => b"\\ldots ".to_vec(),
        Item::Slash => b"/".to_vec(),
        Item::SpacedSlash => b"\\,/\\,".to_vec(),
    })
}

/// `texify._getArrow`
fn get_arrow(a: &[u8]) -> Option<Str> {
    let table: [(&[u8], &[u8]); 10] = [
        (b"->", b"rightarrow"),
        ("→".as_bytes(), b"rightarrow"),
        ("⟶".as_bytes(), b"rightarrow"),
        (b"<-", b"leftarrow"),
        (b"<->", b"leftrightarrow"),
        (b"<-->", b"rightleftarrows"),
        (b"<=>", b"rightleftharpoons"),
        ("⇌".as_bytes(), b"rightleftharpoons"),
        (b"<=>>", b"rightequilibrium"),
        (b"<<=>", b"leftequilibrium"),
    ];
    for (key, value) in table {
        if eql(a, key) {
            return Some(value.to_vec());
        }
    }
    None
}

/// `texify._getBond`
fn get_bond(a: &[u8]) -> Option<Str> {
    let table: [(&[u8], &[u8]); 17] = [
        (b"-", b"{-}"),
        (b"1", b"{-}"),
        (b"=", b"{=}"),
        (b"2", b"{=}"),
        (b"#", b"{\\equiv}"),
        (b"3", b"{\\equiv}"),
        (b"~", b"{\\tripledash}"),
        (
            b"~-",
            b"{\\mathrlap{\\raisebox{-.1em}{$-$}}\\raisebox{.1em}{$\\tripledash$}}",
        ),
        (
            b"~=",
            b"{\\mathrlap{\\raisebox{-.2em}{$-$}}\\mathrlap{\\raisebox{.2em}{$\\tripledash$}}-}",
        ),
        (
            b"~--",
            b"{\\mathrlap{\\raisebox{-.2em}{$-$}}\\mathrlap{\\raisebox{.2em}{$\\tripledash$}}-}",
        ),
        (
            b"-~-",
            b"{\\mathrlap{\\raisebox{-.2em}{$-$}}\\mathrlap{\\raisebox{.2em}{$-$}}\\tripledash}",
        ),
        (b"...", b"{{\\cdot}{\\cdot}{\\cdot}}"),
        (b"....", b"{{\\cdot}{\\cdot}{\\cdot}{\\cdot}}"),
        (b"->", b"{\\rightarrow}"),
        (b"<-", b"{\\leftarrow}"),
        (b"<", b"{<}"),
        (b">", b"{>}"),
    ];
    for (key, value) in table {
        if eql(a, key) {
            return Some(value.to_vec());
        }
    }
    None
}

/// `texify._getOperator`
fn get_operator(a: &[u8]) -> Option<Str> {
    let table: [(&[u8], &[u8]); 14] = [
        (b"+", b" {}+{} "),
        (b"-", b" {}-{} "),
        (b"=", b" {}={} "),
        (b"<", b" {}<{} "),
        (b">", b" {}>{} "),
        (b"<<", b" {}\\ll{} "),
        (b">>", b" {}\\gg{} "),
        (b"\\pm", b" {}\\pm{} "),
        (b"\\approx", b" {}\\approx{} "),
        (b"$\\approx$", b" {}\\approx{} "),
        (b"v", b" \\downarrow{} "),
        (b"(v)", b" \\downarrow{} "),
        (b"^", b" \\uparrow{} "),
        (b"(^)", b" \\uparrow{} "),
    ];
    for (key, value) in table {
        if eql(a, key) {
            return Some(value.to_vec());
        }
    }
    None
}
