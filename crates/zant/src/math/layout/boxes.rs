//! Math boxes, list layout, and TeX inter-atom spacing.

use crate::core::vector::Command;
use crate::math::parser::{self, Node};
use crate::math::symbols::Class;

use super::{Engine, Size, Style};

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

impl Engine {
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

    pub(super) fn build(&mut self, node: &Node, style: Style) -> Box {
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
}

fn kern(width: f32) -> Box {
    Box {
        width,
        is_space: true,
        ..Box::default()
    }
}
