//! TeX math parser (KaTeX-compatible subset) producing a `Node` tree.
//!
//! Structural errors (unbalanced braces, missing arguments, unknown
//! environments) fail the whole parse — the caller then renders the source
//! in the error colour like KaTeX with `throwOnError: false`. Unknown control
//! sequences are kept as `.unknown` nodes and coloured individually.
//!
//! The tree owns its data (`Vec<Node>` / `String` / `Box<Node>`). The parser
//! works on bytes and keeps `i` on UTF-8 boundaries for slicing: a `\`
//! followed by a non-ASCII character advances one whole code point.

mod commands;
mod environments;
mod structures;
mod styles;
mod symbols;
mod tokens;

#[cfg(test)]
mod tests;

use crate::math::fonts;

pub use self::styles::{parse_color, parse_dimen};
pub use crate::math::symbols::Class;

/// Structural parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError;

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("math parse error")
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleSwitch {
    Display,
    Text,
    Script,
    ScriptScript,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LapSide {
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XArrowKind {
    Right,
    Left,
    LeftRight,
    DoubleRight,
    DoubleLeft,
    DoubleLeftRight,
    HookRight,
    HookLeft,
    MapsTo,
    LongEqual,
    HarpoonsRl,
    HarpoonsLr,
    ArrowsRl,
    TwoHeadRight,
    TwoHeadLeft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelKind {
    Cancel,
    Bcancel,
    Xcancel,
    Sout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverUnderKind {
    Overline,
    Underline,
    Overbrace,
    Underbrace,
    Overrightarrow,
    Overleftarrow,
    Overleftrightarrow,
    Underrightarrow,
    Underleftarrow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Atom {
    pub text: String,
    pub class: Class,
    pub variant: fonts::Variant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SupSub {
    pub base: Option<Box<Node>>,
    pub sup: Option<Box<Node>>,
    pub sub: Option<Box<Node>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frac {
    pub num: Vec<Node>,
    pub den: Vec<Node>,
    pub bar: bool,
    pub left: String,
    pub right: String,
    pub style: Option<StyleSwitch>,
    /// `\cfrac`: numerator stays display style.
    pub continued: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sqrt {
    pub body: Vec<Node>,
    pub index: Option<Vec<Node>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LeftRight {
    pub left: String,
    pub right: String,
    pub body: Vec<Node>,
    // `\middle|` appears in `body` as a `.sized_delim` with size 0.
}

#[derive(Clone, Debug, PartialEq)]
pub struct SizedDelim {
    pub delim: String,
    /// 1..4 for \big..\Bigg; 0 = `\middle` (sized to the enclosing \left).
    pub size: u8,
    pub class: Class,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccentNode {
    pub base: Vec<Node>,
    pub glyph: String,
    pub wide: bool,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverUnder {
    pub body: Vec<Node>,
    pub kind: OverUnderKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    pub text: String,
    /// true for ∑-like glyph operators (enlarged in display style).
    pub symbol: bool,
    pub integral: bool,
    pub limits: bool,
    /// `\limits` / `\nolimits` override.
    pub forced: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub text: String,
    pub variant: fonts::Variant,
    pub bold: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub rows: Vec<Vec<Vec<Node>>>,
    /// One entry per column: 'l', 'c', 'r'.
    pub cols: Vec<u8>,
    /// Vertical rule positions: column index before which a `|` is drawn.
    pub col_rules: Vec<usize>,
    /// Rows (index before which) a `\hline` is drawn.
    pub row_rules: Vec<usize>,
    pub left: String,
    pub right: String,
    pub display: bool,
    /// Column separation in em (KaTeX arraycolsep 5pt = 0.5em per side).
    pub col_sep: f32,
    /// aligned/align: alternate r/l columns with no separation inside pairs.
    pub aligned: bool,
    pub small: bool,
    /// Extra space after each row (em), from `\\[..]`.
    pub row_gaps: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Color {
    pub color: u32,
    pub body: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XArrow {
    pub kind: XArrowKind,
    pub above: Vec<Node>,
    pub below: Option<Vec<Node>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// A glyph run treated as one atom (letter, digit, symbol).
    Atom(Atom),
    /// `{...}`: an ord atom made of a math list.
    Group(Vec<Node>),
    SupSub(SupSub),
    Frac(Frac),
    Sqrt(Sqrt),
    LeftRight(LeftRight),
    /// `\big(` etc.: a fixed-size delimiter.
    SizedDelim(SizedDelim),
    Accent(AccentNode),
    /// `\overline`, `\underline`, `\overbrace`, `\underbrace`,
    /// `\overrightarrow`, ...
    OverUnder(OverUnder),
    /// `\sum`, `\int`, `\lim`, `\operatorname{..}`
    Op(Op),
    Text(Text),
    Space(f32),
    Array(Array),
    Color(Color),
    Style {
        style: StyleSwitch,
        body: Vec<Node>,
    },
    Phantom {
        body: Vec<Node>,
        horizontal: bool,
        vertical: bool,
    },
    Boxed(Vec<Node>),
    Cancel {
        body: Vec<Node>,
        kind: CancelKind,
    },
    /// `\overset{top}{base}` / `\underset` / `\stackrel`
    Stack {
        base: Box<Node>,
        over: Option<Box<Node>>,
        under: Option<Box<Node>>,
    },
    /// `\tag{..}`
    Tag(String),
    /// `\xrightarrow[below]{above}` and friends (KaTeX functions/arrow).
    XArrow(XArrow),
    /// `\large`, `\small`, ...: the rest of the group scaled.
    Sized {
        factor: f32,
        body: Vec<Node>,
    },
    /// `\colorbox{bg}{..}` / `\fcolorbox{frame}{bg}{..}`.
    ColorBox {
        background: u32,
        frame: Option<u32>,
        body: Vec<Node>,
    },
    /// `\smash{..}`: no height / depth.
    Smash(Vec<Node>),
    /// `\rule{w}{h}` in em.
    RuleBox {
        width: f32,
        height: f32,
    },
    /// `\mathrlap` / `\mathllap` / `\mathclap` (and `\rlap`, `\llap`):
    /// zero-width content overlapping its neighbours.
    Lap {
        side: LapSide,
        body: Vec<Node>,
    },
    /// `\mathchoice{D}{T}{S}{SS}`: one branch per math style.
    Choice([Vec<Node>; 4]),
    /// `\raisebox{dimen}{..}`: content moved up by `amount` em.
    Raise {
        amount: f32,
        body: Vec<Node>,
    },
    Unknown(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    None,
    Brace,
    Bracket,
    Right,
    EndEnv,
}

pub struct Parser<'a> {
    src: &'a str,
    i: usize,
}

pub fn parse(source: &str) -> Result<Vec<Node>, ParseError> {
    let mut p = Parser::init(source);
    p.parse_all()
}
