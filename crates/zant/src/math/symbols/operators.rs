//! Big operators, named functions (`\sin`, `\lim`, …) and accents.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BigOp {
    pub name: &'static str,
    pub text: &'static str,
    pub limits: bool,
    pub integral: bool,
}

// One entry per line.
#[rustfmt::skip]
pub const BIG_OPS: &[BigOp] = &[
    BigOp { name: "sum", text: "∑", limits: true, integral: false },
    BigOp { name: "prod", text: "∏", limits: true, integral: false },
    BigOp { name: "coprod", text: "∐", limits: true, integral: false },
    BigOp { name: "bigcup", text: "⋃", limits: true, integral: false },
    BigOp { name: "bigcap", text: "⋂", limits: true, integral: false },
    BigOp { name: "bigvee", text: "⋁", limits: true, integral: false },
    BigOp { name: "bigwedge", text: "⋀", limits: true, integral: false },
    BigOp { name: "bigoplus", text: "⨁", limits: true, integral: false },
    BigOp { name: "bigotimes", text: "⨂", limits: true, integral: false },
    BigOp { name: "bigodot", text: "⨀", limits: true, integral: false },
    BigOp { name: "biguplus", text: "⨄", limits: true, integral: false },
    BigOp { name: "bigsqcup", text: "⨆", limits: true, integral: false },
    BigOp { name: "int", text: "∫", limits: false, integral: true },
    BigOp { name: "intop", text: "∫", limits: true, integral: true },
    BigOp { name: "iint", text: "∬", limits: false, integral: true },
    BigOp { name: "iiint", text: "∭", limits: false, integral: true },
    BigOp { name: "oint", text: "∮", limits: false, integral: true },
    BigOp { name: "oiint", text: "∯", limits: false, integral: true },
    BigOp { name: "oiiint", text: "∰", limits: false, integral: true },
    BigOp { name: "smallint", text: "∫", limits: false, integral: true },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Function {
    pub name: &'static str,
    pub text: &'static str,
    pub limits: bool,
}

/// Named operators (`\sin`, `\lim`, ...). `limits` ones take scripts above
/// and below in display style.
// One entry per line.
#[rustfmt::skip]
pub const FUNCTIONS: &[Function] = &[
    Function { name: "sin", text: "sin", limits: false },
    Function { name: "cos", text: "cos", limits: false },
    Function { name: "tan", text: "tan", limits: false },
    Function { name: "cot", text: "cot", limits: false },
    Function { name: "sec", text: "sec", limits: false },
    Function { name: "csc", text: "csc", limits: false },
    Function { name: "arcsin", text: "arcsin", limits: false },
    Function { name: "arccos", text: "arccos", limits: false },
    Function { name: "arctan", text: "arctan", limits: false },
    Function { name: "sinh", text: "sinh", limits: false },
    Function { name: "cosh", text: "cosh", limits: false },
    Function { name: "tanh", text: "tanh", limits: false },
    Function { name: "coth", text: "coth", limits: false },
    Function { name: "log", text: "log", limits: false },
    Function { name: "ln", text: "ln", limits: false },
    Function { name: "lg", text: "lg", limits: false },
    Function { name: "exp", text: "exp", limits: false },
    Function { name: "arg", text: "arg", limits: false },
    Function { name: "deg", text: "deg", limits: false },
    Function { name: "dim", text: "dim", limits: false },
    Function { name: "hom", text: "hom", limits: false },
    Function { name: "ker", text: "ker", limits: false },
    Function { name: "lim", text: "lim", limits: true },
    Function { name: "liminf", text: "lim inf", limits: true },
    Function { name: "limsup", text: "lim sup", limits: true },
    Function { name: "max", text: "max", limits: true },
    Function { name: "min", text: "min", limits: true },
    Function { name: "sup", text: "sup", limits: true },
    Function { name: "inf", text: "inf", limits: true },
    Function { name: "det", text: "det", limits: true },
    Function { name: "gcd", text: "gcd", limits: true },
    Function { name: "Pr", text: "Pr", limits: true },
    Function { name: "argmax", text: "arg max", limits: true },
    Function { name: "argmin", text: "arg min", limits: true },
    Function { name: "plim", text: "plim", limits: true },
    Function { name: "tr", text: "tr", limits: false },
    Function { name: "Tr", text: "Tr", limits: false },
    Function { name: "rank", text: "rank", limits: false },
    Function { name: "sgn", text: "sgn", limits: false },
    Function { name: "mod", text: "mod", limits: false },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    pub name: &'static str,
    pub glyph: &'static str,
    pub wide: bool,
}

// One entry per line.
#[rustfmt::skip]
pub const ACCENTS: &[Accent] = &[
    Accent { name: "hat", glyph: "^", wide: false },
    Accent { name: "check", glyph: "ˇ", wide: false },
    Accent { name: "tilde", glyph: "~", wide: false },
    Accent { name: "acute", glyph: "´", wide: false },
    Accent { name: "grave", glyph: "`", wide: false },
    Accent { name: "dot", glyph: "˙", wide: false },
    Accent { name: "ddot", glyph: "¨", wide: false },
    Accent { name: "breve", glyph: "˘", wide: false },
    Accent { name: "bar", glyph: "¯", wide: false },
    Accent { name: "vec", glyph: "→", wide: false },
    Accent { name: "mathring", glyph: "˚", wide: false },
    Accent { name: "widehat", glyph: "^", wide: true },
    Accent { name: "widetilde", glyph: "~", wide: true },
    Accent { name: "widecheck", glyph: "ˇ", wide: true },
];
