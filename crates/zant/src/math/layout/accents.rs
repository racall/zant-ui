//! Accents and over/under decorations.

use crate::core::vector::{self, Command, Text as VectorText};
use crate::math::{parser, symbols::Class};
use crate::renderer::text::Family;

use super::{Box, Engine, Style, style::Metric};

impl Engine {
    // ── accents and over/under decorations ──────────────────────────────

    pub(super) fn accent(&mut self, a: &parser::AccentNode, style: Style) -> Box {
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

    pub(super) fn over_under(&mut self, o: &parser::OverUnder, style: Style) -> Box {
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
