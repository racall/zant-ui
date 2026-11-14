//! Shared parser types: upstream `ParserOutput` items, `Match`/`Mv`/`Ret`, and
//! the parser buffer. Port of `src/math/mhchem.zig` (types section) — WP-3.7.

use super::MhchemError;

/// Owned byte string, standing in for Zig's arena-backed `[]const u8`.
pub type Str = Vec<u8>;
pub type Items = Vec<Item>;

/// `Chemfive`: one parsed chemical entity (upstream object literal).
pub struct Chemfive {
    pub a: Items,
    pub b: Items,
    pub p: Items,
    pub o: Items,
    pub q: Items,
    pub d: Items,
    pub d_type: Option<Str>,
}

/// `ParserOutput` entry: a bare string or a `{ type_: ... }` object.
///
/// A few variants (`Space`, `Up`, `V`) are never produced by the current tables
/// but exist upstream (`ParserOutput`), so they stay for parity.
#[allow(dead_code)]
pub enum Item {
    /// A bare string pushed into the output array.
    Str(Str),
    Chemfive(Box<Chemfive>),
    Rm(Str),
    Text(Str),
    RomanNumeral(Str),
    StateOfAggregation(Items),
    StateOfAggregationSubscript(Items),
    Bond(Str),
    Frac([Str; 2]),
    PuFrac([Items; 2]),
    TexMath(Str),
    FracCe([Items; 2]),
    Overset([Items; 2]),
    Underset([Items; 2]),
    Underbrace([Items; 2]),
    Color {
        color1: Str,
        color2: Items,
    },
    Color0(Str),
    Arrow {
        r: Str,
        rd: Items,
        rq: Items,
    },
    Operator(Str),
    FirstLevelEscape(Str),
    CommaEnumerationL(Str),
    CommaEnumerationM(Str),
    CommaEnumerationS(Str),
    Space,
    EntitySkip,
    PuSpace1,
    PuSpace2,
    ThousandSeparator,
    CommaDecimal,
    Hyphen,
    AdditionCompound,
    ElectronDot,
    KvX,
    Prime,
    Cdot,
    TightCdot,
    Times,
    Circa,
    Up,
    V,
    Ellipsis,
    Slash,
    SpacedSlash,
}

/// The tag argument of `insert` / `insert+p1` actions.
#[derive(Clone, Copy)]
pub enum ItemTag {
    Hyphen,
    Ellipsis,
    AdditionCompound,
    CommaDecimal,
    ElectronDot,
    KvX,
    Prime,
    Circa,
    FirstLevelEscape,
    CommaEnumerationS,
}

/// `{ type_: option }` (action 'insert').
pub fn void_item(tag: ItemTag) -> Result<Item, MhchemError> {
    Ok(match tag {
        ItemTag::Hyphen => Item::Hyphen,
        ItemTag::Ellipsis => Item::Ellipsis,
        ItemTag::AdditionCompound => Item::AdditionCompound,
        ItemTag::CommaDecimal => Item::CommaDecimal,
        ItemTag::ElectronDot => Item::ElectronDot,
        ItemTag::KvX => Item::KvX,
        ItemTag::Prime => Item::Prime,
        ItemTag::Circa => Item::Circa,
        ItemTag::FirstLevelEscape | ItemTag::CommaEnumerationS => return Err(MhchemError),
    })
}

/// `{ type_: option, p1: m }` (action 'insert+p1').
pub fn p1_item(tag: ItemTag, p1: Str) -> Result<Item, MhchemError> {
    Ok(match tag {
        ItemTag::FirstLevelEscape => Item::FirstLevelEscape(p1),
        ItemTag::CommaEnumerationS => Item::CommaEnumerationS(p1),
        _ => return Err(MhchemError),
    })
}

/// Return value of an action: `undefined`, one value, or an array.
pub enum Ret {
    None,
    One(Item),
    Many(Items),
}

/// `matches.match_`: a string or (for two-group patterns) an array.
#[derive(Clone)]
pub enum Mv {
    S(Str),
    Arr(Vec<Str>),
}

impl Mv {
    pub fn as_str(&self) -> Option<&[u8]> {
        match self {
            Mv::S(s) => Some(s),
            Mv::Arr(_) => None,
        }
    }
}

pub struct Match {
    pub m: Mv,
    pub rem: Str,
}

/// Parser buffer. a == amount, o == element, b == left-side superscript,
/// p == left-side subscript, q == right-side subscript, d == right-side
/// superscript, r == arrow, rdt/rd == arrow script above (type, content),
/// rqt/rq == arrow script below. `None` == JS `undefined`. `text` is Zig's
/// `text_`.
#[derive(Clone, Default)]
pub struct Buffer {
    pub a: Option<Str>,
    pub b: Option<Str>,
    pub p: Option<Str>,
    pub o: Option<Str>,
    pub q: Option<Str>,
    pub d: Option<Str>,
    pub r: Option<Str>,
    pub rdt: Option<Str>,
    pub rd: Option<Str>,
    pub rqt: Option<Str>,
    pub rq: Option<Str>,
    pub text: Option<Str>,
    pub rm: Option<Str>,
    pub d_type: Option<Str>,
    pub sb: bool,
    pub parenthesis_level: i32,
    pub begins_with_bond: bool,
}

/// `mhchemParser.stateMachines` keys.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sm {
    Ce,
    A,
    O,
    Text,
    Pq,
    Bd,
    Oxidation,
    TexMath,
    TexMathTight,
    NineNine,
    Pu,
    Pu2,
    Pu99,
}
