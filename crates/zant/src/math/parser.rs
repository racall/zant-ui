//! Port of `src/math/parser.zig` — WP-3.5.
//! TeX math parser (KaTeX-compatible subset) producing a `Node` tree.
//!
//! Structural errors (unbalanced braces, missing arguments, unknown
//! environments) fail the whole parse — the caller then renders the source
//! in the error colour like KaTeX with `throwOnError: false`. Unknown control
//! sequences are kept as `.unknown` nodes and coloured individually.
//!
//! Port notes: the Zig arena and slices become owned `Vec<Node>` / `String`,
//! and `*const Node` becomes `Box<Node>`. `zant::Str` (WP-1.2) is not in the
//! crate yet, so every text field is a `String`; layout only reads `&str`, so
//! switching to `zant::Str` later is a mechanical edit. `Error =
//! error{OutOfMemory, ParseError}` becomes `Result<_, ParseError>` (allocation
//! failure aborts in Rust). The parser works on bytes like the Zig original
//! and keeps `i` on UTF-8 boundaries for slicing; the one place where Zig
//! advances a single byte into a multi-byte character (`\` followed by a
//! non-ASCII char, which would produce invalid UTF-8) advances one code point
//! instead.

use crate::math::{fonts, mhchem, symbols};

pub use crate::math::symbols::Class;

/// Structural parse error (Zig's `error.ParseError`).
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

impl<'a> Parser<'a> {
    pub fn init(src: &'a str) -> Self {
        Self { src, i: 0 }
    }

    pub fn parse_all(&mut self) -> Result<Vec<Node>, ParseError> {
        let list = self.parse_list(Stop::None)?;
        self.skip_space();
        if self.i < self.src.len() {
            return Err(ParseError); // stray '}' etc.
        }
        Ok(list)
    }

    fn slice(&self, start: usize, end: usize) -> &'a str {
        let src = self.src;
        let end = end.min(src.len());
        &src[start..end]
    }

    fn peek_byte(&self) -> u8 {
        self.src.as_bytes().get(self.i).copied().unwrap_or(0)
    }

    fn next_char_len(&self) -> usize {
        // `self.i` is always on a UTF-8 boundary here.
        self.src
            .get(self.i..)
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8)
    }

    fn skip_space(&mut self) {
        while self.i < self.src.len() && self.src.as_bytes()[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn read_command_name(&mut self) -> &'a str {
        // at '\'
        self.i += 1;
        let start = self.i;
        if self.i >= self.src.len() {
            return "";
        }
        if !self.src.as_bytes()[self.i].is_ascii_alphabetic() {
            // Zig advances one byte; advance one code point so the slice stays
            // valid UTF-8.
            self.i += self.next_char_len();
            return self.slice(start, self.i);
        }
        while self.i < self.src.len() && self.src.as_bytes()[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
        // `\operatorname*`
        self.slice(start, self.i)
    }

    fn at_end(&mut self, stop: Stop) -> bool {
        self.skip_space();
        if self.i >= self.src.len() {
            return true;
        }
        let c = self.peek_byte();
        match stop {
            Stop::None | Stop::Brace => c == b'}',
            Stop::Bracket => c == b']' || c == b'}',
            Stop::Right => c == b'}' || self.looking_at_command("right"),
            Stop::EndEnv => {
                c == b'}'
                    || c == b'&'
                    || self.looking_at_command("end")
                    || self.looking_at("\\\\")
                    || self.looking_at_command("cr")
                    || self.looking_at_command("hline")
            }
        }
    }

    fn looking_at(&self, s: &str) -> bool {
        self.src.as_bytes()[self.i.min(self.src.len())..].starts_with(s.as_bytes())
    }

    fn looking_at_command(&self, name: &str) -> bool {
        let bytes = self.src.as_bytes();
        if self.i >= bytes.len() || bytes[self.i] != b'\\' {
            return false;
        }
        let rest = &bytes[self.i + 1..];
        if !rest.starts_with(name.as_bytes()) {
            return false;
        }
        rest.len() == name.len() || !rest[name.len()].is_ascii_alphabetic()
    }

    pub(crate) fn parse_list(&mut self, stop: Stop) -> Result<Vec<Node>, ParseError> {
        let mut out: Vec<Node> = Vec::new();
        while !self.at_end(stop) {
            if stop == Stop::None && self.peek_byte() == b'}' {
                break;
            }
            let c = self.peek_byte();
            if c == b'^' || c == b'_' {
                // script without a base
                self.attach_scripts(&mut out, None)?;
                continue;
            }
            if c == b'\'' {
                self.attach_scripts(&mut out, None)?;
                continue;
            }
            let node = match self.parse_atom(&mut out)? {
                Some(node) => node,
                None => continue,
            };
            out.push(node);
            self.skip_space();
            let n = self.peek_byte();
            if n == b'^' || n == b'_' || n == b'\'' {
                let base = out.pop().expect("just pushed");
                self.attach_scripts(&mut out, Some(base))?;
            }
        }
        Ok(out)
    }

    fn attach_scripts(
        &mut self,
        out: &mut Vec<Node>,
        base: Option<Node>,
    ) -> Result<(), ParseError> {
        let mut sup: Option<Box<Node>> = None;
        let mut sub: Option<Box<Node>> = None;
        let mut primes: usize = 0;
        loop {
            self.skip_space();
            let c = self.peek_byte();
            if c == b'\'' {
                primes += 1;
                self.i += 1;
                continue;
            }
            if c != b'^' && c != b'_' {
                break;
            }
            self.i += 1;
            let arg = self.parse_script_arg()?;
            if c == b'^' {
                if sup.is_some() {
                    return Err(ParseError); // double superscript
                }
                sup = Some(arg);
            } else {
                if sub.is_some() {
                    return Err(ParseError);
                }
                sub = Some(arg);
            }
        }
        if primes > 0 {
            let prime_text = "′".repeat(primes);
            let prime_node = Node::Atom(Atom {
                text: prime_text,
                class: Class::Ord,
                variant: fonts::Variant::Roman,
            });
            if let Some(existing) = sup {
                sup = Some(Box::new(Node::Group(vec![prime_node, *existing])));
            } else {
                sup = Some(Box::new(prime_node));
            }
        }
        // \limits / \nolimits apply to the op base (handled at parse of the op)
        out.push(Node::SupSub(SupSub {
            base: base.map(Box::new),
            sup,
            sub,
        }));
        Ok(())
    }

    fn parse_script_arg(&mut self) -> Result<Box<Node>, ParseError> {
        self.skip_space();
        if self.i >= self.src.len() {
            return Err(ParseError);
        }
        let c = self.peek_byte();
        if c == b'{' {
            let list = self.parse_group_body()?;
            return Ok(Box::new(Node::Group(list)));
        }
        if c == b'\\' {
            let mut dummy: Vec<Node> = Vec::new();
            let node = match self.parse_atom(&mut dummy)? {
                Some(node) => node,
                None => return Err(ParseError),
            };
            return Ok(Box::new(node));
        }
        // a single character
        let len = self.next_char_len();
        let text = self.slice(self.i, self.i + len);
        self.i += len;
        Ok(Box::new(Self::char_node(text)))
    }

    fn parse_group_body(&mut self) -> Result<Vec<Node>, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'{' {
            return Err(ParseError);
        }
        self.i += 1;
        let list = self.parse_list(Stop::Brace)?;
        self.skip_space();
        if self.peek_byte() != b'}' {
            return Err(ParseError);
        }
        self.i += 1;
        Ok(list)
    }

    /// A required argument: `{...}` or a single token.
    fn parse_arg(&mut self) -> Result<Vec<Node>, ParseError> {
        self.skip_space();
        if self.peek_byte() == b'{' {
            return self.parse_group_body();
        }
        if self.i >= self.src.len() {
            return Err(ParseError);
        }
        let mut dummy: Vec<Node> = Vec::new();
        if self.peek_byte() == b'\\' {
            let node = match self.parse_atom(&mut dummy)? {
                Some(node) => node,
                None => return Err(ParseError),
            };
            return Ok(vec![node]);
        }
        let len = self.next_char_len();
        let text = self.slice(self.i, self.i + len);
        self.i += len;
        Ok(vec![Self::char_node(text)])
    }

    /// Raw text of a `{...}` argument (balanced braces).
    fn raw_arg(&mut self) -> Result<&'a str, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'{' {
            if self.i >= self.src.len() {
                return Err(ParseError);
            }
            let len = self.next_char_len();
            let t = self.slice(self.i, self.i + len);
            self.i += len;
            return Ok(t);
        }
        let start = self.i + 1;
        let mut depth: usize = 0;
        while self.i < self.src.len() {
            let c = self.src.as_bytes()[self.i];
            if c == b'\\' {
                // Zig: `i += 1; continue;` = skip the escaped byte too.
                self.i += 2;
                continue;
            }
            if c == b'{' {
                depth += 1;
            }
            if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    let out = self.slice(start, self.i);
                    self.i += 1;
                    return Ok(out);
                }
            }
            self.i += 1;
        }
        Err(ParseError)
    }

    fn optional_arg(&mut self) -> Result<Option<Vec<Node>>, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'[' {
            return Ok(None);
        }
        self.i += 1;
        let list = self.parse_list(Stop::Bracket)?;
        if self.peek_byte() != b']' {
            return Err(ParseError);
        }
        self.i += 1;
        Ok(Some(list))
    }

    fn char_node(text: &str) -> Node {
        let cp = text.chars().next().unwrap_or('\u{FFFD}');
        if text.len() == 1 {
            let c = text.as_bytes()[0];
            if c == b'-' {
                return Node::Atom(Atom {
                    text: "−".to_string(),
                    class: Class::Bin,
                    variant: fonts::Variant::Normal,
                });
            }
            if c == b'*' {
                return Node::Atom(Atom {
                    text: "∗".to_string(),
                    class: Class::Bin,
                    variant: fonts::Variant::Normal,
                });
            }
            if c == b'~' {
                return Node::Space(0.25);
            }
            if c.is_ascii_digit() || c == b'.' {
                return Node::Atom(Atom {
                    text: text.to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Roman,
                });
            }
        }
        Node::Atom(Atom {
            text: text.to_string(),
            class: symbols::char_class(cp),
            variant: fonts::Variant::Normal,
        })
    }

    fn parse_atom(&mut self, out: &mut Vec<Node>) -> Result<Option<Node>, ParseError> {
        self.skip_space();
        let c = self.peek_byte();
        if c == b'{' {
            let list = self.parse_group_body()?;
            return Ok(Some(Node::Group(list)));
        }
        if c == b'&' {
            return Err(ParseError); // outside an environment
        }
        if c != b'\\' {
            let len = self.next_char_len();
            let text = self.slice(self.i, self.i + len);
            self.i += len;
            // merge runs of digits ("123" is one atom, "1.5" too)
            if c.is_ascii_digit() {
                let start = self.i - len;
                while self.i < self.src.len() {
                    let b = self.src.as_bytes()[self.i];
                    let digit_or_point = b.is_ascii_digit()
                        || (b == b'.'
                            && self.i + 1 < self.src.len()
                            && self.src.as_bytes()[self.i + 1].is_ascii_digit());
                    if !digit_or_point {
                        break;
                    }
                    self.i += 1;
                }
                return Ok(Some(Node::Atom(Atom {
                    text: self.slice(start, self.i).to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Roman,
                })));
            }
            return Ok(Some(Self::char_node(text)));
        }
        let name = self.read_command_name();
        self.command(name, out)
    }

    fn command(&mut self, name: &str, out: &mut Vec<Node>) -> Result<Option<Node>, ParseError> {
        // spacing
        if name == "," || name == "thinspace" {
            return Ok(Some(Node::Space(3.0 / 18.0)));
        }
        if name == ":" || name == ">" || name == "medspace" {
            return Ok(Some(Node::Space(4.0 / 18.0)));
        }
        if name == ";" || name == "thickspace" {
            return Ok(Some(Node::Space(5.0 / 18.0)));
        }
        if name == "!" || name == "negthinspace" {
            return Ok(Some(Node::Space(-3.0 / 18.0)));
        }
        if name == "negmedspace" {
            return Ok(Some(Node::Space(-4.0 / 18.0)));
        }
        if name == "negthickspace" {
            return Ok(Some(Node::Space(-5.0 / 18.0)));
        }
        if name == "quad" {
            return Ok(Some(Node::Space(1.0)));
        }
        if name == "qquad" {
            return Ok(Some(Node::Space(2.0)));
        }
        if name == " " || name == "space" || name == "nobreakspace" {
            return Ok(Some(Node::Space(0.25)));
        }
        if name == "enspace" {
            return Ok(Some(Node::Space(0.5)));
        }
        if name == "hspace"
            || name == "kern"
            || name == "mkern"
            || name == "mskip"
            || name == "hskip"
        {
            let raw = self.raw_arg_or_dimen()?;
            return Ok(Some(Node::Space(parse_dimen(raw))));
        }
        // line break outside environments: ignore (KaTeX: newline in display)
        if name == "\\"
            || name == "newline"
            || name == "nonumber"
            || name == "notag"
            || name == "displaylimits"
            || name == "allowbreak"
            || name == "relax"
        {
            return Ok(None);
        }
        if name == "limits" || name == "nolimits" {
            if let Some(last) = out.last_mut()
                && let Node::Op(op) = last
            {
                op.forced = Some(name == "limits");
            }
            return Ok(None);
        }

        if let Some(sym) = symbols::lookup(name) {
            // .normal italicises only lowercase Greek; other symbols are unchanged
            return Ok(Some(Node::Atom(Atom {
                text: sym.text.to_string(),
                class: sym.class,
                variant: fonts::Variant::Normal,
            })));
        }
        if let Some(op) = symbols::big_op(name) {
            return Ok(Some(Node::Op(Op {
                text: op.text.to_string(),
                symbol: true,
                integral: op.integral,
                limits: op.limits,
                forced: None,
            })));
        }
        if let Some(f) = symbols::function(name) {
            return Ok(Some(Node::Op(Op {
                text: f.text.to_string(),
                symbol: false,
                integral: false,
                limits: f.limits,
                forced: None,
            })));
        }
        if name == "operatorname" || name == "operatornamewithlimits" {
            let mut limits = name == "operatornamewithlimits";
            if self.peek_byte() == b'*' {
                self.i += 1;
                limits = true;
            }
            let raw = self.raw_arg()?;
            return Ok(Some(Node::Op(Op {
                text: strip_braces(raw),
                symbol: false,
                integral: false,
                limits,
                forced: None,
            })));
        }
        if name == "bmod" {
            return Ok(Some(Node::Atom(Atom {
                text: "mod".to_string(),
                class: Class::Bin,
                variant: fonts::Variant::Roman,
            })));
        }
        if name == "pmod" || name == "mod" || name == "pod" {
            let arg = self.parse_arg()?;
            let mut parts: Vec<Node> = Vec::new();
            parts.push(Node::Space(if name == "mod" { 0.75 } else { 1.0 }));
            if name != "mod" {
                parts.push(Node::Atom(Atom {
                    text: "(".to_string(),
                    class: Class::Open,
                    variant: fonts::Variant::Normal,
                }));
            }
            if name != "pod" {
                parts.push(Node::Atom(Atom {
                    text: "mod".to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Roman,
                }));
                parts.push(Node::Space(1.0 / 3.0));
            }
            parts.push(Node::Group(arg));
            if name != "mod" {
                parts.push(Node::Atom(Atom {
                    text: ")".to_string(),
                    class: Class::Close,
                    variant: fonts::Variant::Normal,
                }));
            }
            return Ok(Some(Node::Group(parts)));
        }

        // fractions
        if name == "frac" || name == "dfrac" || name == "tfrac" || name == "cfrac" {
            let num = self.parse_arg()?;
            let den = self.parse_arg()?;
            let style = if name == "dfrac" || name == "cfrac" {
                Some(StyleSwitch::Display)
            } else if name == "tfrac" {
                Some(StyleSwitch::Text)
            } else {
                None
            };
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar: true,
                left: String::new(),
                right: String::new(),
                style,
                continued: name == "cfrac",
            })));
        }
        if name == "binom" || name == "dbinom" || name == "tbinom" {
            let num = self.parse_arg()?;
            let den = self.parse_arg()?;
            let style = if name == "dbinom" {
                Some(StyleSwitch::Display)
            } else if name == "tbinom" {
                Some(StyleSwitch::Text)
            } else {
                None
            };
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar: false,
                left: "(".to_string(),
                right: ")".to_string(),
                style,
                continued: false,
            })));
        }
        if name == "over" || name == "choose" || name == "atop" {
            // infix: everything before in this list is the numerator
            let num = std::mem::take(out);
            let den = self.parse_list(Stop::Brace)?;
            let is_choose = name == "choose";
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar: name == "over",
                left: if is_choose {
                    "(".to_string()
                } else {
                    String::new()
                },
                right: if is_choose {
                    ")".to_string()
                } else {
                    String::new()
                },
                style: None,
                continued: false,
            })));
        }
        if name == "sqrt" {
            let index = self.optional_arg()?;
            let body = self.parse_arg()?;
            return Ok(Some(Node::Sqrt(Sqrt { body, index })));
        }
        if name == "left" {
            let left = self.delim_token()?.to_string();
            let body = self.parse_list(Stop::Right)?;
            if !self.looking_at_command("right") {
                return Err(ParseError);
            }
            self.i += "\\right".len();
            let right = self.delim_token()?.to_string();
            return Ok(Some(Node::LeftRight(LeftRight { left, right, body })));
        }
        if name == "middle" {
            return Ok(Some(Node::SizedDelim(SizedDelim {
                delim: self.delim_token()?.to_string(),
                size: 0,
                class: Class::Rel,
            })));
        }
        if name == "right" {
            return Err(ParseError);
        }
        for (base, size) in [("big", 1u8), ("Big", 2), ("bigg", 3), ("Bigg", 4)] {
            if let Some(rest) = name.strip_prefix(base) {
                for (suffix, class) in [
                    ("", Class::Ord),
                    ("l", Class::Open),
                    ("r", Class::Close),
                    ("m", Class::Rel),
                ] {
                    if rest == suffix {
                        return Ok(Some(Node::SizedDelim(SizedDelim {
                            delim: self.delim_token()?.to_string(),
                            size,
                            class,
                        })));
                    }
                }
            }
        }
        if let Some(acc) = symbols::accent(name) {
            let base = self.parse_arg()?;
            return Ok(Some(Node::Accent(AccentNode {
                base,
                glyph: acc.glyph.to_string(),
                wide: acc.wide,
                name: acc.name.to_string(),
            })));
        }
        for (n, kind) in [
            ("overline", OverUnderKind::Overline),
            ("underline", OverUnderKind::Underline),
            ("overbrace", OverUnderKind::Overbrace),
            ("underbrace", OverUnderKind::Underbrace),
            ("overrightarrow", OverUnderKind::Overrightarrow),
            ("overleftarrow", OverUnderKind::Overleftarrow),
            ("overleftrightarrow", OverUnderKind::Overleftrightarrow),
            ("underrightarrow", OverUnderKind::Underrightarrow),
            ("underleftarrow", OverUnderKind::Underleftarrow),
        ] {
            if name == n {
                return Ok(Some(Node::OverUnder(OverUnder {
                    body: self.parse_arg()?,
                    kind,
                })));
            }
        }
        for (n, kind) in [
            ("xrightarrow", XArrowKind::Right),
            ("xleftarrow", XArrowKind::Left),
            ("xleftrightarrow", XArrowKind::LeftRight),
            ("xRightarrow", XArrowKind::DoubleRight),
            ("xLeftarrow", XArrowKind::DoubleLeft),
            ("xLeftrightarrow", XArrowKind::DoubleLeftRight),
            ("xhookrightarrow", XArrowKind::HookRight),
            ("xhookleftarrow", XArrowKind::HookLeft),
            ("xmapsto", XArrowKind::MapsTo),
            ("xlongequal", XArrowKind::LongEqual),
            ("xrightleftharpoons", XArrowKind::HarpoonsRl),
            ("xleftrightharpoons", XArrowKind::HarpoonsLr),
            ("xrightleftarrows", XArrowKind::ArrowsRl),
            ("xtofrom", XArrowKind::ArrowsRl),
            ("xrightequilibrium", XArrowKind::HarpoonsRl),
            ("xleftequilibrium", XArrowKind::HarpoonsLr),
            ("xtwoheadrightarrow", XArrowKind::TwoHeadRight),
            ("xtwoheadleftarrow", XArrowKind::TwoHeadLeft),
        ] {
            if name == n {
                self.skip_space();
                let below: Option<Vec<Node>> = if self.peek_byte() == b'[' {
                    self.optional_arg()?
                } else {
                    None
                };
                return Ok(Some(Node::XArrow(XArrow {
                    kind,
                    above: self.parse_arg()?,
                    below,
                })));
            }
        }
        for (n, factor) in [
            ("tiny", 0.5),
            ("scriptsize", 0.7),
            ("footnotesize", 0.8),
            ("small", 0.9),
            ("normalsize", 1.0),
            ("large", 1.2),
            ("Large", 1.44),
            ("LARGE", 1.728),
            ("huge", 2.074),
            ("Huge", 2.488),
        ] {
            if name == n {
                return Ok(Some(Node::Sized {
                    factor,
                    body: self.parse_list(Stop::Brace)?,
                }));
            }
        }
        if name == "substack" {
            // lines centred in script style (amsmath \\substack)
            let body = self.raw_arg()?;
            return Ok(Some(Node::Group(self.sub_parse(&[
                "\\begin{smallmatrix}",
                body,
                "\\end{smallmatrix}",
            ])?)));
        }
        if name == "displaylines" {
            let body = self.raw_arg()?;
            return Ok(Some(Node::Group(self.sub_parse(&[
                "\\begin{gathered}",
                body,
                "\\end{gathered}",
            ])?)));
        }
        for (n, side) in [
            ("mathrlap", LapSide::Right),
            ("rlap", LapSide::Right),
            ("mathllap", LapSide::Left),
            ("llap", LapSide::Left),
            ("mathclap", LapSide::Center),
            ("clap", LapSide::Center),
        ] {
            if name == n {
                let body = if n.as_bytes()[0] == b'm' {
                    self.parse_arg()?
                } else {
                    self.text_or_math_arg()?
                };
                return Ok(Some(Node::Lap { side, body }));
            }
        }
        if name == "mathchoice" {
            let display = self.parse_arg()?;
            let text = self.parse_arg()?;
            let script = self.parse_arg()?;
            let scriptscript = self.parse_arg()?;
            return Ok(Some(Node::Choice([display, text, script, scriptscript])));
        }
        if name == "ce" || name == "pu" {
            // mhchem: the argument becomes TeX (KaTeX defines these as macros)
            let arg = self.raw_arg()?;
            let machine = if name == "ce" {
                mhchem::Machine::Ce
            } else {
                mhchem::Machine::Pu
            };
            return match mhchem::to_tex(arg, machine) {
                Ok(tex) => Ok(Some(Node::Group(self.sub_parse(&[tex.as_str()])?))),
                Err(_) => Ok(Some(Node::Unknown(name.to_string()))),
            };
        }
        if name == "tripledash" {
            // mhchem's macro: three small dashes (a "tripledash" bond)
            return Ok(Some(Node::Group(self.sub_parse(&[
                "{\\vphantom{-}\\raisebox{2.56mu}{$\\mkern2mu\\tiny\\text{-}\\mkern1mu\\text{-}\\mkern1mu\\text{-}\\mkern2mu$}}",
            ])?)));
        }
        if name == "href" {
            let _ = self.raw_arg()?; // the URL: formulas are not clickable
            return Ok(Some(Node::Group(self.parse_arg()?)));
        }
        if name == "url" {
            return Ok(Some(Node::Text(Text {
                text: self.raw_arg()?.to_string(),
                variant: fonts::Variant::Monospace,
                bold: false,
            })));
        }
        if name == "colorbox" {
            let c = parse_color(self.raw_arg()?).unwrap_or(0xffffff00);
            return Ok(Some(Node::ColorBox {
                background: c,
                frame: None,
                body: self.text_or_math_arg()?,
            }));
        }
        if name == "fcolorbox" {
            let frame = parse_color(self.raw_arg()?).unwrap_or(0xff000000);
            let bg = parse_color(self.raw_arg()?).unwrap_or(0xffffffff);
            return Ok(Some(Node::ColorBox {
                background: bg,
                frame: Some(frame),
                body: self.text_or_math_arg()?,
            }));
        }
        if name == "smash" {
            self.skip_space();
            if self.peek_byte() == b'[' {
                let _ = self.optional_arg()?;
            }
            return Ok(Some(Node::Smash(self.parse_arg()?)));
        }
        if name == "mathstrut" {
            return Ok(Some(Node::Phantom {
                body: vec![Node::Atom(Atom {
                    text: "(".to_string(),
                    class: Class::Ord,
                    variant: fonts::Variant::Normal,
                })],
                horizontal: false,
                vertical: true,
            }));
        }
        if name == "rule" {
            self.skip_space();
            if self.peek_byte() == b'[' {
                let _ = self.optional_arg()?;
            }
            let w = parse_dimen(self.raw_arg()?);
            let h = parse_dimen(self.raw_arg()?);
            return Ok(Some(Node::RuleBox {
                width: w,
                height: h,
            }));
        }
        if name == "raisebox" {
            let amount = parse_dimen(self.raw_arg()?);
            return Ok(Some(Node::Raise {
                amount,
                body: self.text_or_math_arg()?,
            }));
        }
        if name == "unicode" || name == "char" {
            let raw: &str = if name == "unicode" {
                self.raw_arg()?
            } else {
                self.skip_space();
                let start = self.i;
                while self.i < self.src.len() {
                    let b = self.src.as_bytes()[self.i];
                    if b.is_ascii_alphanumeric() || b == b'"' || b == b'\'' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                self.slice(start, self.i)
            };
            let Some(cp) = parse_char_code(raw) else {
                return Ok(Some(Node::Unknown(name.to_string())));
            };
            return Ok(Some(Node::Atom(Atom {
                text: cp.to_string(),
                class: Class::Ord,
                variant: fonts::Variant::Normal,
            })));
        }
        if name == "genfrac" {
            let left = strip_braces(self.raw_arg()?);
            let right = strip_braces(self.raw_arg()?);
            let thickness = self.raw_arg()?;
            let style_arg = self.raw_arg()?;
            let num = self.parse_arg()?;
            let den = self.parse_arg()?;
            let bar = !thickness.trim_matches(' ').starts_with('0');
            let style_trimmed = style_arg.trim_matches(' ');
            let style = if style_trimmed == "0" {
                Some(StyleSwitch::Display)
            } else if style_trimmed == "1" {
                Some(StyleSwitch::Text)
            } else {
                None
            };
            return Ok(Some(Node::Frac(Frac {
                num,
                den,
                bar,
                left: symbols::delimiter(&left).unwrap_or("").to_string(),
                right: symbols::delimiter(&right).unwrap_or("").to_string(),
                style,
                continued: false,
            })));
        }
        if name == "Set" || name == "set" {
            let body = self.parse_arg()?;
            return Ok(Some(Node::LeftRight(LeftRight {
                left: "{".to_string(),
                right: "}".to_string(),
                body,
            })));
        }
        if name == "overset" || name == "underset" || name == "stackrel" {
            let annotation = self.parse_arg()?;
            let base = self.parse_arg()?;
            let a = Box::new(Node::Group(annotation));
            let b = Box::new(Node::Group(base));
            let (over, under) = if name == "underset" {
                (None, Some(a))
            } else {
                (Some(a), None)
            };
            return Ok(Some(Node::Stack {
                base: b,
                over,
                under,
            }));
        }
        if let Some(variant) = fonts::from_command(name) {
            let body = self.parse_arg()?;
            return Ok(Some(Node::Group(apply_variant(body, variant))));
        }
        if matches!(
            name,
            "text"
                | "textrm"
                | "textnormal"
                | "mbox"
                | "textup"
                | "textsf"
                | "texttt"
                | "textbf"
                | "textit"
                | "hbox"
                | "textmd"
        ) {
            let raw = self.raw_arg()?;
            let variant = if name == "textit" {
                fonts::Variant::Italic
            } else if name == "texttt" {
                fonts::Variant::Monospace
            } else if name == "textsf" {
                fonts::Variant::SansSerif
            } else {
                fonts::Variant::Roman
            };
            return Ok(Some(Node::Text(Text {
                text: text_mode_string(raw),
                variant,
                bold: name == "textbf",
            })));
        }
        if name == "color" || name == "textcolor" {
            let color_name = self.raw_arg()?;
            let Some(color) = parse_color(color_name) else {
                return Ok(Some(Node::Unknown(color_name.to_string())));
            };
            if name == "textcolor" {
                return Ok(Some(Node::Color(Color {
                    color,
                    body: self.parse_arg()?,
                })));
            }
            // \color switches the rest of the current group
            let rest = self.parse_list(Stop::Brace)?;
            return Ok(Some(Node::Color(Color { color, body: rest })));
        }
        for (n, style) in [
            ("displaystyle", StyleSwitch::Display),
            ("textstyle", StyleSwitch::Text),
            ("scriptstyle", StyleSwitch::Script),
            ("scriptscriptstyle", StyleSwitch::ScriptScript),
        ] {
            if name == n {
                return Ok(Some(Node::Style {
                    style,
                    body: self.parse_list(Stop::Brace)?,
                }));
            }
        }
        if name == "phantom" {
            return Ok(Some(Node::Phantom {
                body: self.parse_arg()?,
                horizontal: true,
                vertical: true,
            }));
        }
        if name == "hphantom" {
            return Ok(Some(Node::Phantom {
                body: self.parse_arg()?,
                horizontal: true,
                vertical: false,
            }));
        }
        if name == "vphantom" {
            return Ok(Some(Node::Phantom {
                body: self.parse_arg()?,
                horizontal: false,
                vertical: true,
            }));
        }
        if name == "boxed" || name == "fbox" {
            return Ok(Some(Node::Boxed(self.parse_arg()?)));
        }
        for (n, kind) in [
            ("cancel", CancelKind::Cancel),
            ("bcancel", CancelKind::Bcancel),
            ("xcancel", CancelKind::Xcancel),
            ("sout", CancelKind::Sout),
        ] {
            if name == n {
                return Ok(Some(Node::Cancel {
                    body: self.parse_arg()?,
                    kind,
                }));
            }
        }
        if name == "not" {
            self.skip_space();
            if self.looking_at("=") {
                self.i += 1;
                return Ok(Some(Node::Atom(Atom {
                    text: "≠".to_string(),
                    class: Class::Rel,
                    variant: fonts::Variant::Normal,
                })));
            }
            if self.looking_at_command("in") {
                self.i += 3;
                return Ok(Some(Node::Atom(Atom {
                    text: "∉".to_string(),
                    class: Class::Rel,
                    variant: fonts::Variant::Normal,
                })));
            }
            let next = match self.parse_atom(out)? {
                Some(next) => next,
                None => return Ok(None),
            };
            return Ok(Some(Node::Cancel {
                body: vec![next],
                kind: CancelKind::Cancel,
            }));
        }
        if name == "tag" || name == "tag*" {
            return Ok(Some(Node::Tag(self.raw_arg()?.to_string())));
        }
        if name == "begin" {
            return Ok(Some(self.environment()?));
        }
        if name == "end" {
            return Err(ParseError);
        }
        if name == "mathop" {
            return Ok(Some(Node::Op(Op {
                text: strip_braces(self.raw_arg()?),
                symbol: false,
                integral: false,
                limits: true,
                forced: None,
            })));
        }
        if matches!(
            name,
            "mathrel"
                | "mathbin"
                | "mathord"
                | "mathopen"
                | "mathclose"
                | "mathpunct"
                | "mathinner"
        ) {
            let body = self.parse_arg()?;
            let class: Class = if name == "mathrel" {
                Class::Rel
            } else if name == "mathbin" {
                Class::Bin
            } else if name == "mathopen" {
                Class::Open
            } else if name == "mathclose" {
                Class::Close
            } else if name == "mathpunct" {
                Class::Punct
            } else if name == "mathinner" {
                Class::Inner
            } else {
                Class::Ord
            };
            if body.len() == 1 {
                match body.into_iter().next().expect("one element") {
                    Node::Atom(mut atom) => {
                        atom.class = class;
                        return Ok(Some(Node::Atom(atom)));
                    }
                    other => return Ok(Some(Node::Group(vec![other]))),
                }
            }
            return Ok(Some(Node::Group(body)));
        }
        Ok(Some(Node::Unknown(name.to_string())))
    }

    /// Parses TeX assembled from `parts` (macros expanding to environments).
    fn sub_parse(&mut self, parts: &[&str]) -> Result<Vec<Node>, ParseError> {
        let src = parts.concat();
        parse(&src)
    }

    /// Text-mode argument (\colorbox, \raisebox): `{text}` as roman text,
    /// `{$math$}` as math.
    fn text_or_math_arg(&mut self) -> Result<Vec<Node>, ParseError> {
        let raw = self.raw_arg()?;
        let t = raw.trim_matches(' ');
        if t.len() >= 2 && t.as_bytes()[0] == b'$' && t.as_bytes()[t.len() - 1] == b'$' {
            return parse(&t[1..t.len() - 1]);
        }
        Ok(vec![Node::Text(Text {
            text: text_mode_string(raw),
            variant: fonts::Variant::Roman,
            bold: false,
        })])
    }

    fn raw_arg_or_dimen(&mut self) -> Result<&'a str, ParseError> {
        self.skip_space();
        if self.peek_byte() == b'{' {
            return self.raw_arg();
        }
        let start = self.i;
        while self.i < self.src.len() {
            let b = self.src.as_bytes()[self.i];
            if b.is_ascii_digit() || b == b'.' || b == b'-' {
                self.i += 1;
            } else {
                break;
            }
        }
        while self.i < self.src.len()
            && self.src.as_bytes()[self.i].is_ascii_alphabetic()
            && self.i - start < 8
        {
            self.i += 1;
        }
        Ok(self.slice(start, self.i))
    }

    fn delim_token(&mut self) -> Result<&'a str, ParseError> {
        self.skip_space();
        if self.i >= self.src.len() {
            return Err(ParseError);
        }
        let token: &'a str;
        if self.peek_byte() == b'\\' {
            let start = self.i;
            let _ = self.read_command_name();
            token = self.slice(start, self.i);
        } else {
            let len = self.next_char_len();
            token = self.slice(self.i, self.i + len);
            self.i += len;
        }
        match symbols::delimiter(token) {
            Some(delim) => Ok(delim),
            None => Err(ParseError),
        }
    }

    fn environment(&mut self) -> Result<Node, ParseError> {
        let name = self.raw_arg()?;
        let mut left = String::new();
        let mut right = String::new();
        let mut display = false;
        let mut col_sep: f32 = 0.5;
        let mut aligned = false;
        let mut small = false;
        let mut col_spec: &'a str = "";
        let mut default_align: u8 = b'c';
        if name == "matrix" || name == "matrix*" {
            // no delimiters
        } else if name == "pmatrix" {
            left = "(".to_string();
            right = ")".to_string();
        } else if name == "bmatrix" {
            left = "[".to_string();
            right = "]".to_string();
        } else if name == "Bmatrix" {
            left = "{".to_string();
            right = "}".to_string();
        } else if name == "vmatrix" {
            left = "|".to_string();
            right = "|".to_string();
        } else if name == "Vmatrix" {
            left = "‖".to_string();
            right = "‖".to_string();
        } else if name == "smallmatrix" {
            small = true;
            col_sep = 0.2778;
        } else if name == "cases" || name == "dcases" {
            left = "{".to_string();
            col_spec = "ll";
            col_sep = 0.5;
            display = name == "dcases";
        } else if name == "rcases" {
            right = "}".to_string();
            col_spec = "ll";
        } else if name == "aligned"
            || name == "align"
            || name == "align*"
            || name == "split"
            || name == "alignat"
            || name == "alignat*"
            || name == "alignedat"
            || name == "eqnarray"
            || name == "eqnarray*"
        {
            if name.starts_with("alignat") || name == "alignedat" {
                let _ = self.raw_arg()?;
            }
            aligned = true;
            display = true;
            col_sep = 0.0;
        } else if name == "gathered"
            || name == "gather"
            || name == "gather*"
            || name == "equation"
            || name == "equation*"
        {
            display = true;
            col_sep = 0.0;
        } else if name == "array" || name == "darray" {
            col_spec = self.raw_arg()?;
            display = name == "darray";
        } else {
            return Err(ParseError);
        }
        if name.ends_with("matrix*")
            && let Some(a) = self.optional_raw()?
        {
            default_align = if !a.is_empty() { a.as_bytes()[0] } else { b'c' };
        }

        let mut rows: Vec<Vec<Vec<Node>>> = Vec::new();
        let mut row_rules: Vec<usize> = Vec::new();
        let mut gaps: Vec<f32> = Vec::new();
        let mut cells: Vec<Vec<Node>> = Vec::new();
        let mut max_cols: usize = 0;
        loop {
            self.skip_space();
            if self.looking_at_command("hline") {
                self.i += "\\hline".len();
                row_rules.push(rows.len());
                continue;
            }
            let cell = self.parse_list(Stop::EndEnv)?;
            cells.push(cell);
            self.skip_space();
            if self.peek_byte() == b'&' {
                self.i += 1;
                continue;
            }
            if self.looking_at("\\\\") || self.looking_at_command("cr") {
                self.i += if self.looking_at("\\\\") { 2 } else { 3 };
                let mut gap: f32 = 0.0;
                self.skip_space();
                if self.peek_byte() == b'[' {
                    let Some(close) = self.src.as_bytes()[self.i..]
                        .iter()
                        .position(|&b| b == b']')
                        .map(|p| p + self.i)
                    else {
                        return Err(ParseError);
                    };
                    gap = parse_dimen(self.slice(self.i + 1, close));
                    self.i = close + 1;
                }
                max_cols = max_cols.max(cells.len());
                rows.push(std::mem::take(&mut cells));
                gaps.push(gap);
                continue;
            }
            if self.looking_at_command("end") {
                self.i += "\\end".len();
                let end_name = self.raw_arg()?;
                if end_name != name {
                    return Err(ParseError);
                }
                // drop a trailing empty row (`a \\ b \\`)
                let empty_last = cells.len() == 1 && cells[0].is_empty();
                if !empty_last || rows.is_empty() {
                    max_cols = max_cols.max(cells.len());
                    rows.push(std::mem::take(&mut cells));
                    gaps.push(0.0);
                }
                break;
            }
            return Err(ParseError);
        }

        let mut cols: Vec<u8> = Vec::new();
        let mut col_rules: Vec<usize> = Vec::new();
        if !col_spec.is_empty() {
            for c in col_spec.as_bytes() {
                match c {
                    b'l' | b'c' | b'r' => cols.push(*c),
                    b'|' => col_rules.push(cols.len()),
                    _ => {}
                }
            }
        }
        while cols.len() < max_cols {
            let a: u8 = if aligned {
                if cols.len().is_multiple_of(2) {
                    b'r'
                } else {
                    b'l'
                }
            } else if !col_spec.is_empty() {
                b'l'
            } else {
                default_align
            };
            cols.push(a);
        }
        Ok(Node::Array(Array {
            rows,
            cols,
            col_rules,
            row_rules,
            left,
            right,
            display,
            col_sep,
            aligned,
            small,
            row_gaps: gaps,
        }))
    }

    fn optional_raw(&mut self) -> Result<Option<&'a str>, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'[' {
            return Ok(None);
        }
        let Some(close) = self.src.as_bytes()[self.i..]
            .iter()
            .position(|&b| b == b']')
            .map(|p| p + self.i)
        else {
            return Err(ParseError);
        };
        let out = self.slice(self.i + 1, close);
        self.i = close + 1;
        Ok(Some(out))
    }
}

fn strip_braces(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'{' || c == b'}' {
            i += 1;
            continue;
        }
        if c == b'\\' && i + 1 < bytes.len() {
            i += 1;
            if bytes[i] == b',' || bytes[i] == b' ' {
                out.push(b' ');
                i += 1;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).expect("strip_braces keeps valid UTF-8")
}

/// `\text{...}` content: strip braces, handle `\$`, `\%`, `\&`, `~`.
fn text_mode_string(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'{' || c == b'}' {
            i += 1;
            continue;
        }
        if c == b'~' {
            out.push(b' ');
            i += 1;
            continue;
        }
        if c == b'\\' && i + 1 < bytes.len() {
            let n = bytes[i + 1];
            if !n.is_ascii_alphabetic() {
                out.push(if n == b' ' || n == b',' { b' ' } else { n });
                i += 2;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8(out).expect("text_mode_string keeps valid UTF-8")
}

fn apply_variant(body: Vec<Node>, variant: fonts::Variant) -> Vec<Node> {
    body.into_iter()
        .map(|node| apply_variant_one(node, variant))
        .collect()
}

fn apply_variant_one(node: Node, variant: fonts::Variant) -> Node {
    match node {
        Node::Atom(mut atom) => {
            atom.variant = variant;
            Node::Atom(atom)
        }
        Node::Group(group) => Node::Group(apply_variant(group, variant)),
        Node::SupSub(mut sup_sub) => {
            if let Some(base) = sup_sub.base.take() {
                sup_sub.base = Some(Box::new(apply_variant_one(*base, variant)));
            }
            Node::SupSub(sup_sub)
        }
        other => other,
    }
}

/// `\unicode{x41}`, `\unicode{65}`, `\char"41`, `\char'101`, `\char65`.
fn parse_char_code(raw: &str) -> Option<char> {
    let t = raw.trim_matches(|c| c == ' ' || c == '{' || c == '}');
    if t.is_empty() {
        return None;
    }
    let bytes = t.as_bytes();
    let v: u32 = if bytes[0] == b'x' || bytes[0] == b'"' {
        u32::from_str_radix(&t[1..], 16).ok()?
    } else if bytes[0] == b'\'' {
        u32::from_str_radix(&t[1..], 8).ok()?
    } else {
        t.parse::<u32>().ok()?
    };
    if v > 0x10FFFF || (0xD800..=0xDFFF).contains(&v) {
        return None;
    }
    char::from_u32(v)
}

/// Parses a TeX dimension into em (`3mu`, `1em`, `2pt`, `0.5ex`).
pub fn parse_dimen(raw: &str) -> f32 {
    let t = raw.trim_matches(|c| c == ' ' || c == '\t');
    let bytes = t.as_bytes();
    let mut end: usize = 0;
    while end < bytes.len()
        && (bytes[end].is_ascii_digit()
            || bytes[end] == b'.'
            || bytes[end] == b'-'
            || bytes[end] == b'+')
    {
        end += 1;
    }
    let value: f32 = t[..end].parse().unwrap_or(0.0);
    let unit = t[end..].trim_matches(' ');
    if unit == "mu" {
        value / 18.0
    } else if unit == "pt" {
        value / 10.0
    } else if unit == "ex" {
        value * 0.431
    } else if unit == "px" {
        value / 16.0
    } else if unit == "mm" {
        value * 0.2845
    } else if unit == "cm" {
        value * 2.845
    } else if unit == "in" {
        value * 7.227
    } else {
        value // em (default)
    }
}

pub fn parse_color(raw: &str) -> Option<u32> {
    let t = raw.trim_matches(|c| c == ' ' || c == '\t');
    if t.is_empty() {
        return None;
    }
    let bytes = t.as_bytes();
    if bytes[0] == b'#' || (t.len() == 6 && is_hex(t)) {
        let hex = if bytes[0] == b'#' { &t[1..] } else { t };
        if hex.len() == 6 {
            return Some(0xff000000 | u32::from_str_radix(hex, 16).ok()?);
        }
        if hex.len() == 3 {
            let v = u32::from_str_radix(hex, 16).ok()?;
            let r = (v >> 8) & 0xf;
            let g = (v >> 4) & 0xf;
            let b = v & 0xf;
            return Some(0xff000000 | (r * 17) << 16 | (g * 17) << 8 | (b * 17));
        }
        return None;
    }
    let named: &[(&str, u32)] = &[
        ("red", 0xffdf0030),
        ("blue", 0xff0000ff),
        ("green", 0xff008000),
        ("black", 0xff000000),
        ("white", 0xffffffff),
        ("gray", 0xff808080),
        ("grey", 0xff808080),
        ("orange", 0xffff8000),
        ("purple", 0xff800080),
        ("magenta", 0xffff00ff),
        ("cyan", 0xff00ffff),
        ("yellow", 0xffffd700),
        ("brown", 0xffa52a2a),
        ("pink", 0xffff69b4),
        ("teal", 0xff008080),
        ("olive", 0xff808000),
        ("violet", 0xff8000ff),
        ("lime", 0xff00ff00),
        ("darkgray", 0xff404040),
        ("lightgray", 0xffbfbfbf),
    ];
    for (n, color) in named {
        if t.eq_ignore_ascii_case(n) {
            return Some(*color);
        }
    }
    None
}

fn is_hex(t: &str) -> bool {
    t.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

pub fn parse(source: &str) -> Result<Vec<Node>, ParseError> {
    let mut p = Parser::init(source);
    p.parse_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_structures() {
        let nodes = parse("\\frac{a}{b} + x_i^2 + \\sqrt[3]{y} \\left( \\sum_{k=1}^n k \\right)")
            .expect("parse structures");
        assert!(matches!(nodes[0], Node::Frac(_)));
        assert!(matches!(nodes[2], Node::SupSub(_)));
        assert!(matches!(nodes[4], Node::Sqrt(_)));
        assert!(matches!(nodes[5], Node::LeftRight(_)));
        let env = parse("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}").expect("pmatrix");
        match &env[0] {
            Node::Array(array) => {
                assert_eq!(array.rows.len(), 2);
                assert_eq!(array.cols.len(), 2);
            }
            other => panic!("expected array, got {other:?}"),
        }
        assert_eq!(parse("\\frac{a}{b"), Err(ParseError));
        let cases =
            parse("f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases}")
                .expect("cases");
        assert!(matches!(cases.last(), Some(Node::Array(_))));
    }

    #[test]
    fn parse_errors() {
        assert_eq!(parse("x^1^2"), Err(ParseError)); // double superscript
        assert_eq!(parse("\\left( x"), Err(ParseError)); // missing \right
        assert_eq!(parse("\\begin{nope} x \\end{nope}"), Err(ParseError));
        assert_eq!(parse("}"), Err(ParseError));
        assert_eq!(parse("\\frac"), Err(ParseError));
    }

    #[test]
    fn parse_font_commands() {
        let nodes = parse("\\mathbb{R}").expect("mathbb");
        match &nodes[0] {
            Node::Group(group) => match &group[0] {
                Node::Atom(atom) => {
                    assert_eq!(atom.text, "R");
                    assert_eq!(atom.variant, fonts::Variant::DoubleStruck);
                }
                other => panic!("expected atom, got {other:?}"),
            },
            other => panic!("expected group, got {other:?}"),
        }
        let nodes = parse("\\text{hi}").expect("text");
        match &nodes[0] {
            Node::Text(text) => {
                assert_eq!(text.text, "hi");
                assert_eq!(text.variant, fonts::Variant::Roman);
            }
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn delimiters_and_arrays() {
        let nodes = parse("\\left( x \\right)").expect("left right");
        match &nodes[0] {
            Node::LeftRight(lr) => {
                assert_eq!(lr.left, "(");
                assert_eq!(lr.right, ")");
            }
            other => panic!("expected left_right, got {other:?}"),
        }
        let nodes = parse("\\begin{bmatrix} 1 \\end{bmatrix}").expect("bmatrix");
        match &nodes[0] {
            Node::Array(array) => {
                assert_eq!(array.left, "[");
                assert_eq!(array.right, "]");
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn mhchem_dispatch() {
        let nodes = parse("\\ce{H2O}").expect("\\ce");
        assert!(matches!(&nodes[0], Node::Group(group) if !group.is_empty()));
        let nodes = parse("\\pu{1 kg}").expect("\\pu");
        assert!(matches!(&nodes[0], Node::Group(_)));
    }

    #[test]
    fn dimen_and_color() {
        assert_eq!(parse_dimen("3mu"), 3.0 / 18.0);
        assert_eq!(parse_dimen("2pt"), 0.2);
        assert_eq!(parse_dimen("1.5"), 1.5);
        assert_eq!(parse_color("#ff0000"), Some(0xffff0000));
        assert_eq!(parse_color("#f00"), Some(0xffff0000));
        assert_eq!(parse_color("red"), Some(0xffdf0030));
        assert_eq!(parse_color("nope"), None);
    }
    /// Every formula of `examples/math.zig` parses (the parser smoke corpus).
    #[test]
    fn example_formulas_parse() {
        let formulas: [&str; 18] = [
            "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}",
            "\\sum_{k=1}^{n} k^2 = \\frac{n(n+1)(2n+1)}{6}",
            "\\int_{-\\infty}^{\\infty} e^{-x^2}\\,dx = \\sqrt{\\pi}",
            "\\lim_{x \\to 0} \\frac{\\sin x}{x} = 1 \\qquad \\prod_{i=1}^{n} a_i",
            "\\left( \\sum_{i=1}^n a_i b_i \\right)^2 \\le \\left( \\sum_{i=1}^n a_i^2 \\right) \\left( \\sum_{i=1}^n b_i^2 \\right)",
            "A = \\begin{pmatrix} a_{11} & a_{12} & a_{13} \\\\ a_{21} & a_{22} & a_{23} \\\\ a_{31} & a_{32} & a_{33} \\end{pmatrix}, \\quad \\det\\begin{vmatrix} a & b \\\\ c & d \\end{vmatrix} = ad - bc",
            "|x| = \\begin{cases} x & \\text{if } x \\ge 0 \\\\ -x & \\text{otherwise} \\end{cases}",
            "\\begin{aligned} (a+b)^2 &= a^2 + 2ab + b^2 \\\\ &\\ne a^2 + b^2 \\end{aligned}",
            "\\hat{\\theta} = \\arg\\max_{\\theta} \\mathcal{L}(\\theta), \\quad \\vec{F} = m\\vec{a}, \\quad \\overline{z} = a - bi",
            "\\nabla \\times \\mathbf{E} = -\\frac{\\partial \\mathbf{B}}{\\partial t}, \\quad \\mathbb{R}^n, \\mathbb{Z}, \\mathfrak{g}",
            "\\sqrt[3]{x^3 + y^3} \\quad \\binom{n}{k} = \\frac{n!}{k!(n-k)!} \\quad \\boxed{E = mc^2}",
            "\\underbrace{1 + 2 + \\cdots + n}_{n \\text{ terms}} \\quad \\overbrace{a + b}^{2} \\quad e^{i\\pi} + 1 = 0",
            "f(x) = \\frac{1}{\\sigma\\sqrt{2\\pi}} e^{-\\frac{(x-\\mu)^2}{2\\sigma^2}}",
            "\\frac{1}{1 + \\frac{1}{1 + \\frac{1}{x}}} \\quad \\frac{\\partial^2 u}{\\partial t^2} = c^2 \\nabla^2 u",
            "\\mathrm{2H_2 + O_2} \\xrightarrow[\\Delta]{\\text{Pt}} \\mathrm{2H_2O} \\quad \\mathrm{N_2 + 3H_2} \\xrightleftharpoons[\\text{high } T]{\\text{cat.}} \\mathrm{2NH_3} \\quad A \\xRightarrow{f} B",
            "\\sum_{\\substack{0<i<m \\\\ 0<j<n}} P(i,j) \\quad \\colorbox{#ffe58f}{$x^2$} \\quad \\fcolorbox{red}{white}{key} \\quad {\\large L}{\\small s} \\quad X \\xhookrightarrow{f} Y \\xmapsto{g} Z \\xlongequal{\\text{def}} W",
            "\\ce{2H2 + O2 ->[\\text{点燃}] 2H2O} \\quad \\ce{N2 + 3H2 <=>[\\text{催化剂}][\\text{高温高压}] 2NH3} \\quad \\ce{CaCO3 ->[\\Delta] CaO + CO2 ^}",
            "\\ce{SO4^2- + Ba^2+ -> BaSO4 v} \\quad \\ce{[Cu(NH3)4]^2+} \\quad \\ce{CuSO4*5H2O} \\quad \\ce{^{227}_{90}Th} \\quad \\ce{Fe^{III}} \\quad \\pu{123 kJ mol-1} \\quad \\pu{1.2e3 kg m/s^2}",
        ];
        for formula in formulas {
            let nodes = parse(formula).unwrap_or_else(|err| panic!("{formula:?}: {err}"));
            assert!(!nodes.is_empty(), "{formula:?} produced no nodes");
        }
    }
}
