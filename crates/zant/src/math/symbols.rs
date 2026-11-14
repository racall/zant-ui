//! Port of `src/math/symbols.zig` — WP-3.5.
//! TeX command tables (subset of KaTeX's `symbols.js` / `functions`).

/// TeX atom classes (KaTeX `groupTypes`), used for inter-atom spacing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: &'static str,
    pub text: &'static str,
    pub class: Class,
}

// Ported verbatim, one entry per line like the Zig table literal.
#[rustfmt::skip]
pub const TABLE: &[Symbol] = &[
    // Greek lowercase (rendered italic like KaTeX)
    Symbol { name: "alpha", text: "α", class: Class::Ord },
    Symbol { name: "beta", text: "β", class: Class::Ord },
    Symbol { name: "gamma", text: "γ", class: Class::Ord },
    Symbol { name: "delta", text: "δ", class: Class::Ord },
    Symbol { name: "epsilon", text: "ϵ", class: Class::Ord },
    Symbol { name: "varepsilon", text: "ε", class: Class::Ord },
    Symbol { name: "zeta", text: "ζ", class: Class::Ord },
    Symbol { name: "eta", text: "η", class: Class::Ord },
    Symbol { name: "theta", text: "θ", class: Class::Ord },
    Symbol { name: "vartheta", text: "ϑ", class: Class::Ord },
    Symbol { name: "iota", text: "ι", class: Class::Ord },
    Symbol { name: "kappa", text: "κ", class: Class::Ord },
    Symbol { name: "varkappa", text: "ϰ", class: Class::Ord },
    Symbol { name: "lambda", text: "λ", class: Class::Ord },
    Symbol { name: "mu", text: "μ", class: Class::Ord },
    Symbol { name: "nu", text: "ν", class: Class::Ord },
    Symbol { name: "xi", text: "ξ", class: Class::Ord },
    Symbol { name: "omicron", text: "ο", class: Class::Ord },
    Symbol { name: "pi", text: "π", class: Class::Ord },
    Symbol { name: "varpi", text: "ϖ", class: Class::Ord },
    Symbol { name: "rho", text: "ρ", class: Class::Ord },
    Symbol { name: "varrho", text: "ϱ", class: Class::Ord },
    Symbol { name: "sigma", text: "σ", class: Class::Ord },
    Symbol { name: "varsigma", text: "ς", class: Class::Ord },
    Symbol { name: "tau", text: "τ", class: Class::Ord },
    Symbol { name: "upsilon", text: "υ", class: Class::Ord },
    Symbol { name: "phi", text: "ϕ", class: Class::Ord },
    Symbol { name: "varphi", text: "φ", class: Class::Ord },
    Symbol { name: "chi", text: "χ", class: Class::Ord },
    Symbol { name: "psi", text: "ψ", class: Class::Ord },
    Symbol { name: "omega", text: "ω", class: Class::Ord },
    Symbol { name: "digamma", text: "ϝ", class: Class::Ord },
    // Greek uppercase (upright)
    Symbol { name: "Gamma", text: "Γ", class: Class::Ord },
    Symbol { name: "Delta", text: "Δ", class: Class::Ord },
    Symbol { name: "Theta", text: "Θ", class: Class::Ord },
    Symbol { name: "Lambda", text: "Λ", class: Class::Ord },
    Symbol { name: "Xi", text: "Ξ", class: Class::Ord },
    Symbol { name: "Pi", text: "Π", class: Class::Ord },
    Symbol { name: "Sigma", text: "Σ", class: Class::Ord },
    Symbol { name: "Upsilon", text: "Υ", class: Class::Ord },
    Symbol { name: "Phi", text: "Φ", class: Class::Ord },
    Symbol { name: "Psi", text: "Ψ", class: Class::Ord },
    Symbol { name: "Omega", text: "Ω", class: Class::Ord },
    Symbol { name: "varGamma", text: "𝛤", class: Class::Ord },
    Symbol { name: "varDelta", text: "𝛥", class: Class::Ord },
    Symbol { name: "varTheta", text: "𝛩", class: Class::Ord },
    Symbol { name: "varLambda", text: "𝛬", class: Class::Ord },
    Symbol { name: "varPi", text: "𝛱", class: Class::Ord },
    Symbol { name: "varSigma", text: "𝛴", class: Class::Ord },
    Symbol { name: "varPhi", text: "𝛷", class: Class::Ord },
    Symbol { name: "varPsi", text: "𝛹", class: Class::Ord },
    Symbol { name: "varOmega", text: "𝛺", class: Class::Ord },
    // Hebrew / misc ord
    Symbol { name: "aleph", text: "ℵ", class: Class::Ord },
    Symbol { name: "beth", text: "ℶ", class: Class::Ord },
    Symbol { name: "gimel", text: "ℷ", class: Class::Ord },
    Symbol { name: "infty", text: "∞", class: Class::Ord },
    Symbol { name: "partial", text: "∂", class: Class::Ord },
    Symbol { name: "nabla", text: "∇", class: Class::Ord },
    Symbol { name: "forall", text: "∀", class: Class::Ord },
    Symbol { name: "exists", text: "∃", class: Class::Ord },
    Symbol { name: "nexists", text: "∄", class: Class::Ord },
    Symbol { name: "emptyset", text: "∅", class: Class::Ord },
    Symbol { name: "varnothing", text: "∅", class: Class::Ord },
    Symbol { name: "hbar", text: "ℏ", class: Class::Ord },
    Symbol { name: "hslash", text: "ℏ", class: Class::Ord },
    Symbol { name: "ell", text: "ℓ", class: Class::Ord },
    Symbol { name: "wp", text: "℘", class: Class::Ord },
    Symbol { name: "Re", text: "ℜ", class: Class::Ord },
    Symbol { name: "Im", text: "ℑ", class: Class::Ord },
    Symbol { name: "imath", text: "ı", class: Class::Ord },
    Symbol { name: "jmath", text: "ȷ", class: Class::Ord },
    Symbol { name: "angle", text: "∠", class: Class::Ord },
    Symbol { name: "measuredangle", text: "∡", class: Class::Ord },
    Symbol { name: "triangle", text: "△", class: Class::Ord },
    Symbol { name: "square", text: "□", class: Class::Ord },
    Symbol { name: "Box", text: "□", class: Class::Ord },
    Symbol { name: "blacksquare", text: "■", class: Class::Ord },
    Symbol { name: "lozenge", text: "◊", class: Class::Ord },
    Symbol { name: "bot", text: "⊥", class: Class::Ord },
    Symbol { name: "top", text: "⊤", class: Class::Ord },
    Symbol { name: "neg", text: "¬", class: Class::Ord },
    Symbol { name: "lnot", text: "¬", class: Class::Ord },
    Symbol { name: "prime", text: "′", class: Class::Ord },
    Symbol { name: "backprime", text: "‵", class: Class::Ord },
    Symbol { name: "degree", text: "°", class: Class::Ord },
    Symbol { name: "clubsuit", text: "♣", class: Class::Ord },
    Symbol { name: "diamondsuit", text: "♢", class: Class::Ord },
    Symbol { name: "heartsuit", text: "♡", class: Class::Ord },
    Symbol { name: "spadesuit", text: "♠", class: Class::Ord },
    Symbol { name: "flat", text: "♭", class: Class::Ord },
    Symbol { name: "natural", text: "♮", class: Class::Ord },
    Symbol { name: "sharp", text: "♯", class: Class::Ord },
    Symbol { name: "checkmark", text: "✓", class: Class::Ord },
    Symbol { name: "dagger", text: "†", class: Class::Ord },
    Symbol { name: "ddagger", text: "‡", class: Class::Ord },
    Symbol { name: "S", text: "§", class: Class::Ord },
    Symbol { name: "P", text: "¶", class: Class::Ord },
    Symbol { name: "copyright", text: "©", class: Class::Ord },
    Symbol { name: "pounds", text: "£", class: Class::Ord },
    Symbol { name: "yen", text: "¥", class: Class::Ord },
    Symbol { name: "surd", text: "√", class: Class::Ord },
    Symbol { name: "complement", text: "∁", class: Class::Ord },
    Symbol { name: "Finv", text: "Ⅎ", class: Class::Ord },
    Symbol { name: "mho", text: "℧", class: Class::Ord },
    Symbol { name: "eth", text: "ð", class: Class::Ord },
    Symbol { name: "diagup", text: "╱", class: Class::Ord },
    Symbol { name: "diagdown", text: "╲", class: Class::Ord },
    Symbol { name: "star", text: "⋆", class: Class::Bin },
    Symbol { name: "ldots", text: "…", class: Class::Inner },
    Symbol { name: "dots", text: "…", class: Class::Inner },
    Symbol { name: "cdots", text: "⋯", class: Class::Inner },
    Symbol { name: "vdots", text: "⋮", class: Class::Ord },
    Symbol { name: "ddots", text: "⋱", class: Class::Inner },
    Symbol { name: "dotsc", text: "…", class: Class::Inner },
    Symbol { name: "dotsb", text: "⋯", class: Class::Inner },
    Symbol { name: "dotsm", text: "⋯", class: Class::Inner },
    Symbol { name: "dotsi", text: "⋯", class: Class::Inner },
    Symbol { name: "dotso", text: "…", class: Class::Inner },
    // binary operators
    Symbol { name: "pm", text: "±", class: Class::Bin },
    Symbol { name: "mp", text: "∓", class: Class::Bin },
    Symbol { name: "times", text: "×", class: Class::Bin },
    Symbol { name: "div", text: "÷", class: Class::Bin },
    Symbol { name: "cdot", text: "⋅", class: Class::Bin },
    Symbol { name: "ast", text: "∗", class: Class::Bin },
    Symbol { name: "circ", text: "∘", class: Class::Bin },
    Symbol { name: "bullet", text: "∙", class: Class::Bin },
    Symbol { name: "oplus", text: "⊕", class: Class::Bin },
    Symbol { name: "ominus", text: "⊖", class: Class::Bin },
    Symbol { name: "otimes", text: "⊗", class: Class::Bin },
    Symbol { name: "oslash", text: "⊘", class: Class::Bin },
    Symbol { name: "odot", text: "⊙", class: Class::Bin },
    Symbol { name: "cap", text: "∩", class: Class::Bin },
    Symbol { name: "cup", text: "∪", class: Class::Bin },
    Symbol { name: "sqcap", text: "⊓", class: Class::Bin },
    Symbol { name: "sqcup", text: "⊔", class: Class::Bin },
    Symbol { name: "uplus", text: "⊎", class: Class::Bin },
    Symbol { name: "wedge", text: "∧", class: Class::Bin },
    Symbol { name: "land", text: "∧", class: Class::Bin },
    Symbol { name: "vee", text: "∨", class: Class::Bin },
    Symbol { name: "lor", text: "∨", class: Class::Bin },
    Symbol { name: "setminus", text: "∖", class: Class::Bin },
    Symbol { name: "smallsetminus", text: "∖", class: Class::Bin },
    Symbol { name: "wr", text: "≀", class: Class::Bin },
    Symbol { name: "amalg", text: "⨿", class: Class::Bin },
    Symbol { name: "diamond", text: "⋄", class: Class::Bin },
    Symbol { name: "bigtriangleup", text: "△", class: Class::Bin },
    Symbol { name: "bigtriangledown", text: "▽", class: Class::Bin },
    Symbol { name: "triangleleft", text: "◃", class: Class::Bin },
    Symbol { name: "triangleright", text: "▹", class: Class::Bin },
    Symbol { name: "bigcirc", text: "◯", class: Class::Bin },
    Symbol { name: "dotplus", text: "∔", class: Class::Bin },
    Symbol { name: "ltimes", text: "⋉", class: Class::Bin },
    Symbol { name: "rtimes", text: "⋊", class: Class::Bin },
    Symbol { name: "boxplus", text: "⊞", class: Class::Bin },
    Symbol { name: "boxminus", text: "⊟", class: Class::Bin },
    Symbol { name: "boxtimes", text: "⊠", class: Class::Bin },
    Symbol { name: "boxdot", text: "⊡", class: Class::Bin },
    Symbol { name: "divideontimes", text: "⋇", class: Class::Bin },
    // relations
    Symbol { name: "leq", text: "≤", class: Class::Rel },
    Symbol { name: "le", text: "≤", class: Class::Rel },
    Symbol { name: "geq", text: "≥", class: Class::Rel },
    Symbol { name: "ge", text: "≥", class: Class::Rel },
    Symbol { name: "neq", text: "≠", class: Class::Rel },
    Symbol { name: "ne", text: "≠", class: Class::Rel },
    Symbol { name: "leqslant", text: "⩽", class: Class::Rel },
    Symbol { name: "geqslant", text: "⩾", class: Class::Rel },
    Symbol { name: "ll", text: "≪", class: Class::Rel },
    Symbol { name: "gg", text: "≫", class: Class::Rel },
    Symbol { name: "lll", text: "⋘", class: Class::Rel },
    Symbol { name: "ggg", text: "⋙", class: Class::Rel },
    Symbol { name: "approx", text: "≈", class: Class::Rel },
    Symbol { name: "approxeq", text: "≊", class: Class::Rel },
    Symbol { name: "sim", text: "∼", class: Class::Rel },
    Symbol { name: "simeq", text: "≃", class: Class::Rel },
    Symbol { name: "cong", text: "≅", class: Class::Rel },
    Symbol { name: "equiv", text: "≡", class: Class::Rel },
    Symbol { name: "propto", text: "∝", class: Class::Rel },
    Symbol { name: "varpropto", text: "∝", class: Class::Rel },
    Symbol { name: "doteq", text: "≐", class: Class::Rel },
    Symbol { name: "triangleq", text: "≜", class: Class::Rel },
    Symbol { name: "coloneqq", text: "≔", class: Class::Rel },
    Symbol { name: "eqqcolon", text: "≕", class: Class::Rel },
    Symbol { name: "coloneq", text: "≔", class: Class::Rel },
    Symbol { name: "Coloneqq", text: "⩴", class: Class::Rel },
    Symbol { name: "prec", text: "≺", class: Class::Rel },
    Symbol { name: "succ", text: "≻", class: Class::Rel },
    Symbol { name: "preceq", text: "⪯", class: Class::Rel },
    Symbol { name: "succeq", text: "⪰", class: Class::Rel },
    Symbol { name: "in", text: "∈", class: Class::Rel },
    Symbol { name: "notin", text: "∉", class: Class::Rel },
    Symbol { name: "ni", text: "∋", class: Class::Rel },
    Symbol { name: "owns", text: "∋", class: Class::Rel },
    Symbol { name: "subset", text: "⊂", class: Class::Rel },
    Symbol { name: "supset", text: "⊃", class: Class::Rel },
    Symbol { name: "subseteq", text: "⊆", class: Class::Rel },
    Symbol { name: "supseteq", text: "⊇", class: Class::Rel },
    Symbol { name: "subsetneq", text: "⊊", class: Class::Rel },
    Symbol { name: "supsetneq", text: "⊋", class: Class::Rel },
    Symbol { name: "nsubseteq", text: "⊈", class: Class::Rel },
    Symbol { name: "nsupseteq", text: "⊉", class: Class::Rel },
    Symbol { name: "sqsubseteq", text: "⊑", class: Class::Rel },
    Symbol { name: "sqsupseteq", text: "⊒", class: Class::Rel },
    Symbol { name: "parallel", text: "∥", class: Class::Rel },
    Symbol { name: "nparallel", text: "∦", class: Class::Rel },
    Symbol { name: "perp", text: "⊥", class: Class::Rel },
    Symbol { name: "mid", text: "∣", class: Class::Rel },
    Symbol { name: "nmid", text: "∤", class: Class::Rel },
    Symbol { name: "vdash", text: "⊢", class: Class::Rel },
    Symbol { name: "dashv", text: "⊣", class: Class::Rel },
    Symbol { name: "models", text: "⊨", class: Class::Rel },
    Symbol { name: "vDash", text: "⊨", class: Class::Rel },
    Symbol { name: "Vdash", text: "⊩", class: Class::Rel },
    Symbol { name: "smile", text: "⌣", class: Class::Rel },
    Symbol { name: "frown", text: "⌢", class: Class::Rel },
    Symbol { name: "asymp", text: "≍", class: Class::Rel },
    Symbol { name: "bowtie", text: "⋈", class: Class::Rel },
    Symbol { name: "lesssim", text: "≲", class: Class::Rel },
    Symbol { name: "gtrsim", text: "≳", class: Class::Rel },
    Symbol { name: "nless", text: "≮", class: Class::Rel },
    Symbol { name: "ngtr", text: "≯", class: Class::Rel },
    Symbol { name: "nleq", text: "≰", class: Class::Rel },
    Symbol { name: "ngeq", text: "≱", class: Class::Rel },
    Symbol { name: "lneq", text: "⪇", class: Class::Rel },
    Symbol { name: "gneq", text: "⪈", class: Class::Rel },
    Symbol { name: "nsim", text: "≁", class: Class::Rel },
    Symbol { name: "ncong", text: "≇", class: Class::Rel },
    Symbol { name: "therefore", text: "∴", class: Class::Rel },
    Symbol { name: "because", text: "∵", class: Class::Rel },
    Symbol { name: "colon", text: ":", class: Class::Punct },
    Symbol { name: "vcentcolon", text: ":", class: Class::Rel },
    // arrows (relations)
    Symbol { name: "to", text: "→", class: Class::Rel },
    Symbol { name: "rightarrow", text: "→", class: Class::Rel },
    Symbol { name: "leftarrow", text: "←", class: Class::Rel },
    Symbol { name: "gets", text: "←", class: Class::Rel },
    Symbol { name: "leftrightarrow", text: "↔", class: Class::Rel },
    Symbol { name: "Rightarrow", text: "⇒", class: Class::Rel },
    Symbol { name: "Leftarrow", text: "⇐", class: Class::Rel },
    Symbol { name: "Leftrightarrow", text: "⇔", class: Class::Rel },
    Symbol { name: "implies", text: "⟹", class: Class::Rel },
    Symbol { name: "impliedby", text: "⟸", class: Class::Rel },
    Symbol { name: "iff", text: "⟺", class: Class::Rel },
    Symbol { name: "longrightarrow", text: "⟶", class: Class::Rel },
    Symbol { name: "longleftarrow", text: "⟵", class: Class::Rel },
    Symbol { name: "longleftrightarrow", text: "⟷", class: Class::Rel },
    Symbol { name: "Longrightarrow", text: "⟹", class: Class::Rel },
    Symbol { name: "Longleftarrow", text: "⟸", class: Class::Rel },
    Symbol { name: "Longleftrightarrow", text: "⟺", class: Class::Rel },
    Symbol { name: "mapsto", text: "↦", class: Class::Rel },
    Symbol { name: "longmapsto", text: "⟼", class: Class::Rel },
    Symbol { name: "hookrightarrow", text: "↪", class: Class::Rel },
    Symbol { name: "hookleftarrow", text: "↩", class: Class::Rel },
    Symbol { name: "uparrow", text: "↑", class: Class::Rel },
    Symbol { name: "downarrow", text: "↓", class: Class::Rel },
    Symbol { name: "updownarrow", text: "↕", class: Class::Rel },
    Symbol { name: "Uparrow", text: "⇑", class: Class::Rel },
    Symbol { name: "Downarrow", text: "⇓", class: Class::Rel },
    Symbol { name: "Updownarrow", text: "⇕", class: Class::Rel },
    Symbol { name: "nearrow", text: "↗", class: Class::Rel },
    Symbol { name: "searrow", text: "↘", class: Class::Rel },
    Symbol { name: "swarrow", text: "↙", class: Class::Rel },
    Symbol { name: "nwarrow", text: "↖", class: Class::Rel },
    Symbol { name: "rightleftharpoons", text: "⇌", class: Class::Rel },
    Symbol { name: "leftrightarrows", text: "⇆", class: Class::Rel },
    Symbol { name: "rightleftarrows", text: "⇄", class: Class::Rel },
    Symbol { name: "geqq", text: "≧", class: Class::Rel },
    Symbol { name: "leqq", text: "≦", class: Class::Rel },
    Symbol { name: "lessgtr", text: "≶", class: Class::Rel },
    Symbol { name: "gtrless", text: "≷", class: Class::Rel },
    Symbol { name: "textdegree", text: "°", class: Class::Ord },
    Symbol { name: "micro", text: "µ", class: Class::Ord },
    Symbol { name: "ohm", text: "Ω", class: Class::Ord },
    Symbol { name: "celsius", text: "℃", class: Class::Ord },
    Symbol { name: "And", text: "&", class: Class::Ord },
    Symbol { name: "AA", text: "Å", class: Class::Ord },
    Symbol { name: "aa", text: "å", class: Class::Ord },
    Symbol { name: "leftrightharpoons", text: "⇋", class: Class::Rel },
    Symbol { name: "rightharpoonup", text: "⇀", class: Class::Rel },
    Symbol { name: "rightharpoondown", text: "⇁", class: Class::Rel },
    Symbol { name: "leftharpoonup", text: "↼", class: Class::Rel },
    Symbol { name: "leftharpoondown", text: "↽", class: Class::Rel },
    Symbol { name: "rightrightarrows", text: "⇉", class: Class::Rel },
    Symbol { name: "leftleftarrows", text: "⇇", class: Class::Rel },
    Symbol { name: "leadsto", text: "⇝", class: Class::Rel },
    Symbol { name: "circlearrowleft", text: "↺", class: Class::Rel },
    Symbol { name: "circlearrowright", text: "↻", class: Class::Rel },
    Symbol { name: "nrightarrow", text: "↛", class: Class::Rel },
    Symbol { name: "nleftarrow", text: "↚", class: Class::Rel },
    Symbol { name: "nRightarrow", text: "⇏", class: Class::Rel },
    Symbol { name: "twoheadrightarrow", text: "↠", class: Class::Rel },
    // delimiters as ordinary symbols
    Symbol { name: "lbrace", text: "{", class: Class::Open },
    Symbol { name: "rbrace", text: "}", class: Class::Close },
    Symbol { name: "langle", text: "⟨", class: Class::Open },
    Symbol { name: "rangle", text: "⟩", class: Class::Close },
    Symbol { name: "lfloor", text: "⌊", class: Class::Open },
    Symbol { name: "rfloor", text: "⌋", class: Class::Close },
    Symbol { name: "lceil", text: "⌈", class: Class::Open },
    Symbol { name: "rceil", text: "⌉", class: Class::Close },
    Symbol { name: "lvert", text: "|", class: Class::Open },
    Symbol { name: "rvert", text: "|", class: Class::Close },
    Symbol { name: "lVert", text: "‖", class: Class::Open },
    Symbol { name: "rVert", text: "‖", class: Class::Close },
    Symbol { name: "vert", text: "|", class: Class::Ord },
    Symbol { name: "Vert", text: "‖", class: Class::Ord },
    Symbol { name: "lbrack", text: "[", class: Class::Open },
    Symbol { name: "rbrack", text: "]", class: Class::Close },
    Symbol { name: "lgroup", text: "⟮", class: Class::Open },
    Symbol { name: "rgroup", text: "⟯", class: Class::Close },
    Symbol { name: "llbracket", text: "⟦", class: Class::Open },
    Symbol { name: "rrbracket", text: "⟧", class: Class::Close },
    Symbol { name: "backslash", text: "∖", class: Class::Ord },
    // escaped characters
    Symbol { name: "{", text: "{", class: Class::Open },
    Symbol { name: "}", text: "}", class: Class::Close },
    Symbol { name: "|", text: "‖", class: Class::Ord },
    Symbol { name: "#", text: "#", class: Class::Ord },
    Symbol { name: "$", text: "$", class: Class::Ord },
    Symbol { name: "%", text: "%", class: Class::Ord },
    Symbol { name: "&", text: "&", class: Class::Ord },
    Symbol { name: "_", text: "_", class: Class::Ord },
    Symbol { name: "textbackslash", text: "\\", class: Class::Ord },
    Symbol { name: "lt", text: "<", class: Class::Rel },
    Symbol { name: "gt", text: ">", class: Class::Rel },
];

pub fn lookup(name: &str) -> Option<Symbol> {
    TABLE.iter().find(|sym| sym.name == name).copied()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BigOp {
    pub name: &'static str,
    pub text: &'static str,
    pub limits: bool,
    pub integral: bool,
}

// Ported verbatim, one entry per line like the Zig table literal.
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

pub fn big_op(name: &str) -> Option<BigOp> {
    BIG_OPS.iter().find(|op| op.name == name).copied()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Function {
    pub name: &'static str,
    pub text: &'static str,
    pub limits: bool,
}

/// Named operators (`\sin`, `\lim`, ...). `limits` ones take scripts above
/// and below in display style.
// Ported verbatim, one entry per line like the Zig table literal.
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

pub fn function(name: &str) -> Option<Function> {
    FUNCTIONS.iter().find(|f| f.name == name).copied()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    pub name: &'static str,
    pub glyph: &'static str,
    pub wide: bool,
}

// Ported verbatim, one entry per line like the Zig table literal.
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

pub fn accent(name: &str) -> Option<Accent> {
    ACCENTS.iter().find(|a| a.name == name).copied()
}

/// Delimiter names usable after `\left`, `\right`, `\big` ... → glyph text.
pub fn delimiter(token: &str) -> Option<&str> {
    if token.len() == 1 {
        return match token.as_bytes()[0] {
            b'(' | b')' | b'[' | b']' | b'|' | b'/' | b'<' | b'>' => match token.as_bytes()[0] {
                b'<' => Some("⟨"),
                b'>' => Some("⟩"),
                _ => Some(token),
            },
            b'.' => Some(""),
            _ => None,
        };
    }
    let name = token.strip_prefix('\\').unwrap_or(token);
    if name == "{" || name == "lbrace" {
        return Some("{");
    }
    if name == "}" || name == "rbrace" {
        return Some("}");
    }
    if name == "|" || name == "Vert" || name == "lVert" || name == "rVert" {
        return Some("‖");
    }
    if name == "vert" || name == "lvert" || name == "rvert" {
        return Some("|");
    }
    if name == "backslash" {
        return Some("\\");
    }
    for n in [
        "langle",
        "rangle",
        "lfloor",
        "rfloor",
        "lceil",
        "rceil",
        "lgroup",
        "rgroup",
        "uparrow",
        "downarrow",
        "updownarrow",
        "Uparrow",
        "Downarrow",
        "Updownarrow",
        "llbracket",
        "rrbracket",
    ] {
        if name == n {
            return Some(lookup(n).expect("delimiter is in the symbol table").text);
        }
    }
    None
}

/// Class of a literal character in math mode.
pub fn char_class(cp: char) -> Class {
    match cp {
        '+' | '-' | '*' | '\u{2212}' | '\u{00B1}' | '\u{00D7}' | '\u{00F7}' | '\u{22C5}'
        | '\u{2218}' | '\u{2219}' => Class::Bin,
        '=' | '<' | '>' | ':' | '\u{2264}' | '\u{2265}' | '\u{2260}' | '\u{2248}' | '\u{2261}'
        | '\u{223C}' | '\u{2208}' | '\u{2209}' | '\u{2282}' | '\u{2283}' | '\u{2286}'
        | '\u{2287}' | '\u{2192}' | '\u{2190}' | '\u{21D2}' | '\u{21D4}' => Class::Rel,
        '(' | '[' | '\u{27E8}' => Class::Open,
        ')' | ']' | '!' | '?' | '\u{27E9}' => Class::Close,
        ',' | ';' => Class::Punct,
        _ => Class::Ord,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Counts match the Zig table literals in `symbols.zig`.
    #[test]
    fn table_counts() {
        assert_eq!(TABLE.len(), 321);
        assert_eq!(BIG_OPS.len(), 20);
        assert_eq!(FUNCTIONS.len(), 40);
        assert_eq!(ACCENTS.len(), 14);
    }

    /// `lookup` returns the first table entry with the given name; the Zig
    /// table has no duplicate names.
    #[test]
    fn table_names_are_unique() {
        for (i, sym) in TABLE.iter().enumerate() {
            for other in &TABLE[i + 1..] {
                assert_ne!(sym.name, other.name);
            }
        }
    }

    #[test]
    fn lookup_samples() {
        assert_eq!(
            lookup("alpha"),
            Some(Symbol {
                name: "alpha",
                text: "α",
                class: Class::Ord
            })
        );
        assert_eq!(lookup("pm").expect("pm").class, Class::Bin);
        assert_eq!(lookup("leq").expect("leq").class, Class::Rel);
        assert_eq!(lookup("lbrace").expect("lbrace").class, Class::Open);
        assert_eq!(lookup("rbrace").expect("rbrace").class, Class::Close);
        assert_eq!(lookup("colon").expect("colon").class, Class::Punct);
        assert_eq!(lookup("ldots").expect("ldots").class, Class::Inner);
        assert_eq!(lookup("textbackslash").expect("textbackslash").text, "\\");
        assert_eq!(lookup("not-a-command"), None);
    }

    #[test]
    fn big_ops_and_functions() {
        let sum = big_op("sum").expect("sum");
        assert!(sum.limits);
        assert!(!sum.integral);
        let int = big_op("int").expect("int");
        assert!(!int.limits);
        assert!(int.integral);
        assert!(function("lim").expect("lim").limits);
        assert!(!function("sin").expect("sin").limits);
        assert_eq!(function("liminf").expect("liminf").text, "lim inf");
        assert_eq!(big_op("?"), None);
        assert_eq!(function("?"), None);
    }

    #[test]
    fn accent_samples() {
        assert!(!accent("hat").expect("hat").wide);
        assert!(accent("widehat").expect("widehat").wide);
        assert_eq!(accent("vec").expect("vec").glyph, "→");
        assert_eq!(accent("?"), None);
    }

    #[test]
    fn delimiters() {
        assert_eq!(delimiter("("), Some("("));
        assert_eq!(delimiter("]"), Some("]"));
        assert_eq!(delimiter("<"), Some("⟨"));
        assert_eq!(delimiter(">"), Some("⟩"));
        assert_eq!(delimiter("."), Some(""));
        assert_eq!(delimiter("x"), None);
        assert_eq!(delimiter("\\{"), Some("{"));
        assert_eq!(delimiter("\\lbrace"), Some("{"));
        assert_eq!(delimiter("\\}"), Some("}"));
        assert_eq!(delimiter("|"), Some("|"));
        assert_eq!(delimiter("\\|"), Some("‖"));
        assert_eq!(delimiter("\\Vert"), Some("‖"));
        assert_eq!(delimiter("\\vert"), Some("|"));
        assert_eq!(delimiter("\\backslash"), Some("\\"));
        assert_eq!(delimiter("\\langle"), Some("⟨"));
        assert_eq!(delimiter("\\Updownarrow"), Some("⇕"));
        assert_eq!(delimiter("\\unknown"), None);
    }

    #[test]
    fn char_classes() {
        assert_eq!(char_class('+'), Class::Bin);
        assert_eq!(char_class('\u{2212}'), Class::Bin);
        assert_eq!(char_class('='), Class::Rel);
        assert_eq!(char_class('\u{2192}'), Class::Rel);
        assert_eq!(char_class('('), Class::Open);
        assert_eq!(char_class('\u{27E8}'), Class::Open);
        assert_eq!(char_class(']'), Class::Close);
        assert_eq!(char_class('!'), Class::Close);
        assert_eq!(char_class(','), Class::Punct);
        assert_eq!(char_class('x'), Class::Ord);
    }
}
