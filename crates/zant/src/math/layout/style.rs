//! TeX styles, font metrics, and rule thickness.

use super::Engine;

// ── styles (KaTeX Style.js) ───────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Display,
    Text,
    Script,
    ScriptScript,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub size: Size,
    pub cramped: bool,
}

impl Style {
    pub const DISPLAY: Style = Style {
        size: Size::Display,
        cramped: false,
    };
    pub const TEXT: Style = Style {
        size: Size::Text,
        cramped: false,
    };

    /// A non-cramped style of the given size.
    pub const fn new(size: Size) -> Style {
        Style {
            size,
            cramped: false,
        }
    }

    pub fn sup(self) -> Style {
        Style {
            size: smaller(self.size),
            cramped: self.cramped,
        }
    }

    pub fn sub(self) -> Style {
        Style {
            size: smaller(self.size),
            cramped: true,
        }
    }

    pub fn frac_num(self) -> Style {
        Style {
            size: match self.size {
                Size::Display => Size::Text,
                Size::Text => Size::Script,
                _ => Size::ScriptScript,
            },
            cramped: self.cramped,
        }
    }

    pub fn frac_den(self) -> Style {
        let mut s = self.frac_num();
        s.cramped = true;
        s
    }

    pub fn cramp(self) -> Style {
        Style {
            size: self.size,
            cramped: true,
        }
    }

    fn metric_index(self) -> usize {
        match self.size {
            Size::Display | Size::Text => 0,
            Size::Script => 1,
            Size::ScriptScript => 2,
        }
    }

    fn multiplier(self) -> f32 {
        match self.size {
            Size::Display | Size::Text => 1.0,
            Size::Script => 0.7,
            Size::ScriptScript => 0.5,
        }
    }

    pub(super) fn tight(self) -> bool {
        self.size == Size::Script || self.size == Size::ScriptScript
    }
}

fn smaller(s: Size) -> Size {
    match s {
        Size::Display | Size::Text => Size::Script,
        Size::Script | Size::ScriptScript => Size::ScriptScript,
    }
}

// ── font metrics (KaTeX sigmasAndXis, per text/script/scriptscript) ───────

#[derive(Clone, Copy, Debug)]
pub(super) enum Metric {
    XHeight,
    /// Kept from KaTeX's sigma table even though the layout never reads it.
    #[allow(dead_code)]
    Quad,
    Num1,
    Num2,
    Num3,
    Denom1,
    Denom2,
    Sup1,
    Sup2,
    Sup3,
    Sub1,
    Sub2,
    SupDrop,
    SubDrop,
    Delim1,
    Delim2,
    AxisHeight,
    DefaultRuleThickness,
    BigOpSpacing1,
    BigOpSpacing2,
    BigOpSpacing3,
    BigOpSpacing4,
    BigOpSpacing5,
}

impl Metric {
    /// The text / script / scriptscript value of the sigma.
    fn sigmas(self) -> [f32; 3] {
        match self {
            Metric::XHeight => [0.431, 0.431, 0.431],
            Metric::Quad => [1.000, 1.171, 1.472],
            Metric::Num1 => [0.677, 0.732, 0.925],
            Metric::Num2 => [0.394, 0.384, 0.387],
            Metric::Num3 => [0.444, 0.471, 0.504],
            Metric::Denom1 => [0.686, 0.752, 1.025],
            Metric::Denom2 => [0.345, 0.344, 0.532],
            Metric::Sup1 => [0.413, 0.503, 0.504],
            Metric::Sup2 => [0.363, 0.431, 0.404],
            Metric::Sup3 => [0.289, 0.286, 0.294],
            Metric::Sub1 => [0.150, 0.143, 0.200],
            Metric::Sub2 => [0.247, 0.286, 0.400],
            Metric::SupDrop => [0.386, 0.353, 0.494],
            Metric::SubDrop => [0.050, 0.071, 0.100],
            Metric::Delim1 => [2.390, 1.700, 1.980],
            Metric::Delim2 => [1.010, 1.157, 1.420],
            Metric::AxisHeight => [0.250, 0.250, 0.250],
            Metric::DefaultRuleThickness => [0.04, 0.049, 0.049],
            Metric::BigOpSpacing1 => [0.111, 0.111, 0.111],
            Metric::BigOpSpacing2 => [0.166, 0.166, 0.166],
            Metric::BigOpSpacing3 => [0.2, 0.2, 0.2],
            Metric::BigOpSpacing4 => [0.6, 0.611, 0.611],
            Metric::BigOpSpacing5 => [0.1, 0.143, 0.143],
        }
    }
}

impl Engine {
    pub(super) fn em(&self, style: Style) -> f32 {
        self.options.size * style.multiplier()
    }

    pub(super) fn metric(&self, style: Style, m: Metric) -> f32 {
        m.sigmas()[style.metric_index()] * self.em(style)
    }

    pub(super) fn rule(&self, style: Style) -> f32 {
        0.9f32.max(self.metric(style, Metric::DefaultRuleThickness))
    }
}
