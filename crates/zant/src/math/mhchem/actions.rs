//! Actions: per-machine `actions` first, then `mhchemParser.actions`
//! (`run_action`).

use super::chars::{decode, is_all_digits, is_js_space, truthy};
use super::enums::{A, Act, ActionOption};
use super::outputs::{
    after_od, ce_output, enumber, half, output9, pu_output, pu2_output, push_sign,
};
use super::types::{Buffer, Item, ItemTag, Items, Mv, Ret, Sm, p1_item, void_item};
use super::{MhchemError, app, braced, cat, concat_array, go, mstr};

/// `{ type_: option, p1: m }` item tag argument of `insert` / `insert+p1`.
fn option_item(action: A) -> Result<ItemTag, MhchemError> {
    match action.option {
        ActionOption::Item(tag) => Ok(tag),
        _ => Err(MhchemError),
    }
}

/// `action.option.str` (bond/operator names declared in the tables).
fn option_str(action: A) -> Option<&'static [u8]> {
    match action.option {
        ActionOption::Str(s) => Some(s.as_bytes()),
        _ => None,
    }
}

/// `action.option.int` (`output`'s `entityFollows`).
fn option_int(action: A) -> Option<u8> {
    match action.option {
        ActionOption::Int(n) => Some(n),
        _ => None,
    }
}

/// `action.option.bool` (`- after o/d`'s `isAfterD`).
fn option_bool(action: A) -> bool {
    match action.option {
        ActionOption::Bool(b) => b,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Actions: per-machine `actions` first, then `mhchemParser.actions`
// ---------------------------------------------------------------------------

pub fn run_action(sm: Sm, buffer: &mut Buffer, m: Mv, action: A) -> Result<Ret, MhchemError> {
    let ret: Option<Ret> = match action.type_ {
        // --- mhchemParser.actions (generic) ---
        Act::SetA => {
            buffer.a = Some(app(buffer.a.as_deref(), &m));
            None
        }
        Act::SetB => {
            buffer.b = Some(app(buffer.b.as_deref(), &m));
            None
        }
        Act::SetP => {
            buffer.p = Some(app(buffer.p.as_deref(), &m));
            None
        }
        Act::SetO => {
            buffer.o = Some(app(buffer.o.as_deref(), &m));
            None
        }
        Act::SetQ => {
            buffer.q = Some(app(buffer.q.as_deref(), &m));
            None
        }
        Act::SetD => {
            buffer.d = Some(app(buffer.d.as_deref(), &m));
            None
        }
        Act::SetRm => {
            buffer.rm = Some(app(buffer.rm.as_deref(), &m));
            None
        }
        Act::SetText => {
            buffer.text = Some(app(buffer.text.as_deref(), &m));
            None
        }
        Act::Insert => Some(Ret::One(void_item(option_item(action)?)?)),
        Act::InsertP1 => Some(Ret::One(p1_item(option_item(action)?, mstr(&m))?)),
        Act::Copy => Some(match m {
            Mv::S(s) => Ret::One(Item::Str(s)),
            Mv::Arr(arr) => Ret::Many(arr.into_iter().map(Item::Str).collect()),
        }),
        Act::Rm => Some(Ret::One(Item::Rm(mstr(&m)))),
        Act::Text => Some(Ret::Many(go(Some(mstr(&m)), Sm::Text)?)),
        Act::BracedText => Some(braced(go(Some(mstr(&m)), Sm::Text)?)),
        Act::TexMath => Some(Ret::Many(go(Some(mstr(&m)), Sm::TexMath)?)),
        Act::TexMathTight => Some(Ret::Many(go(Some(mstr(&m)), Sm::TexMathTight)?)),
        Act::Bond => Some(Ret::One(Item::Bond(match option_str(action) {
            Some(s) => s.to_vec(),
            None => mstr(&m),
        }))),
        Act::Color0Output => Some(Ret::One(Item::Color0(super::chars::first_element(&m)))),
        Act::Ce => Some(Ret::Many(go(Some(mstr(&m)), Sm::Ce)?)),
        Act::Half => Some(half(&mstr(&m))?),
        Act::NineNine => Some(Ret::Many(go(Some(mstr(&m)), Sm::NineNine)?)),

        // --- shared names, resolved per machine ---
        Act::Output => Some(match sm {
            Sm::Ce => ce_output(buffer, option_int(action).unwrap_or(0))?,
            Sm::Text => {
                if !truthy(&buffer.text) {
                    Ret::None
                } else {
                    let ret: Item = Item::Text(buffer.text.take().unwrap_or_default());
                    *buffer = Buffer::default();
                    Ret::One(ret)
                }
            }
            Sm::TexMath | Sm::TexMathTight => {
                if !truthy(&buffer.o) {
                    Ret::None
                } else {
                    let ret: Item = Item::TexMath(buffer.o.take().unwrap_or_default());
                    *buffer = Buffer::default();
                    Ret::One(ret)
                }
            }
            Sm::Pu => pu_output(buffer)?,
            Sm::Pu2 => pu2_output(buffer)?,
            _ => return Err(MhchemError), // "MhchemBugA"
        }),
        Act::StateOfAggregation => Some(match sm {
            Sm::Ce => Ret::One(Item::StateOfAggregation(go(Some(mstr(&m)), Sm::O)?)),
            Sm::Pq => Ret::One(Item::StateOfAggregationSubscript(go(
                Some(mstr(&m)),
                Sm::O,
            )?)),
            _ => return Err(MhchemError),
        }),
        Act::ColorOutput => {
            let arr = match &m {
                Mv::Arr(arr) => arr,
                Mv::S(_) => return Err(MhchemError),
            };
            let machine: Sm = match sm {
                Sm::Ce => Sm::Ce,
                Sm::Pq => Sm::Pq,
                Sm::Bd => Sm::Bd,
                _ => return Err(MhchemError),
            };
            if arr.len() < 2 {
                return Err(MhchemError);
            }
            Some(Ret::One(Item::Color {
                color1: arr[0].clone(),
                color2: go(Some(arr[1].clone()), machine)?,
            }))
        }
        Act::Comma => match sm {
            Sm::Ce => {
                let s = mstr(&m);
                // m.replace(/\s*$/, '')
                let mut end = s.len();
                while end > 0 {
                    let mut start = end - 1;
                    while start > 0 && (s[start] & 0xC0) == 0x80 {
                        start -= 1;
                    }
                    if !is_js_space(decode(&s, start).c) {
                        break;
                    }
                    end = start;
                }
                let a = &s[..end];
                let with_space = a.len() != s.len();
                Some(Ret::One(if with_space && buffer.parenthesis_level == 0 {
                    Item::CommaEnumerationL(a.to_vec())
                } else {
                    Item::CommaEnumerationM(a.to_vec())
                }))
            }
            Sm::NineNine | Sm::Pu99 => Some(Ret::One(Item::CommaDecimal)),
            _ => return Err(MhchemError),
        },
        Act::Operator => Some(Ret::One(Item::Operator(match option_str(action) {
            Some(s) => s.to_vec(),
            None => mstr(&m),
        }))),
        Act::Space => Some(match sm {
            Sm::Pu => Ret::One(Item::PuSpace1),
            Sm::Pu2 => Ret::One(Item::PuSpace2),
            _ => return Err(MhchemError),
        }),

        // --- stateMachines.ce.actions ---
        Act::OAfterD => {
            let ret = if is_all_digits(buffer.d.as_deref().unwrap_or(b"")) {
                let tmp = buffer.d.take();
                let ret = ce_output(buffer, 0)?;
                buffer.b = tmp;
                ret
            } else {
                ce_output(buffer, 0)?
            };
            buffer.o = Some(app(buffer.o.as_deref(), &m));
            Some(ret)
        }
        Act::DEqualsKv => {
            buffer.d = Some(mstr(&m));
            buffer.d_type = Some(b"kv".to_vec());
            None
        }
        Act::ChargeOrBond => {
            if buffer.begins_with_bond {
                let mut ret: Items = Vec::new();
                concat_array(&mut ret, ce_output(buffer, 0)?);
                ret.push(Item::Bond(b"-".to_vec()));
                Some(Ret::Many(ret))
            } else {
                buffer.d = Some(mstr(&m));
                None
            }
        }
        Act::MinusAfterOD => Some(after_od(buffer, &mstr(&m), option_bool(action))?),
        Act::AToO => {
            buffer.o = buffer.a.take();
            None
        }
        Act::SbTrue => {
            buffer.sb = true;
            None
        }
        Act::SbFalse => {
            buffer.sb = false;
            None
        }
        Act::BeginsWithBondTrue => {
            buffer.begins_with_bond = true;
            None
        }
        Act::BeginsWithBondFalse => {
            buffer.begins_with_bond = false;
            None
        }
        Act::ParenthesisLevelInc => {
            buffer.parenthesis_level += 1;
            None
        }
        Act::ParenthesisLevelDec => {
            buffer.parenthesis_level -= 1;
            None
        }
        Act::OxidationOutput => Some(braced(go(Some(mstr(&m)), Sm::Oxidation)?)),
        Act::FracOutput | Act::OversetOutput | Act::UndersetOutput | Act::UnderbraceOutput => {
            let arr = match &m {
                Mv::Arr(arr) => arr,
                Mv::S(_) => return Err(MhchemError),
            };
            if arr.len() < 2 {
                return Err(MhchemError);
            }
            let pair: [Items; 2] = [
                go(Some(arr[0].clone()), Sm::Ce)?,
                go(Some(arr[1].clone()), Sm::Ce)?,
            ];
            Some(Ret::One(match action.type_ {
                Act::FracOutput => Item::FracCe(pair),
                Act::OversetOutput => Item::Overset(pair),
                Act::UndersetOutput => Item::Underset(pair),
                _ => Item::Underbrace(pair),
            }))
        }
        Act::SetR => {
            buffer.r = Some(mstr(&m));
            None
        }
        Act::SetRdt => {
            buffer.rdt = Some(mstr(&m));
            None
        }
        Act::SetRd => {
            buffer.rd = Some(mstr(&m));
            None
        }
        Act::SetRqt => {
            buffer.rqt = Some(mstr(&m));
            None
        }
        Act::SetRq => {
            buffer.rq = Some(mstr(&m));
            None
        }

        // --- stateMachines.oxidation.actions ---
        Act::RomanNumeral => Some(Ret::One(Item::RomanNumeral(mstr(&m)))),
        // --- stateMachines['tex-math tight'].actions ---
        Act::TightOperator => {
            let matched = mstr(&m);
            buffer.o = Some(cat(&[
                buffer.o.as_deref().unwrap_or(b""),
                b"{",
                &matched,
                b"}",
            ]));
            None
        }
        // --- stateMachines.pu.actions ---
        Act::Enumber => Some(enumber(m)?),
        Act::NumberPow => {
            let arr = match &m {
                Mv::Arr(arr) => arr,
                Mv::S(_) => return Err(MhchemError),
            };
            if arr.len() < 3 {
                return Err(MhchemError);
            }
            let mut ret: Items = Vec::new();
            push_sign(&mut ret, &arr[0]);
            concat_array(&mut ret, Ret::Many(go(Some(arr[1].clone()), Sm::Pu99)?));
            ret.push(Item::Str(cat(&[b"^{", &arr[2], b"}"])));
            Some(Ret::Many(ret))
        }
        // --- stateMachines['pu-2'].actions ---
        Act::Cdot => Some(Ret::One(Item::TightCdot)),
        Act::CaretNegativeOne => {
            let matched = mstr(&m);
            buffer.rm = Some(cat(&[
                buffer.rm.as_deref().unwrap_or(b"undefined"),
                b"^{",
                &matched,
                b"}",
            ]));
            None
        }
        // --- stateMachines['pu-9,9'].actions ---
        Act::Output0 => Some(output9(buffer, false)),
        Act::OutputO => Some(output9(buffer, true)),
    };
    Ok(ret.unwrap_or(Ret::None))
}
