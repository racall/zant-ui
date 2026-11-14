//! TeX box layout, ported from KaTeX's HTML builder (`buildHTML.js`,
//! `functions/{supsub,genfrac,sqrt,op,delimsizing,accent,…}.js`,
//! `environments/array.js`) and its font metrics (`fontMetrics.js`).
//!
//! Every box is measured in pixels with its origin on the baseline at the
//! left edge; y grows downwards. Glyphs are drawn with STIX Two Math.
//!
//! The public entry point ([`Options`] + [`layout`]) lives at the end of this
//! file; the engine's own options are [`EngineOptions`]. The `ui` wrappers
//! (`math::block`, `math::render`) are in `math/block.rs`.

use crate::core::hash::{HashValue, Wyhash};
pub use crate::core::vector::Drawing;
use crate::core::vector::{self, Command, Point, Rect as VectorRect, Text as VectorText};
use crate::math::{fonts, parser, symbols};
use crate::renderer::text::Family;

use parser::Node;
use symbols::Class;

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

    fn tight(self) -> bool {
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
enum Metric {
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

// ── spacing (KaTeX spacingData.js, in mu) ─────────────────────────────────

fn spacing_mu(left: Class, right: Class, tight: bool) -> f32 {
    if tight {
        return match left {
            Class::Ord | Class::Close | Class::Inner => {
                if right == Class::Op {
                    3.0
                } else {
                    0.0
                }
            }
            Class::Op => {
                if right == Class::Ord || right == Class::Op {
                    3.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
    }
    let thin: f32 = 3.0;
    let med: f32 = 4.0;
    let thick: f32 = 5.0;
    match left {
        Class::Ord => match right {
            Class::Op | Class::Inner => thin,
            Class::Bin => med,
            Class::Rel => thick,
            _ => 0.0,
        },
        Class::Op => match right {
            Class::Ord | Class::Op | Class::Inner => thin,
            Class::Rel => thick,
            _ => 0.0,
        },
        Class::Bin => match right {
            Class::Ord | Class::Op | Class::Open | Class::Inner => med,
            _ => 0.0,
        },
        Class::Rel => match right {
            Class::Ord | Class::Op | Class::Open | Class::Inner => thick,
            _ => 0.0,
        },
        Class::Open => 0.0,
        Class::Close => match right {
            Class::Op | Class::Inner => thin,
            Class::Bin => med,
            Class::Rel => thick,
            _ => 0.0,
        },
        Class::Punct => match right {
            Class::Rel => thick,
            Class::Bin => 0.0,
            _ => thin,
        },
        Class::Inner => match right {
            Class::Ord | Class::Op | Class::Open | Class::Punct | Class::Inner => thin,
            Class::Bin => med,
            Class::Rel => thick,
            _ => 0.0,
        },
    }
}

// ── boxes ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Box {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub cmds: Vec<Command>,
    pub class: Class,
    /// Single glyph (TeX "character box"): scripts attach without drops.
    pub is_char: bool,
    /// Italic correction (scripts after slanted glyphs move right).
    pub italic: f32,
    /// Spaces do not take part in inter-atom spacing.
    pub is_space: bool,
    /// `\tag` content, carried to the top level.
    pub is_tag: bool,
}

impl Default for Box {
    fn default() -> Box {
        Box {
            width: 0.0,
            height: 0.0,
            depth: 0.0,
            cmds: Vec::new(),
            class: Class::Ord,
            is_char: false,
            italic: 0.0,
            is_space: false,
            is_tag: false,
        }
    }
}

/// The layout engine's options.
#[derive(Clone, Copy, Debug)]
pub struct EngineOptions {
    /// Pixel size of 1em in text style.
    pub size: f32,
    pub color: u32,
    pub error_color: u32,
}

pub struct Engine {
    pub options: EngineOptions,
    pub color: u32,
    pub tag: Option<Box>,
    /// Delimiter height for `\middle` inside the current `\left…\right`.
    pub middle_total: Option<f32>,
}

impl Engine {
    pub fn init(options: EngineOptions) -> Engine {
        Engine {
            options,
            color: options.color,
            tag: None,
            middle_total: None,
        }
    }

    fn em(&self, style: Style) -> f32 {
        self.options.size * style.multiplier()
    }

    fn metric(&self, style: Style, m: Metric) -> f32 {
        m.sigmas()[style.metric_index()] * self.em(style)
    }

    fn rule(&self, style: Style) -> f32 {
        0.9f32.max(self.metric(style, Metric::DefaultRuleThickness))
    }

    // ── primitives ──────────────────────────────────────────────────────

    fn glyph(&self, text: &str, size: f32, class: Class, italic: bool) -> Box {
        let m = vector::measure(text, size, Family::Math, 400, false);
        let cmds = vec![Command::Text(VectorText {
            x: 0.0,
            y: 0.0,
            text: crate::Str::copy(text),
            size,
            family: Family::Math,
            weight: 400,
            italic: false,
            color: self.color,
            scale_y: 1.0,
        })];
        Box {
            width: m.advance,
            height: m.ascent,
            depth: m.descent,
            cmds,
            class,
            is_char: true,
            italic: if italic { 0.04 * size } else { 0.0 },
            ..Box::default()
        }
    }

    fn text_box(&self, text: &str, size: f32, color: u32) -> Box {
        let m = vector::measure(text, size, Family::Math, 400, false);
        let cmds = vec![Command::Text(VectorText {
            x: 0.0,
            y: 0.0,
            text: crate::Str::copy(text),
            size,
            family: Family::Math,
            weight: 400,
            italic: false,
            color,
            scale_y: 1.0,
        })];
        // text keeps the font's line extent so mixed rows align
        Box {
            width: m.advance,
            height: m.ascent.max(0.7 * size),
            depth: m.descent.max(0.2 * size),
            cmds,
            ..Box::default()
        }
    }

    fn translated(&self, box_: &Box, dx: f32, dy: f32, out: &mut Vec<Command>) {
        for cmd in &box_.cmds {
            out.push(vector::translate(cmd, dx, dy));
        }
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Command>) {
        out.push(Command::Rect(VectorRect {
            x,
            y,
            width: w,
            height: h,
            color: self.color,
            radius: 0.0,
        }));
    }

    fn stroke(&self, points: &[Point], width: f32, out: &mut Vec<Command>) {
        out.push(Command::Path(vector::Path {
            points: points.to_vec(),
            color: self.color,
            stroke_width: width,
            closed: false,
        }));
    }

    /// Kept as a drawing primitive even though no layout path fills a polygon.
    #[allow(dead_code)]
    fn fill(&self, points: &[Point], out: &mut Vec<Command>) {
        out.push(Command::Path(vector::Path {
            points: points.to_vec(),
            color: self.color,
            stroke_width: 0.0,
            closed: false,
        }));
    }

    // ── math lists ──────────────────────────────────────────────────────

    pub fn list(&mut self, nodes: &[Node], style: Style) -> Box {
        let mut boxes: Vec<Box> = Vec::new();
        let mut current = style;
        for node in nodes {
            if let Node::Style { style: sw, body } = node {
                // \displaystyle etc. switch the rest of the group
                current = match *sw {
                    parser::StyleSwitch::Display => Style::DISPLAY,
                    parser::StyleSwitch::Text => Style::TEXT,
                    parser::StyleSwitch::Script => Style::new(Size::Script),
                    parser::StyleSwitch::ScriptScript => Style::new(Size::ScriptScript),
                };
                boxes.push(self.list(body, current));
                continue;
            }
            if let Node::Tag(tag) = node {
                let text = format!("({tag})");
                self.tag = Some(self.text_box(&text, self.em(Style::TEXT), self.color));
                continue;
            }
            boxes.push(self.build(node, current));
        }
        self.hlist(&mut boxes, style)
    }

    /// Joins boxes horizontally with TeX inter-atom spacing.
    fn hlist(&self, boxes: &mut [Box], style: Style) -> Box {
        // bin → ord conversions (KaTeX buildExpression)
        let mut prev_class: Option<Class> = None;
        for i in 0..boxes.len() {
            if boxes[i].is_space {
                continue;
            }
            if boxes[i].class == Class::Bin {
                let mut next_class: Option<Class> = None;
                let mut j = i + 1;
                while j < boxes.len() {
                    if !boxes[j].is_space {
                        next_class = Some(boxes[j].class);
                        break;
                    }
                    j += 1;
                }
                let left_cancel = match prev_class {
                    None => true,
                    Some(prev) => matches!(
                        prev,
                        Class::Bin | Class::Op | Class::Rel | Class::Open | Class::Punct
                    ),
                };
                let right_cancel =
                    matches!(next_class, Some(Class::Rel | Class::Close | Class::Punct));
                if left_cancel || right_cancel {
                    boxes[i].class = Class::Ord;
                }
            }
            prev_class = Some(boxes[i].class);
        }
        let mut out: Vec<Command> = Vec::new();
        let mut x: f32 = 0.0;
        let mut height: f32 = 0.0;
        let mut depth: f32 = 0.0;
        let mut last: Option<Class> = None;
        let mu = self.em(style) / 18.0;
        for box_ in boxes.iter() {
            if !box_.is_space {
                if let Some(l) = last {
                    x += spacing_mu(l, box_.class, style.tight()) * mu;
                }
                last = Some(box_.class);
            }
            self.translated(box_, x, 0.0, &mut out);
            x += box_.width;
            height = height.max(box_.height);
            depth = depth.max(box_.depth);
        }
        let single = boxes.len() == 1;
        let is_char = single && boxes[0].is_char;
        Box {
            width: x,
            height,
            depth,
            cmds: out,
            class: Class::Ord,
            is_char,
            italic: if single { boxes[0].italic } else { 0.0 },
            ..Box::default()
        }
    }

    fn build(&mut self, node: &Node, style: Style) -> Box {
        match node {
            Node::Atom(a) => self.atom(a, style),
            Node::Group(g) => {
                let mut b = self.list(g, style);
                b.class = Class::Ord;
                b
            }
            Node::SupSub(s) => self.supsub(s, style),
            Node::Frac(f) => self.frac(f, style),
            Node::Sqrt(s) => self.sqrt(s, style),
            Node::LeftRight(lr) => self.left_right(lr, style),
            Node::SizedDelim(d) => self.sized_delim(d, style),
            Node::Accent(a) => self.accent(a, style),
            Node::OverUnder(o) => self.over_under(o, style),
            Node::Op(o) => self.op(o, style),
            Node::Text(t) => self.text_node(t, style),
            Node::Space(s) => kern(*s * self.em(style)),
            Node::Array(a) => self.array(a, style),
            Node::Color(c) => {
                let saved = self.color;
                self.color = c.color;
                let b = self.list(&c.body, style);
                self.color = saved;
                b
            }
            Node::Style { body, .. } => self.list(body, style),
            Node::Phantom {
                body,
                horizontal,
                vertical,
            } => {
                let inner = self.list(body, style);
                Box {
                    width: if *horizontal { inner.width } else { 0.0 },
                    height: if *vertical { inner.height } else { 0.0 },
                    depth: if *vertical { inner.depth } else { 0.0 },
                    ..Box::default()
                }
            }
            Node::Boxed(b) => self.boxed(b, style),
            Node::Cancel { body, kind } => self.cancel(body, *kind, style),
            Node::Stack { base, over, under } => {
                self.stack(base, over.as_deref(), under.as_deref(), style)
            }
            Node::Tag(_) => kern(0.0),
            Node::XArrow(x) => self.x_arrow(x, style),
            Node::Sized { factor, body } => {
                let saved = self.options.size;
                self.options.size = saved * *factor;
                let b = self.list(body, style);
                self.options.size = saved;
                b
            }
            Node::ColorBox {
                background,
                frame,
                body,
            } => self.color_box(*background, *frame, body, style),
            Node::Lap { side, body } => {
                let inner = self.list(body, style);
                let mut out: Vec<Command> = Vec::new();
                let dx: f32 = match *side {
                    parser::LapSide::Right => 0.0,
                    parser::LapSide::Left => -inner.width,
                    parser::LapSide::Center => -inner.width / 2.0,
                };
                self.translated(&inner, dx, 0.0, &mut out);
                Box {
                    width: 0.0,
                    height: inner.height,
                    depth: inner.depth,
                    cmds: out,
                    ..Box::default()
                }
            }
            Node::Raise { amount, body } => {
                let inner = self.list(body, style);
                let dy = *amount * self.em(style);
                let mut out: Vec<Command> = Vec::new();
                self.translated(&inner, 0.0, -dy, &mut out);
                Box {
                    width: inner.width,
                    height: (inner.height + dy).max(0.0),
                    depth: (inner.depth - dy).max(0.0),
                    cmds: out,
                    ..Box::default()
                }
            }
            Node::Choice(choices) => {
                let index = match style.size {
                    Size::Display => 0,
                    Size::Text => 1,
                    Size::Script => 2,
                    Size::ScriptScript => 3,
                };
                self.list(&choices[index], style)
            }
            Node::Smash(body) => {
                let mut inner = self.list(body, style);
                inner.height = 0.0;
                inner.depth = 0.0;
                inner
            }
            Node::RuleBox { width, height } => {
                let mut out: Vec<Command> = Vec::new();
                let w = *width * self.em(style);
                let h = *height * self.em(style);
                self.rect(0.0, -h, w, h, &mut out);
                Box {
                    width: w,
                    height: h,
                    depth: 0.0,
                    cmds: out,
                    ..Box::default()
                }
            }
            Node::Unknown(name) => {
                let saved = self.color;
                self.color = self.options.error_color;
                let text = format!("\\{name}");
                let b = self.text_box(&text, self.em(style), self.options.error_color);
                self.color = saved;
                b
            }
        }
    }

    fn atom(&self, a: &parser::Atom, style: Style) -> Box {
        let mapped = fonts::map_text(a.variant, &a.text);
        let italic = a.variant == fonts::Variant::Normal && mapped != a.text;
        self.glyph(&mapped, self.em(style), a.class, italic)
    }

    fn text_node(&self, t: &parser::Text, style: Style) -> Box {
        let variant: fonts::Variant = if t.bold {
            fonts::Variant::Bold
        } else if t.variant == fonts::Variant::Roman {
            fonts::Variant::Roman
        } else {
            t.variant
        };
        let mapped = fonts::map_text(variant, &t.text);
        let mut b = self.text_box(&mapped, self.em(style), self.color);
        b.class = Class::Ord;
        b
    }

    // ── supsub (KaTeX functions/supsub.js) ──────────────────────────────

    fn supsub(&mut self, s: &parser::SupSub, style: Style) -> Box {
        // operators with limits and braces with labels stack their scripts
        if let Some(base_node) = &s.base {
            match &**base_node {
                Node::Op(o) => {
                    let limits = o.forced.unwrap_or(o.limits && style.size == Size::Display);
                    if limits {
                        return self.op_limits(o, s, style);
                    }
                }
                Node::OverUnder(ou) => {
                    if matches!(
                        ou.kind,
                        parser::OverUnderKind::Overbrace | parser::OverUnderKind::Underbrace
                    ) {
                        return self.brace_label(ou, s, style);
                    }
                }
                _ => {}
            }
        }
        let base: Box = match &s.base {
            Some(b) => self.build(b, style),
            None => Box::default(),
        };
        let sup_style = style.sup();
        let sub_style = style.sub();
        let sup_box: Option<Box> = s.sup.as_ref().map(|n| self.build(n, sup_style));
        let sub_box: Option<Box> = s.sub.as_ref().map(|n| self.build(n, sub_style));

        let mut sup_shift: f32 = 0.0;
        let mut sub_shift: f32 = 0.0;
        if !base.is_char {
            sup_shift = base.height - self.metric(sup_style, Metric::SupDrop);
            sub_shift = base.depth + self.metric(sub_style, Metric::SubDrop);
        }
        let min_sup_shift = if style.size == Size::Display {
            self.metric(style, Metric::Sup1)
        } else if style.cramped {
            self.metric(style, Metric::Sup3)
        } else {
            self.metric(style, Metric::Sup2)
        };
        let x_height = self.metric(style, Metric::XHeight);
        if let (Some(sp), Some(sb)) = (&sup_box, &sub_box) {
            sup_shift = sup_shift.max(min_sup_shift).max(sp.depth + 0.25 * x_height);
            sub_shift = sub_shift.max(self.metric(style, Metric::Sub2));
            let rule_width = self.metric(style, Metric::DefaultRuleThickness);
            let max_width = 4.0 * rule_width;
            if (sup_shift - sp.depth) - (sb.height - sub_shift) < max_width {
                sub_shift = max_width - (sup_shift - sp.depth) + sb.height;
                let psi = 0.8 * x_height - (sup_shift - sp.depth);
                if psi > 0.0 {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
        } else if let Some(sb) = &sub_box {
            sub_shift = sub_shift
                .max(self.metric(style, Metric::Sub1))
                .max(sb.height - 0.8 * x_height);
        } else if let Some(sp) = &sup_box {
            sup_shift = sup_shift.max(min_sup_shift).max(sp.depth + 0.25 * x_height);
        }

        let mut out: Vec<Command> = Vec::new();
        self.translated(&base, 0.0, 0.0, &mut out);
        let script_space = 0.05 * self.options.size;
        let mut width = base.width;
        let mut height = base.height;
        let mut depth = base.depth;
        let mut scripts_width: f32 = 0.0;
        if let Some(sp) = &sup_box {
            self.translated(sp, base.width + base.italic, -sup_shift, &mut out);
            scripts_width = scripts_width.max(sp.width + base.italic);
            height = height.max(sup_shift + sp.height);
            depth = depth.max(sp.depth - sup_shift);
        }
        if let Some(sb) = &sub_box {
            self.translated(sb, base.width, sub_shift, &mut out);
            scripts_width = scripts_width.max(sb.width);
            depth = depth.max(sub_shift + sb.depth);
            height = height.max(sb.height - sub_shift);
        }
        width += scripts_width + script_space;
        Box {
            width,
            height,
            depth,
            cmds: out,
            class: base.class,
            ..Box::default()
        }
    }

    // ── genfrac (KaTeX functions/genfrac.js) ────────────────────────────

    fn frac(&mut self, f: &parser::Frac, outer: Style) -> Box {
        let style: Style = match f.style {
            Some(parser::StyleSwitch::Display) => Style::DISPLAY,
            Some(parser::StyleSwitch::Text) => Style::TEXT,
            Some(parser::StyleSwitch::Script) => Style::new(Size::Script),
            Some(parser::StyleSwitch::ScriptScript) => Style::new(Size::ScriptScript),
            None => outer,
        };
        let num_style = if f.continued {
            Style::DISPLAY
        } else {
            style.frac_num()
        };
        let num = self.list(&f.num, num_style);
        let den = self.list(&f.den, style.frac_den());
        let rule_width: f32 = if f.bar { self.rule(style) } else { 0.0 };
        let rule_spacing = self.metric(style, Metric::DefaultRuleThickness);
        let mut num_shift: f32;
        let clearance: f32;
        let mut den_shift: f32;
        if style.size == Size::Display {
            num_shift = self.metric(style, Metric::Num1);
            clearance = if rule_width > 0.0 {
                3.0 * rule_spacing
            } else {
                7.0 * rule_spacing
            };
            den_shift = self.metric(style, Metric::Denom1);
        } else {
            if rule_width > 0.0 {
                num_shift = self.metric(style, Metric::Num2);
                clearance = rule_spacing;
            } else {
                num_shift = self.metric(style, Metric::Num3);
                clearance = 3.0 * rule_spacing;
            }
            den_shift = self.metric(style, Metric::Denom2);
        }
        let axis = self.metric(style, Metric::AxisHeight);
        if rule_width == 0.0 {
            let candidate = (num_shift - num.depth) - (den.height - den_shift);
            if candidate < clearance {
                num_shift += 0.5 * (clearance - candidate);
                den_shift += 0.5 * (clearance - candidate);
            }
        } else {
            let above = (num_shift - num.depth) - (axis + 0.5 * rule_width);
            if above < clearance {
                num_shift += clearance - above;
            }
            let below = (axis - 0.5 * rule_width) - (den.height - den_shift);
            if below < clearance {
                den_shift += clearance - below;
            }
        }
        let null_delim = 0.12 * self.em(style);
        let inner_w = num.width.max(den.width);
        let mut out: Vec<Command> = Vec::new();
        self.translated(
            &num,
            null_delim + (inner_w - num.width) / 2.0,
            -num_shift,
            &mut out,
        );
        self.translated(
            &den,
            null_delim + (inner_w - den.width) / 2.0,
            den_shift,
            &mut out,
        );
        if rule_width > 0.0 {
            self.rect(
                null_delim,
                -axis - rule_width / 2.0,
                inner_w,
                rule_width,
                &mut out,
            );
        }
        let mut box_ = Box {
            width: inner_w + 2.0 * null_delim,
            height: num_shift + num.height,
            depth: den_shift + den.depth,
            cmds: out,
            class: Class::Ord,
            ..Box::default()
        };
        if !f.left.is_empty() || !f.right.is_empty() {
            let delim_size = if style.size == Size::Display {
                self.metric(style, Metric::Delim1)
            } else {
                self.metric(style, Metric::Delim2)
            };
            box_ = self.wrap_delims(box_, &f.left, &f.right, style, delim_size);
            box_.class = Class::Ord;
        }
        box_
    }

    // ── delimiters (KaTeX delimiter.js) ─────────────────────────────────

    /// A delimiter glyph with total height `total`, centred on the axis.
    fn delimiter(&self, delim: &str, total: f32, style: Style, class: Class) -> Box {
        let size = self.em(style);
        if delim.is_empty() {
            return Box {
                width: 0.12 * size,
                class,
                is_space: false,
                ..Box::default()
            };
        }
        let base = vector::measure(delim, size, Family::Math, 400, false);
        let ink = (base.ascent + base.descent).max(0.01);
        let mut font_size = size;
        let mut scale_y: f32 = 1.0;
        if total > ink {
            let ratio = total / ink;
            let grow = ratio.min(1.35);
            font_size = size * grow;
            scale_y = ratio / grow;
        }
        let m = vector::measure(delim, font_size, Family::Math, 400, false);
        let asc = m.ascent * scale_y;
        let desc = m.descent * scale_y;
        let axis = self.metric(style, Metric::AxisHeight);
        // ink centre on the math axis
        let baseline = -axis + (asc - desc) / 2.0;
        let cmds = vec![Command::Text(VectorText {
            x: 0.0,
            y: baseline,
            text: crate::Str::copy(delim),
            size: font_size,
            family: Family::Math,
            weight: 400,
            italic: false,
            color: self.color,
            scale_y,
        })];
        Box {
            width: m.advance,
            height: asc - baseline,
            depth: desc + baseline,
            cmds,
            class,
            ..Box::default()
        }
    }

    /// Size of `\left`/`\right` around content (KaTeX makeLeftRightDelim).
    fn left_right_total(&self, height: f32, depth: f32, style: Style) -> f32 {
        let axis = self.metric(style, Metric::AxisHeight);
        let delimiter_factor: f32 = 901.0;
        let delimiter_extend = 0.5 * self.em(style);
        let max_dist = (height - axis).max(depth + axis);
        (max_dist / 500.0 * delimiter_factor).max(2.0 * max_dist - delimiter_extend)
    }

    fn wrap_delims(&self, inner: Box, left: &str, right: &str, style: Style, total: f32) -> Box {
        let mut parts = [
            self.delimiter(left, total, style, Class::Open),
            inner,
            self.delimiter(right, total, style, Class::Close),
        ];
        parts[1].class = Class::Inner;
        let mut out: Vec<Command> = Vec::new();
        let mut x: f32 = 0.0;
        let mut h: f32 = 0.0;
        let mut d: f32 = 0.0;
        for p in &parts {
            self.translated(p, x, 0.0, &mut out);
            x += p.width;
            h = h.max(p.height);
            d = d.max(p.depth);
        }
        Box {
            width: x,
            height: h,
            depth: d,
            cmds: out,
            class: Class::Inner,
            ..Box::default()
        }
    }

    fn left_right(&mut self, lr: &parser::LeftRight, style: Style) -> Box {
        // first pass: content size decides delimiter size (also for \middle)
        let mut inner = self.list(&lr.body, style);
        let total = self.left_right_total(inner.height, inner.depth, style);
        let mut has_middle = false;
        for n in &lr.body {
            if let Node::SizedDelim(d) = n
                && d.size == 0
            {
                has_middle = true;
            }
        }
        if has_middle {
            self.middle_total = Some(total);
            inner = self.list(&lr.body, style);
            self.middle_total = None;
        }
        self.wrap_delims(inner, &lr.left, &lr.right, style, total)
    }

    fn sized_delim(&self, d: &parser::SizedDelim, style: Style) -> Box {
        let heights = [0.0f32, 1.2, 1.8, 2.4, 3.0];
        let total = if d.size == 0 {
            self.middle_total.unwrap_or(self.em(style))
        } else {
            heights[usize::from(d.size)] * self.options.size
        };
        self.delimiter(&d.delim, total, style, d.class)
    }

    // ── sqrt (KaTeX functions/sqrt.js + delimiter.makeSqrtImage) ────────

    fn sqrt(&mut self, s: &parser::Sqrt, style: Style) -> Box {
        let mut inner = self.list(&s.body, style.cramp());
        let em_size = self.em(style);
        if inner.height == 0.0 {
            inner.height = self.metric(style, Metric::XHeight);
        }
        let theta = self.rule(style);
        let phi = if style.size == Size::Display {
            self.metric(style, Metric::XHeight)
        } else {
            theta
        };
        let mut line_clearance = theta + phi / 4.0;
        let min_height = inner.height + inner.depth + line_clearance + theta;
        let total = min_height.max(1.0 * em_size);
        let delim_depth = total - theta;
        if delim_depth > inner.height + inner.depth + line_clearance {
            line_clearance = (line_clearance + delim_depth - inner.height - inner.depth) / 2.0;
        }
        let top = -(inner.height + line_clearance + theta);
        let bottom = top + total;
        let surd_w = if total > 1.8 * em_size {
            1.0 * em_size
        } else {
            0.833 * em_size
        };
        let small = total.min(1.0 * em_size);

        let mut out: Vec<Command> = Vec::new();
        let mut x0: f32 = 0.0;
        let mut index_box: Option<Box> = None;
        if let Some(idx) = &s.index {
            let ib = self.list(idx, Style::new(Size::ScriptScript));
            let anchor = 0.55 * surd_w;
            if ib.width > anchor {
                x0 = ib.width - anchor;
            }
            index_box = Some(ib);
        }
        // surd: left tick, heavy down-stroke, thin up-stroke to the vinculum
        let ax = x0 + 0.06 * surd_w;
        let ay = bottom - 0.42 * small;
        let bx = x0 + 0.22 * surd_w;
        let by = bottom - 0.52 * small;
        let cx = x0 + 0.50 * surd_w;
        let cy = bottom;
        let dx = x0 + surd_w;
        let dy = top + theta / 2.0;
        self.stroke(&[[ax, ay], [bx, by]], theta * 1.1, &mut out);
        self.stroke(&[[bx, by], [cx, cy]], theta * 2.4, &mut out);
        self.stroke(&[[cx, cy], [dx, dy]], theta * 1.1, &mut out);
        let body_x = dx + 0.06 * em_size;
        self.rect(
            dx - theta * 0.3,
            top,
            body_x - dx + inner.width + 0.06 * em_size + theta * 0.3,
            theta,
            &mut out,
        );
        self.translated(&inner, body_x, 0.0, &mut out);
        let mut height = -top;
        if let Some(ib) = &index_box {
            let raise = bottom - 0.6 * total;
            self.translated(
                ib,
                x0 + 0.55 * surd_w - ib.width,
                raise - ib.depth,
                &mut out,
            );
            height = height.max(-(raise - ib.depth) + ib.height);
        }
        Box {
            width: body_x + inner.width + 0.06 * em_size,
            height,
            depth: bottom.max(inner.depth),
            cmds: out,
            ..Box::default()
        }
    }

    // ── big operators (KaTeX functions/op.js, assembleSupSub) ───────────

    fn op_box(&self, o: &parser::Op, style: Style) -> Box {
        let size = self.em(style);
        if !o.symbol {
            let mut b = self.text_box(&o.text, size, self.color);
            b.class = Class::Op;
            return b;
        }
        // KaTeX draws display operators from the Size2 font (∑ ≈ 1.45×, ∫ ≈ 2×)
        let factor: f32 = if style.size == Size::Display {
            if o.integral { 2.0 } else { 1.45 }
        } else if o.integral {
            1.15
        } else {
            1.0
        };
        let font_size = size * factor;
        let m = vector::measure(&o.text, font_size, Family::Math, 400, false);
        let axis = self.metric(style, Metric::AxisHeight);
        let baseline = -axis + (m.ascent - m.descent) / 2.0;
        let cmds = vec![Command::Text(VectorText {
            x: 0.0,
            y: baseline,
            text: crate::Str::copy(&o.text),
            size: font_size,
            family: Family::Math,
            weight: 400,
            italic: false,
            color: self.color,
            scale_y: 1.0,
        })];
        Box {
            width: m.advance,
            height: m.ascent - baseline,
            depth: m.descent + baseline,
            cmds,
            class: Class::Op,
            italic: if o.integral { 0.3 * font_size } else { 0.0 },
            ..Box::default()
        }
    }

    fn op(&self, o: &parser::Op, style: Style) -> Box {
        self.op_box(o, style)
    }

    fn op_limits(&mut self, o: &parser::Op, s: &parser::SupSub, style: Style) -> Box {
        let base = self.op_box(o, style);
        let sup: Option<Box> = s.sup.as_ref().map(|n| self.build(n, style.sup()));
        let sub: Option<Box> = s.sub.as_ref().map(|n| self.build(n, style.sub()));
        let slant = base.italic;
        self.assemble_limits(base, sup, sub, style, slant)
    }

    fn assemble_limits(
        &self,
        base: Box,
        sup: Option<Box>,
        sub: Option<Box>,
        style: Style,
        slant: f32,
    ) -> Box {
        let mut width = base.width;
        if let Some(b) = &sup {
            width = width.max(b.width);
        }
        if let Some(b) = &sub {
            width = width.max(b.width);
        }
        let mut out: Vec<Command> = Vec::new();
        self.translated(&base, (width - base.width) / 2.0, 0.0, &mut out);
        let mut height = base.height;
        let mut depth = base.depth;
        let spacing5 = self.metric(style, Metric::BigOpSpacing5);
        if let Some(b) = &sup {
            let kern_sup = self
                .metric(style, Metric::BigOpSpacing1)
                .max(self.metric(style, Metric::BigOpSpacing3) - b.depth);
            let baseline = -(base.height + kern_sup + b.depth);
            self.translated(b, (width - b.width) / 2.0 + slant / 2.0, baseline, &mut out);
            height = base.height + kern_sup + b.depth + b.height + spacing5;
        }
        if let Some(b) = &sub {
            let kern_sub = self
                .metric(style, Metric::BigOpSpacing2)
                .max(self.metric(style, Metric::BigOpSpacing4) - b.height);
            let baseline = base.depth + kern_sub + b.height;
            self.translated(b, (width - b.width) / 2.0 - slant / 2.0, baseline, &mut out);
            depth = base.depth + kern_sub + b.height + b.depth + spacing5;
        }
        Box {
            width,
            height,
            depth,
            cmds: out,
            class: Class::Op,
            ..Box::default()
        }
    }

    fn stack(
        &mut self,
        base_node: &Node,
        over: Option<&Node>,
        under: Option<&Node>,
        style: Style,
    ) -> Box {
        let mut base = self.build(base_node, style);
        base.class = Class::Rel;
        let base_is_char = base.is_char;
        let over: Option<Box> = over.map(|n| self.build(n, style.sup()));
        let under: Option<Box> = under.map(|n| self.build(n, style.sub()));
        let mut b = self.assemble_limits(base, over, under, style, 0.0);
        // \overset/\stackrel: tighter than operator limits, keep base class
        b.class = if base_is_char { Class::Rel } else { Class::Ord };
        if let Node::Group(g) = base_node
            && g.len() == 1
            && let Node::Atom(a) = &g[0]
        {
            b.class = a.class;
        }
        b
    }

    // ── accents and over/under decorations ──────────────────────────────

    fn accent(&mut self, a: &parser::AccentNode, style: Style) -> Box {
        let base = self.list(&a.base, style.cramp());
        let size = self.em(style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        self.translated(&base, 0.0, 0.0, &mut out);
        let clearance = base.height.max(self.metric(style, Metric::XHeight));
        let skew: f32 = if base.is_char {
            base.italic + 0.03 * size
        } else {
            0.0
        };
        let height: f32;
        let name = a.name.as_str();
        if a.wide || (base.width > 1.2 * size && name != "vec") {
            // stretchy accents drawn as paths over the whole base
            let w = base.width;
            let y0 = -(clearance + 0.08 * size);
            let h = 0.2 * size;
            if name == "widehat" || name == "hat" {
                self.stroke(
                    &[[0.02 * w, y0], [w / 2.0, y0 - h], [0.98 * w, y0]],
                    theta * 1.2,
                    &mut out,
                );
            } else if name == "widecheck" || name == "check" {
                self.stroke(
                    &[[0.02 * w, y0 - h], [w / 2.0, y0], [0.98 * w, y0 - h]],
                    theta * 1.2,
                    &mut out,
                );
            } else if name == "bar" {
                self.rect(0.0, y0 - theta, w, theta, &mut out);
            } else {
                // tilde wave
                let mut pts = [[0.0f32; 2]; 17];
                for (i, p) in pts.iter_mut().enumerate() {
                    let t = i as f32 / 16.0;
                    *p = [
                        t * w,
                        y0 - h / 2.0 - (t * std::f32::consts::TAU).sin() * h / 2.5,
                    ];
                }
                self.stroke(&pts, theta * 1.2, &mut out);
            }
            height = clearance + 0.08 * size + h + theta;
        } else if name == "vec" {
            let w = base.width.max(0.5 * size);
            let y = -(clearance + 0.16 * size);
            let head = 0.1 * size;
            let x0 = (base.width - w) / 2.0 + skew;
            self.stroke(&[[x0, y], [x0 + w, y]], theta, &mut out);
            self.stroke(
                &[
                    [x0 + w - head, y - head * 0.7],
                    [x0 + w, y],
                    [x0 + w - head, y + head * 0.7],
                ],
                theta,
                &mut out,
            );
            height = clearance + 0.16 * size + head;
        } else {
            let glyph_text = accent_glyph(name);
            let m = vector::measure(glyph_text, size, Family::Math, 400, false);
            let gap = 0.06 * size;
            // the accent's ink bottom sits `gap` above the base
            let baseline = -(clearance + gap) - m.ink_bottom;
            let x = (base.width - m.advance) / 2.0 + skew;
            out.push(Command::Text(VectorText {
                x,
                y: baseline,
                text: crate::Str::copy(glyph_text),
                size,
                family: Family::Math,
                weight: 400,
                italic: false,
                color: self.color,
                scale_y: 1.0,
            }));
            height = -(baseline + m.ink_top);
        }
        Box {
            width: base.width,
            height: height.max(base.height),
            depth: base.depth,
            cmds: out,
            class: Class::Ord,
            is_char: base.is_char,
            italic: base.italic,
            ..Box::default()
        }
    }

    fn brace(
        &self,
        w: f32,
        y_edge: f32,
        depth_dir: f32,
        theta: f32,
        size: f32,
        out: &mut Vec<Command>,
    ) {
        // depth_dir = -1: opening downward (overbrace), +1: upward (underbrace)
        let h = 0.32 * size;
        let r = (0.22 * size).min(w / 4.0);
        let mid = w / 2.0;
        let ym = y_edge + depth_dir * h * 0.5;
        let tip = y_edge + depth_dir * h;
        self.stroke(
            &[
                [0.0, y_edge],
                [r * 0.35, ym - depth_dir * h * 0.12],
                [r, ym],
                [mid - r, ym],
                [mid - r * 0.35, ym + depth_dir * h * 0.12],
                [mid, tip],
                [mid + r * 0.35, ym + depth_dir * h * 0.12],
                [mid + r, ym],
                [w - r, ym],
                [w - r * 0.35, ym - depth_dir * h * 0.12],
                [w, y_edge],
            ],
            theta * 1.4,
            out,
        );
    }

    fn over_under(&mut self, o: &parser::OverUnder, style: Style) -> Box {
        let body_style = if o.kind == parser::OverUnderKind::Overline {
            style.cramp()
        } else {
            style
        };
        let base = self.list(&o.body, body_style);
        let theta = self.rule(style);
        let size = self.em(style);
        let mut out: Vec<Command> = Vec::new();
        self.translated(&base, 0.0, 0.0, &mut out);
        let mut height = base.height;
        let mut depth = base.depth;
        let w = base.width;
        match o.kind {
            parser::OverUnderKind::Overline => {
                self.rect(
                    0.0,
                    -(base.height + 3.0 * theta + theta),
                    w,
                    theta,
                    &mut out,
                );
                height = base.height + 5.0 * theta;
            }
            parser::OverUnderKind::Underline => {
                self.rect(0.0, base.depth + 3.0 * theta, w, theta, &mut out);
                depth = base.depth + 5.0 * theta;
            }
            parser::OverUnderKind::Overbrace => {
                let y = -(base.height + 0.1 * size);
                self.brace(w, y, -1.0, theta, size, &mut out);
                height = base.height + 0.1 * size + 0.32 * size + theta;
            }
            parser::OverUnderKind::Underbrace => {
                let y = base.depth + 0.1 * size;
                self.brace(w, y, 1.0, theta, size, &mut out);
                depth = base.depth + 0.1 * size + 0.32 * size + theta;
            }
            parser::OverUnderKind::Overrightarrow
            | parser::OverUnderKind::Overleftarrow
            | parser::OverUnderKind::Overleftrightarrow
            | parser::OverUnderKind::Underrightarrow
            | parser::OverUnderKind::Underleftarrow => {
                let over = matches!(
                    o.kind,
                    parser::OverUnderKind::Overrightarrow
                        | parser::OverUnderKind::Overleftarrow
                        | parser::OverUnderKind::Overleftrightarrow
                );
                let y = if over {
                    -(base.height + 0.18 * size)
                } else {
                    base.depth + 0.18 * size
                };
                let head = 0.12 * size;
                self.stroke(&[[0.0, y], [w, y]], theta, &mut out);
                if o.kind != parser::OverUnderKind::Overleftarrow
                    && o.kind != parser::OverUnderKind::Underleftarrow
                {
                    self.stroke(
                        &[
                            [w - head, y - head * 0.7],
                            [w, y],
                            [w - head, y + head * 0.7],
                        ],
                        theta,
                        &mut out,
                    );
                }
                if o.kind == parser::OverUnderKind::Overleftarrow
                    || o.kind == parser::OverUnderKind::Underleftarrow
                    || o.kind == parser::OverUnderKind::Overleftrightarrow
                {
                    self.stroke(
                        &[[head, y - head * 0.7], [0.0, y], [head, y + head * 0.7]],
                        theta,
                        &mut out,
                    );
                }
                if over {
                    height = base.height + 0.18 * size + head;
                } else {
                    depth = base.depth + 0.18 * size + head;
                }
            }
        }
        Box {
            width: w,
            height,
            depth,
            cmds: out,
            class: Class::Ord,
            ..Box::default()
        }
    }

    /// `\xrightarrow[below]{above}` (KaTeX functions/arrow.js + stretchy.js):
    /// labels in script style padded 0.5em each side, the arrow at least
    /// its minimum width and centred on the axis, labels 0.111em from it.
    fn x_arrow(&mut self, x: &parser::XArrow, style: Style) -> Box {
        let size = self.em(style);
        let theta = self.rule(style);
        let above = self.list(&x.above, style.sup());
        let below: Option<Box> = x.below.as_ref().map(|b| self.list(b, style.sub()));
        let pad_above = 0.5 * self.em(style.sup());
        let pad_below = 0.5 * self.em(style.sub());
        let min_w: f32 = match x.kind {
            parser::XArrowKind::Right | parser::XArrowKind::Left => 1.469,
            parser::XArrowKind::DoubleRight | parser::XArrowKind::DoubleLeft => 1.526,
            parser::XArrowKind::LeftRight
            | parser::XArrowKind::DoubleLeftRight
            | parser::XArrowKind::HarpoonsRl
            | parser::XArrowKind::HarpoonsLr
            | parser::XArrowKind::ArrowsRl => 1.75,
            parser::XArrowKind::HookRight | parser::XArrowKind::HookLeft => 1.08,
            parser::XArrowKind::MapsTo => 1.5,
            parser::XArrowKind::LongEqual
            | parser::XArrowKind::TwoHeadRight
            | parser::XArrowKind::TwoHeadLeft => 0.888,
        };
        let mut w = (min_w * size).max(above.width + 2.0 * pad_above);
        if let Some(b) = &below {
            w = w.max(b.width + 2.0 * pad_below);
        }
        // arrow body heights in em (stretchy.js): single 0.522, double 0.56,
        // long equal 0.334, harpoon pairs 0.716
        let body_h: f32 = size
            * match x.kind {
                parser::XArrowKind::DoubleRight
                | parser::XArrowKind::DoubleLeft
                | parser::XArrowKind::DoubleLeftRight => 0.56,
                parser::XArrowKind::LongEqual
                | parser::XArrowKind::TwoHeadRight
                | parser::XArrowKind::TwoHeadLeft => 0.334,
                parser::XArrowKind::HarpoonsRl
                | parser::XArrowKind::HarpoonsLr
                | parser::XArrowKind::ArrowsRl => 0.716,
                _ => 0.522,
            };
        let axis = self.metric(style, Metric::AxisHeight);
        let mut out: Vec<Command> = Vec::new();
        self.draw_x_arrow(x.kind, w, -axis, theta, size, &mut out);
        // labels: baseline shifts as in KaTeX's vlist (negative = up)
        let mut upper_shift = -axis - 0.5 * body_h - 0.111 * size;
        if above.depth > 0.25 * size {
            upper_shift -= above.depth;
        }
        self.translated(&above, (w - above.width) / 2.0, upper_shift, &mut out);
        let mut height = (-upper_shift + above.height).max(axis + body_h / 2.0);
        let mut depth = 0.0f32.max(-axis + body_h / 2.0);
        if let Some(b) = &below {
            let lower_shift = -axis + b.height + 0.5 * body_h + 0.111 * size;
            self.translated(b, (w - b.width) / 2.0, lower_shift, &mut out);
            depth = depth.max(lower_shift + b.depth);
        }
        height = height.max(0.0);
        Box {
            width: w,
            height,
            depth,
            cmds: out,
            class: Class::Rel,
            ..Box::default()
        }
    }

    fn draw_x_arrow(
        &self,
        kind: parser::XArrowKind,
        w: f32,
        y: f32,
        theta: f32,
        size: f32,
        out: &mut Vec<Command>,
    ) {
        use parser::XArrowKind as K;
        let head = 0.14 * size;
        let hy = head * 0.65;
        let gap = 0.09 * size; // between the lines of a double arrow
        match kind {
            K::Right
            | K::Left
            | K::LeftRight
            | K::MapsTo
            | K::HookRight
            | K::HookLeft
            | K::TwoHeadRight
            | K::TwoHeadLeft => {
                self.stroke(&[[0.0, y], [w, y]], theta, out);
                let right_head = matches!(
                    kind,
                    K::Right | K::LeftRight | K::MapsTo | K::HookRight | K::TwoHeadRight
                );
                let left_head =
                    matches!(kind, K::Left | K::LeftRight | K::HookLeft | K::TwoHeadLeft);
                if right_head {
                    self.stroke(
                        &[[w - head, y - hy], [w, y], [w - head, y + hy]],
                        theta,
                        out,
                    );
                }
                if left_head {
                    self.stroke(&[[head, y - hy], [0.0, y], [head, y + hy]], theta, out);
                }
                if kind == K::TwoHeadRight {
                    self.stroke(
                        &[
                            [w - 2.0 * head, y - hy],
                            [w - head, y],
                            [w - 2.0 * head, y + hy],
                        ],
                        theta,
                        out,
                    );
                }
                if kind == K::TwoHeadLeft {
                    self.stroke(
                        &[[2.0 * head, y - hy], [head, y], [2.0 * head, y + hy]],
                        theta,
                        out,
                    );
                }
                if kind == K::MapsTo {
                    self.stroke(&[[0.0, y - hy], [0.0, y + hy]], theta, out);
                }
                if kind == K::HookRight || kind == K::HookLeft {
                    // ↪ / ↩: a half circle curling up from the tail
                    let r = hy;
                    let mut pts = [[0.0f32; 2]; 9];
                    for (i, p) in pts.iter_mut().enumerate() {
                        let t = std::f32::consts::PI * i as f32 / 8.0;
                        *p = if kind == K::HookRight {
                            [r - r * t.sin(), y - r + r * t.cos()]
                        } else {
                            [w - r + r * t.sin(), y - r + r * t.cos()]
                        };
                    }
                    self.stroke(&pts, theta, out);
                }
            }
            K::DoubleRight | K::DoubleLeft | K::DoubleLeftRight => {
                let right = kind != K::DoubleLeft;
                let left = kind != K::DoubleRight;
                let x0: f32 = if left { head * 0.6 } else { 0.0 };
                let x1: f32 = if right { w - head * 0.6 } else { w };
                self.stroke(&[[x0, y - gap / 2.0], [x1, y - gap / 2.0]], theta, out);
                self.stroke(&[[x0, y + gap / 2.0], [x1, y + gap / 2.0]], theta, out);
                if right {
                    self.stroke(
                        &[
                            [w - head * 1.1, y - hy * 1.4],
                            [w, y],
                            [w - head * 1.1, y + hy * 1.4],
                        ],
                        theta,
                        out,
                    );
                }
                if left {
                    self.stroke(
                        &[
                            [head * 1.1, y - hy * 1.4],
                            [0.0, y],
                            [head * 1.1, y + hy * 1.4],
                        ],
                        theta,
                        out,
                    );
                }
            }
            K::LongEqual => {
                self.stroke(&[[0.0, y - gap / 2.0], [w, y - gap / 2.0]], theta, out);
                self.stroke(&[[0.0, y + gap / 2.0], [w, y + gap / 2.0]], theta, out);
            }
            K::HarpoonsRl | K::HarpoonsLr | K::ArrowsRl => {
                // upper line points right, lower points left (⇌ / ⇄)
                let top = y - gap * 1.1;
                let bottom = y + gap * 1.1;
                let upper_right = kind != K::HarpoonsLr;
                self.stroke(&[[0.0, top], [w, top]], theta, out);
                self.stroke(&[[0.0, bottom], [w, bottom]], theta, out);
                if kind == K::ArrowsRl {
                    self.stroke(
                        &[[w - head, top - hy], [w, top], [w - head, top + hy]],
                        theta,
                        out,
                    );
                    self.stroke(
                        &[[head, bottom - hy], [0.0, bottom], [head, bottom + hy]],
                        theta,
                        out,
                    );
                } else if upper_right {
                    self.stroke(&[[w - head, top - hy * 1.2], [w, top]], theta, out);
                    self.stroke(&[[head, bottom + hy * 1.2], [0.0, bottom]], theta, out);
                } else {
                    self.stroke(&[[head, top - hy * 1.2], [0.0, top]], theta, out);
                    self.stroke(&[[w - head, bottom + hy * 1.2], [w, bottom]], theta, out);
                }
            }
        }
    }

    fn color_box(
        &mut self,
        background: u32,
        frame: Option<u32>,
        body: &[Node],
        style: Style,
    ) -> Box {
        let inner = self.list(body, style);
        let pad = 0.3 * self.em(style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        let w = inner.width + 2.0 * pad;
        let top = -(inner.height + pad);
        let bottom = inner.depth + pad;
        out.push(Command::Rect(VectorRect {
            x: 0.0,
            y: top,
            width: w,
            height: bottom - top,
            color: background,
            radius: 0.0,
        }));
        if let Some(frame) = frame {
            let saved = self.color;
            self.color = frame;
            self.stroke(
                &[[0.0, top], [w, top], [w, bottom], [0.0, bottom], [0.0, top]],
                theta,
                &mut out,
            );
            self.color = saved;
        }
        self.translated(&inner, pad, 0.0, &mut out);
        Box {
            width: w,
            height: -top,
            depth: bottom,
            cmds: out,
            ..Box::default()
        }
    }

    fn brace_label(&mut self, ou: &parser::OverUnder, s: &parser::SupSub, style: Style) -> Box {
        let base = self.over_under(ou, style);
        let label_node: Option<&Node> = if ou.kind == parser::OverUnderKind::Overbrace {
            s.sup.as_deref().or(s.sub.as_deref())
        } else {
            s.sub.as_deref().or(s.sup.as_deref())
        };
        let label: Option<Box> = label_node.map(|n| self.build(n, style.sup()));
        if ou.kind == parser::OverUnderKind::Overbrace {
            self.assemble_limits(base, label, None, style, 0.0)
        } else {
            self.assemble_limits(base, None, label, style, 0.0)
        }
    }

    // ── boxes and cancels ───────────────────────────────────────────────

    fn boxed(&mut self, body: &[Node], style: Style) -> Box {
        let inner = self.list(body, style);
        let pad = 0.3 * self.em(style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        self.translated(&inner, pad, 0.0, &mut out);
        let w = inner.width + 2.0 * pad;
        let top = -(inner.height + pad);
        let bottom = inner.depth + pad;
        self.stroke(
            &[[0.0, top], [w, top], [w, bottom], [0.0, bottom]],
            theta,
            &mut out,
        );
        // close the rectangle
        self.stroke(&[[0.0, bottom], [0.0, top]], theta, &mut out);
        Box {
            width: w,
            height: -top + theta,
            depth: bottom + theta,
            cmds: out,
            ..Box::default()
        }
    }

    fn cancel(&mut self, body: &[Node], kind: parser::CancelKind, style: Style) -> Box {
        let inner = self.list(body, style);
        let theta = self.rule(style);
        let mut out: Vec<Command> = Vec::new();
        self.translated(&inner, 0.0, 0.0, &mut out);
        let w = inner.width;
        let top = -inner.height - 0.05 * self.em(style);
        let bottom = inner.depth + 0.05 * self.em(style);
        match kind {
            parser::CancelKind::Cancel => {
                self.stroke(&[[0.0, bottom], [w, top]], theta, &mut out);
            }
            parser::CancelKind::Bcancel => {
                self.stroke(&[[0.0, top], [w, bottom]], theta, &mut out);
            }
            parser::CancelKind::Xcancel => {
                self.stroke(&[[0.0, bottom], [w, top]], theta, &mut out);
                self.stroke(&[[0.0, top], [w, bottom]], theta, &mut out);
            }
            parser::CancelKind::Sout => {
                self.rect(
                    0.0,
                    -self.metric(style, Metric::AxisHeight) - theta / 2.0,
                    w,
                    theta,
                    &mut out,
                );
            }
        }
        Box {
            width: w,
            height: inner.height,
            depth: inner.depth,
            cmds: out,
            class: inner.class,
            ..Box::default()
        }
    }

    // ── arrays (KaTeX environments/array.js) ────────────────────────────

    fn array(&mut self, a: &parser::Array, outer: Style) -> Box {
        let cell_style: Style = if a.small {
            Style::new(Size::Script)
        } else if a.display {
            Style::DISPLAY
        } else {
            Style::TEXT
        };
        let em_size = self.em(outer);
        let arraystretch: f32 = if a.small { 0.5 } else { 1.0 };
        let baselineskip = 1.2 * em_size;
        let arrayskip = arraystretch * baselineskip;
        let strut_h = 0.7 * arrayskip;
        let strut_d = 0.3 * arrayskip;
        let jot: f32 = if a.aligned || a.display {
            0.3 * em_size
        } else {
            0.0
        };
        let ncols = a.cols.len();

        let mut cells: Vec<Vec<Box>> = Vec::with_capacity(a.rows.len());
        let mut col_widths = vec![0.0f32; ncols];
        let mut row_h = vec![0.0f32; a.rows.len()];
        let mut row_d = vec![0.0f32; a.rows.len()];
        for (r, row) in a.rows.iter().enumerate() {
            let mut row_boxes: Vec<Box> = Vec::with_capacity(ncols);
            let mut h = strut_h;
            let mut d = strut_d;
            for c in 0..ncols {
                // aligned: KaTeX starts every right-hand cell with an empty
                // ord so a leading relation (`&=`) keeps its spacing
                let box_ = if c >= row.len() {
                    Box::default()
                } else if a.aligned && c % 2 == 1 {
                    let mut nodes: Vec<Node> = Vec::with_capacity(row[c].len() + 1);
                    nodes.push(Node::Group(Vec::new()));
                    nodes.extend(row[c].iter().cloned());
                    self.list(&nodes, cell_style)
                } else {
                    self.list(&row[c], cell_style)
                };
                col_widths[c] = col_widths[c].max(box_.width);
                h = h.max(box_.height);
                d = d.max(box_.depth);
                row_boxes.push(box_);
            }
            row_h[r] = h;
            row_d[r] = d + jot;
            cells.push(row_boxes);
        }
        // vertical positions
        let mut row_y = vec![0.0f32; a.rows.len()];
        let mut total: f32 = 0.0;
        for r in 0..a.rows.len() {
            total += row_h[r];
            row_y[r] = total;
            total += row_d[r];
            if r < a.row_gaps.len() {
                total += a.row_gaps[r] * em_size;
            }
        }
        let offset = total / 2.0 + self.metric(outer, Metric::AxisHeight);

        // horizontal positions
        let mut col_x = vec![0.0f32; ncols];
        let mut x: f32 = 0.0;
        let sep = a.col_sep * em_size;
        for c in 0..ncols {
            if c > 0 {
                if a.aligned {
                    x += if c % 2 == 0 { 1.0 * em_size } else { 0.0 };
                } else {
                    x += 2.0 * sep;
                }
            }
            col_x[c] = x;
            x += col_widths[c];
        }
        let width = x;
        let mut out: Vec<Command> = Vec::new();
        for r in 0..a.rows.len() {
            let baseline = row_y[r] - offset;
            for c in 0..ncols {
                let box_ = &cells[r][c];
                let slack = col_widths[c] - box_.width;
                let dx: f32 = match a.cols[c] {
                    b'l' => 0.0,
                    b'r' => slack,
                    _ => slack / 2.0,
                };
                self.translated(box_, col_x[c] + dx, baseline, &mut out);
            }
        }
        let theta = self.metric(outer, Metric::DefaultRuleThickness);
        for &before in &a.col_rules {
            let rx = if before == 0 {
                -sep
            } else if before >= ncols {
                width + sep
            } else {
                col_x[before] - sep
            };
            self.rect(rx - theta / 2.0, -offset, theta, total, &mut out);
        }
        for &before in &a.row_rules {
            let mut ry: f32 = 0.0;
            for r in 0..before.min(a.rows.len()) {
                ry += row_h[r] + row_d[r];
            }
            self.rect(0.0, ry - offset - theta / 2.0, width, theta, &mut out);
        }
        let mut box_ = Box {
            width,
            height: offset,
            depth: total - offset,
            cmds: out,
            class: Class::Ord,
            ..Box::default()
        };
        if !a.left.is_empty() || !a.right.is_empty() {
            let total_h = self.left_right_total(box_.height, box_.depth, outer);
            let gap = if !a.left.is_empty() && a.right.is_empty() {
                0.2 * em_size
            } else {
                0.0
            };
            box_ = self.wrap_delims_gapped(box_, &a.left, &a.right, outer, total_h, gap);
        }
        box_
    }

    fn wrap_delims_gapped(
        &self,
        inner: Box,
        left: &str,
        right: &str,
        style: Style,
        total: f32,
        gap: f32,
    ) -> Box {
        let mut padded = inner;
        if gap > 0.0 {
            let mut out: Vec<Command> = Vec::new();
            self.translated(&padded, gap, 0.0, &mut out);
            padded.cmds = out;
            padded.width += gap;
        }
        self.wrap_delims(padded, left, right, style, total)
    }
}

fn kern(width: f32) -> Box {
    Box {
        width,
        is_space: true,
        ..Box::default()
    }
}

fn accent_glyph(name: &str) -> &'static str {
    match name {
        "hat" => "ˆ",
        "check" => "ˇ",
        "tilde" => "˜",
        "acute" => "ˊ",
        "grave" => "ˋ",
        "dot" => "˙",
        "ddot" => "¨",
        "breve" => "˘",
        "bar" => "ˉ",
        "mathring" => "˚",
        _ => "ˆ",
    }
}

// ── public entry point ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Font size of the surrounding text (px).
    pub size: f32,
    /// Math is drawn `scale` times larger than the surrounding text
    /// (KaTeX: 1.21 with Computer Modern; STIX Two has a larger x-height).
    pub scale: f32,
    pub color: u32,
    /// Streamdown's default `errorColor` is the muted foreground.
    pub error_color: u32,
    /// Display (block) style; false = inline/text style.
    pub display: bool,
    /// Shrinks the formula to fit this width (display math).
    pub max_width: Option<f32>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            size: 16.0,
            scale: 1.1,
            color: 0xff292d2a,
            error_color: 0xff8b918c,
            display: true,
            max_width: None,
        }
    }
}

impl HashValue for Options {
    fn hash_value(&self, h: &mut Wyhash) {
        // the fields in declaration order
        self.size.hash_value(h);
        self.scale.hash_value(h);
        self.color.hash_value(h);
        self.error_color.hash_value(h);
        self.display.hash_value(h);
        self.max_width.hash_value(h);
    }
}

/// Lays out `source` into a drawing.
pub fn layout(source: &str, options: Options) -> Drawing {
    let mut drawing = layout_once(source, &options, options.size * options.scale);
    if let Some(max) = options.max_width
        && drawing.width > max
        && drawing.width > 0.0
    {
        let factor = (max / drawing.width).max(0.5);
        drawing = layout_once(source, &options, options.size * options.scale * factor);
    }
    let mut h = Wyhash::init(0x6d617468);
    h.update(source.as_bytes());
    options.hash_value(&mut h);
    drawing.hash = h.finish();
    drawing
}

fn layout_once(source: &str, options: &Options, size: f32) -> Drawing {
    let nodes = match parser::parse(source) {
        Ok(nodes) => nodes,
        Err(_) => return error_drawing(source, options, size),
    };
    let mut engine = Engine::init(EngineOptions {
        size,
        color: options.color,
        error_color: options.error_color,
    });
    let style = if options.display {
        Style::DISPLAY
    } else {
        Style::TEXT
    };
    let mut box_ = engine.list(&nodes, style);
    if let Some(tag) = engine.tag.take() {
        // \tag{n}: placed after a quad
        let mut cmds: Vec<Command> = box_.cmds.clone();
        let x = box_.width + 2.0 * size;
        for cmd in &tag.cmds {
            cmds.push(vector::translate(cmd, x, 0.0));
        }
        box_.cmds = cmds;
        box_.width = x + tag.width;
        box_.height = box_.height.max(tag.height);
        box_.depth = box_.depth.max(tag.depth);
    }
    Drawing {
        width: box_.width,
        ascent: box_.height,
        descent: box_.depth,
        commands: box_.cmds,
        hash: 0,
    }
}

fn error_drawing(source: &str, options: &Options, size: f32) -> Drawing {
    let text = collapse_whitespace(source);
    let m = vector::measure(&text, size * 0.9, Family::Math, 400, false);
    let cmds = vec![Command::Text(VectorText {
        x: 0.0,
        y: 0.0,
        text: crate::Str::copy(&text),
        size: size * 0.9,
        family: Family::Math,
        weight: 400,
        italic: false,
        color: options.error_color,
        scale_y: 1.0,
    })];
    Drawing {
        width: m.advance,
        ascent: m.ascent.max(m.font_ascent),
        descent: m.descent.max(m.font_descent),
        commands: cmds,
        hash: 0,
    }
}

/// The source shown for a formula that does not parse (often one still
/// streaming): whitespace runs, line breaks included, become one space, as the
/// browser shows KaTeX's error span. A newline would draw as a missing glyph.
fn collapse_whitespace(source: &str) -> String {
    let trimmed = source.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    let mut out: Vec<u8> = Vec::with_capacity(trimmed.len());
    let mut space = false;
    for &c in trimmed.as_bytes() {
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            space = true;
            continue;
        }
        if space {
            out.push(b' ');
            space = false;
        }
        out.push(c);
    }
    // Only ASCII spaces were inserted at character boundaries, so this is
    // still valid UTF-8.
    String::from_utf8(out).expect("collapse_whitespace keeps UTF-8 valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_source_collapses_line_breaks() {
        assert_eq!(
            collapse_whitespace("\n\\begin{aligned}\n  f(x) &= a \\\\\r\n&= b\n"),
            "\\begin{aligned} f(x) &= a \\\\ &= b"
        );
    }
}
