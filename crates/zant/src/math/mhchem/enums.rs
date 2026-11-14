//! State-machine table types: the action enum, its options, `Task` and
//! `Transition`, plus the const constructors used to declare the tables.
//!
//! Port of `src/math/mhchem.zig` (Act / Option / A / Task / Transition).

use super::patterns::Pat;
use super::types::ItemTag;

/// `mhchemParser.actions` + per-machine action names (the Zig enum member is
/// noted in the trailing comment).
#[derive(Clone, Copy)]
pub enum Act {
    SetA,                // "a="
    SetB,                // "b="
    SetP,                // "p="
    SetO,                // "o="
    SetQ,                // "q="
    SetD,                // "d="
    SetRm,               // "rm="
    SetText,             // "text="
    Insert,              // insert
    InsertP1,            // "insert+p1"
    Copy,                // copy
    Rm,                  // rm
    Text,                // text
    BracedText,          // "{text}"
    TexMath,             // "tex-math"
    TexMathTight,        // "tex-math tight"
    Bond,                // bond
    Color0Output,        // "color0-output"
    Ce,                  // ce
    Half,                // "1/2"
    NineNine,            // "9,9"
    Output,              // output
    StateOfAggregation,  // "state of aggregation"
    ColorOutput,         // "color-output"
    Comma,               // comma
    Operator,            // operator
    Space,               // space
    OAfterD,             // "o after d"
    DEqualsKv,           // "d= kv"
    ChargeOrBond,        // "charge or bond"
    MinusAfterOD,        // "- after o/d"
    AToO,                // "a to o"
    SbTrue,              // "sb=true"
    SbFalse,             // "sb=false"
    BeginsWithBondTrue,  // "beginsWithBond=true"
    BeginsWithBondFalse, // "beginsWithBond=false"
    ParenthesisLevelInc, // "parenthesisLevel++"
    ParenthesisLevelDec, // "parenthesisLevel--"
    OxidationOutput,     // "oxidation-output"
    FracOutput,          // "frac-output"
    OversetOutput,       // "overset-output"
    UndersetOutput,      // "underset-output"
    UnderbraceOutput,    // "underbrace-output"
    SetR,                // "r="
    SetRdt,              // "rdt="
    SetRd,               // "rd="
    SetRqt,              // "rqt="
    SetRq,               // "rq="
    RomanNumeral,        // "roman-numeral"
    TightOperator,       // "tight operator"
    Enumber,             // enumber
    NumberPow,           // "number^"
    Cdot,                // cdot
    CaretNegativeOne,    // "^(-1)"
    Output0,             // "output-0"
    OutputO,             // "output-o"
}

/// `option` of an action (`{ type_: ..., option: ... }`).
#[derive(Clone, Copy)]
pub enum ActionOption {
    None,
    Str(&'static str),
    Int(u8),
    Bool(bool),
    Item(ItemTag),
}

/// `{ type_: ..., option: ... }`
#[derive(Clone, Copy)]
pub struct A {
    pub type_: Act,
    pub option: ActionOption,
}

#[derive(Clone, Copy)]
pub struct Task {
    pub action_: &'static [A],
    pub next_state: Option<&'static str>,
    pub revisit: bool,
    pub to_continue: bool,
}

#[derive(Clone, Copy)]
pub struct Transition {
    pub patterns: &'static [Pat],
    pub states: &'static str,
    pub task: Task,
}

pub const fn act(t: Act) -> A {
    A {
        type_: t,
        option: ActionOption::None,
    }
}

/// Action with a string `option` (bond/operator names declared inline).
pub const fn with_str(t: Act, s: &'static str) -> A {
    A {
        type_: t,
        option: ActionOption::Str(s),
    }
}

/// Action with an integer `option` (`output`'s `entityFollows`).
pub const fn with_int(t: Act, n: u8) -> A {
    A {
        type_: t,
        option: ActionOption::Int(n),
    }
}

/// Action with a boolean `option` (`- after o/d`'s `isAfterD`).
pub const fn with_bool(t: Act, b: bool) -> A {
    A {
        type_: t,
        option: ActionOption::Bool(b),
    }
}

/// Action with an output-item `option` (`insert` / `insert+p1`).
pub const fn with_item(t: Act, tag: ItemTag) -> A {
    A {
        type_: t,
        option: ActionOption::Item(tag),
    }
}

pub const fn task(
    action_: &'static [A],
    next_state: Option<&'static str>,
    revisit: bool,
    to_continue: bool,
) -> Task {
    Task {
        action_,
        next_state,
        revisit,
        to_continue,
    }
}

/// One `'pattern|pattern': { 'state|state': task }` entry of
/// `createTransitions`.
pub const fn tr(patterns: &'static [Pat], states: &'static str, task: Task) -> Transition {
    Transition {
        patterns,
        states,
        task,
    }
}
