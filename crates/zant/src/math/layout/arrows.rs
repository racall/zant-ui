//! Extensible arrows and their labels.

use crate::core::vector::Command;
use crate::math::{parser, symbols::Class};

use super::{Box, Engine, Style, style::Metric};

impl Engine {
    /// `\xrightarrow[below]{above}` (KaTeX functions/arrow.js + stretchy.js):
    /// labels in script style padded 0.5em each side, the arrow at least
    /// its minimum width and centred on the axis, labels 0.111em from it.
    pub(super) fn x_arrow(&mut self, x: &parser::XArrow, style: Style) -> Box {
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
}
