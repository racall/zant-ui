//! `mhchemParser.stateMachines`: the remaining transition tables, one entry per
//! line so they can be diffed against upstream (the `ce` table lives in
//! `tables_ce.rs`).

use super::enums::{Act, Transition, act, task, tr, with_item, with_str};
use super::patterns::Pat;
use super::tables_ce::CE_TRANSITIONS;
use super::types::{ItemTag, Sm};

#[rustfmt::skip]
static A_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[], None, false, false)),
    tr(&[Pat::HalfEnd], "0", task(&[act(Act::Half)], None, false, false)),
    tr(&[Pat::Else], "0", task(&[], Some("1"), true, false)),
    tr(&[Pat::DollarParenDollar], "*", task(&[act(Act::TexMathTight)], Some("1"), false, false)),
    tr(&[Pat::Comma], "*", task(&[with_item(Act::Insert, ItemTag::CommaDecimal)], None, false, false)),
    tr(&[Pat::Else2], "*", task(&[act(Act::Copy)], None, false, false)),
];

#[rustfmt::skip]
static O_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[], None, false, false)),
    tr(&[Pat::HalfEnd], "0", task(&[act(Act::Half)], None, false, false)),
    tr(&[Pat::Else], "0", task(&[], Some("1"), true, false)),
    tr(&[Pat::Letters], "*", task(&[act(Act::Rm)], None, false, false)),
    tr(&[Pat::BackslashCa], "*", task(&[with_item(Act::Insert, ItemTag::Circa)], None, false, false)),
    tr(&[Pat::MacroBracedBraced, Pat::MacroBraced, Pat::Macro], "*", task(&[act(Act::Copy)], None, false, false)),
    tr(&[Pat::DollarBracedDollar, Pat::DollarParenDollar], "*", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::BracedParen], "*", task(&[act(Act::BracedText)], None, false, false)),
    tr(&[Pat::Else2], "*", task(&[act(Act::Copy)], None, false, false)),
];

#[rustfmt::skip]
static TEXT_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[act(Act::Output)], None, false, false)),
    tr(&[Pat::Braced], "*", task(&[act(Act::SetText)], None, false, false)),
    tr(&[Pat::DollarBracedDollar, Pat::DollarParenDollar], "*", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::Greek], "*", task(&[act(Act::Output), act(Act::Rm)], None, false, false)),
    tr(&[Pat::BackslashComma, Pat::MacroBracedBraced, Pat::MacroBraced, Pat::Macro], "*", task(&[act(Act::Output), act(Act::Copy)], None, false, false)),
    tr(&[Pat::Else], "*", task(&[act(Act::SetText)], None, false, false)),
];

#[rustfmt::skip]
static PQ_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[], None, false, false)),
    tr(&[Pat::StateOfAggregationEnd], "*", task(&[act(Act::StateOfAggregation)], None, false, false)),
    tr(&[Pat::IEnd], "0", task(&[], Some("!f"), true, false)),
    tr(&[Pat::KvLettersComma], "0", task(&[act(Act::Rm)], Some("0"), false, false)),
    tr(&[Pat::FormulaEnd], "0", task(&[], Some("f"), true, false)),
    tr(&[Pat::HalfEnd], "0", task(&[act(Act::Half)], None, false, false)),
    tr(&[Pat::Else], "0", task(&[], Some("!f"), true, false)),
    tr(&[Pat::DollarBracedDollar, Pat::DollarParenDollar], "*", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::BracedParen], "*", task(&[act(Act::Text)], None, false, false)),
    tr(&[Pat::AZ], "f", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::Letters], "*", task(&[act(Act::Rm)], None, false, false)),
    tr(&[Pat::N9DotComma9], "*", task(&[act(Act::NineNine)], None, false, false)),
    tr(&[Pat::Comma], "*", task(&[with_item(Act::InsertP1, ItemTag::CommaEnumerationS)], None, false, false)),
    tr(&[Pat::ColorBracedBraced1, Pat::ColorParenBraced2], "*", task(&[act(Act::ColorOutput)], None, false, false)),
    tr(&[Pat::ColorBraced0], "*", task(&[act(Act::Color0Output)], None, false, false)),
    tr(&[Pat::CeBraced], "*", task(&[act(Act::Ce)], None, false, false)),
    tr(&[Pat::BackslashComma, Pat::MacroBracedBraced, Pat::MacroBraced, Pat::Macro], "*", task(&[act(Act::Copy)], None, false, false)),
    tr(&[Pat::Else2], "*", task(&[act(Act::Copy)], None, false, false)),
];

#[rustfmt::skip]
static BD_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[], None, false, false)),
    tr(&[Pat::XEnd], "0", task(&[], Some("!f"), true, false)),
    tr(&[Pat::FormulaEnd], "0", task(&[], Some("f"), true, false)),
    tr(&[Pat::Else], "0", task(&[], Some("!f"), true, false)),
    tr(&[Pat::N9DotComma9NoMissing0], "*", task(&[act(Act::NineNine)], None, false, false)),
    tr(&[Pat::Dot], "*", task(&[with_item(Act::Insert, ItemTag::ElectronDot)], None, false, false)),
    tr(&[Pat::AZ], "f", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::X], "*", task(&[with_item(Act::Insert, ItemTag::KvX)], None, false, false)),
    tr(&[Pat::Letters], "*", task(&[act(Act::Rm)], None, false, false)),
    tr(&[Pat::Prime], "*", task(&[with_item(Act::Insert, ItemTag::Prime)], None, false, false)),
    tr(&[Pat::DollarBracedDollar, Pat::DollarParenDollar], "*", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::BracedParen], "*", task(&[act(Act::Text)], None, false, false)),
    tr(&[Pat::ColorBracedBraced1, Pat::ColorParenBraced2], "*", task(&[act(Act::ColorOutput)], None, false, false)),
    tr(&[Pat::ColorBraced0], "*", task(&[act(Act::Color0Output)], None, false, false)),
    tr(&[Pat::CeBraced], "*", task(&[act(Act::Ce)], None, false, false)),
    tr(&[Pat::BackslashComma, Pat::MacroBracedBraced, Pat::MacroBraced, Pat::Macro], "*", task(&[act(Act::Copy)], None, false, false)),
    tr(&[Pat::Else2], "*", task(&[act(Act::Copy)], None, false, false)),
];

#[rustfmt::skip]
static OXIDATION_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[], None, false, false)),
    tr(&[Pat::RomanNumeral], "*", task(&[act(Act::RomanNumeral)], None, false, false)),
    tr(&[Pat::DollarBracedDollar, Pat::DollarParenDollar], "*", task(&[act(Act::TexMath)], None, false, false)),
    tr(&[Pat::Else], "*", task(&[act(Act::Copy)], None, false, false)),
];

#[rustfmt::skip]
static TEX_MATH_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[act(Act::Output)], None, false, false)),
    tr(&[Pat::CeBraced], "*", task(&[act(Act::Output), act(Act::Ce)], None, false, false)),
    tr(&[Pat::Braced, Pat::BackslashComma, Pat::MacroBracedBraced, Pat::MacroBraced, Pat::Macro], "*", task(&[act(Act::SetO)], None, false, false)),
    tr(&[Pat::Else], "*", task(&[act(Act::SetO)], None, false, false)),
];

#[rustfmt::skip]
static TEX_MATH_TIGHT_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[act(Act::Output)], None, false, false)),
    tr(&[Pat::CeBraced], "*", task(&[act(Act::Output), act(Act::Ce)], None, false, false)),
    tr(&[Pat::Braced, Pat::BackslashComma, Pat::MacroBracedBraced, Pat::MacroBraced, Pat::Macro], "*", task(&[act(Act::SetO)], None, false, false)),
    tr(&[Pat::Minus, Pat::Plus], "*", task(&[act(Act::TightOperator)], None, false, false)),
    tr(&[Pat::Else], "*", task(&[act(Act::SetO)], None, false, false)),
];

#[rustfmt::skip]
static NINE_NINE_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[], None, false, false)),
    tr(&[Pat::Comma], "*", task(&[act(Act::Comma)], None, false, false)),
    tr(&[Pat::Else], "*", task(&[act(Act::Copy)], None, false, false)),
];

#[rustfmt::skip]
static PU_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[act(Act::Output)], None, false, false)),
    tr(&[Pat::SpaceEnd], "*", task(&[act(Act::Output), act(Act::Space)], None, false, false)),
    tr(&[Pat::OpenGroup, Pat::CloseGroup], "0|a", task(&[act(Act::Copy)], None, false, false)),
    tr(&[Pat::NumberPow], "0", task(&[act(Act::NumberPow)], Some("a"), false, false)),
    tr(&[Pat::Enumber], "0", task(&[act(Act::Enumber)], Some("a"), false, false)),
    tr(&[Pat::Space], "0|a", task(&[], None, false, false)),
    tr(&[Pat::PmOperator], "0|a", task(&[with_str(Act::Operator, "\\pm")], Some("0"), false, false)),
    tr(&[Pat::Operator], "0|a", task(&[act(Act::Copy)], Some("0"), false, false)),
    tr(&[Pat::DoubleSlash], "d", task(&[act(Act::SetO)], Some("/"), false, false)),
    tr(&[Pat::Slash], "d", task(&[act(Act::SetO)], Some("/"), false, false)),
    tr(&[Pat::Braced, Pat::Else], "0|d", task(&[act(Act::SetD)], Some("d"), false, false)),
    tr(&[Pat::Braced, Pat::Else], "a", task(&[act(Act::Space), act(Act::SetD)], Some("d"), false, false)),
    tr(&[Pat::Braced, Pat::Else], "/|q", task(&[act(Act::SetQ)], Some("q"), false, false)),
];

#[rustfmt::skip]
static PU_2_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "*", task(&[act(Act::Output)], None, false, false)),
    tr(&[Pat::Star], "*", task(&[act(Act::Output), act(Act::Cdot)], Some("0"), false, false)),
    tr(&[Pat::Macro], "*", task(&[act(Act::SetRm)], None, false, false)),
    tr(&[Pat::Space], "*", task(&[act(Act::Output), act(Act::Space)], Some("0"), false, false)),
    tr(&[Pat::CaretBraced, Pat::CaretNegativeOne], "1", task(&[act(Act::CaretNegativeOne)], None, false, false)),
    tr(&[Pat::N9DotComma9], "0", task(&[act(Act::SetRm)], Some("0"), false, false)),
    tr(&[Pat::N9DotComma9], "1", task(&[act(Act::CaretNegativeOne)], Some("0"), false, false)),
    tr(&[Pat::Braced, Pat::Else], "*", task(&[act(Act::SetRm)], Some("1"), false, false)),
];

#[rustfmt::skip]
static PU_NINE_NINE_TRANSITIONS: &[Transition] = &[
    tr(&[Pat::Empty], "0", task(&[act(Act::Output0)], None, false, false)),
    tr(&[Pat::Empty], "o", task(&[act(Act::OutputO)], None, false, false)),
    tr(&[Pat::Comma], "0", task(&[act(Act::Output0), act(Act::Comma)], Some("o"), false, false)),
    tr(&[Pat::Dot], "0", task(&[act(Act::Output0), act(Act::Copy)], Some("o"), false, false)),
    tr(&[Pat::Else], "*", task(&[act(Act::SetText)], None, false, false)),
];

/// `mhchemParser.stateMachines[sm].transitions`
pub fn machine_transitions(sm: Sm) -> &'static [Transition] {
    match sm {
        Sm::Ce => CE_TRANSITIONS,
        Sm::A => A_TRANSITIONS,
        Sm::O => O_TRANSITIONS,
        Sm::Text => TEXT_TRANSITIONS,
        Sm::Pq => PQ_TRANSITIONS,
        Sm::Bd => BD_TRANSITIONS,
        Sm::Oxidation => OXIDATION_TRANSITIONS,
        Sm::TexMath => TEX_MATH_TRANSITIONS,
        Sm::TexMathTight => TEX_MATH_TIGHT_TRANSITIONS,
        Sm::NineNine => NINE_NINE_TRANSITIONS,
        Sm::Pu => PU_TRANSITIONS,
        Sm::Pu2 => PU_2_TRANSITIONS,
        Sm::Pu99 => PU_NINE_NINE_TRANSITIONS,
    }
}

/// `machine.transitions[state] || machine.transitions['*']` helper.
pub fn state_in(states: &str, state: &[u8]) -> bool {
    states.split('|').any(|s| s.as_bytes() == state)
}

pub fn state_exists(transitions: &[Transition], state: &[u8]) -> bool {
    transitions.iter().any(|t| state_in(t.states, state))
}
