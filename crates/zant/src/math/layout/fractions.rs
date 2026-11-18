//! Fractions, binomials, and radicals.

use crate::core::vector::Command;
use crate::math::{parser, symbols::Class};

use super::{Box, Engine, Size, Style, style::Metric};

impl Engine {
    // ── genfrac (KaTeX functions/genfrac.js) ────────────────────────────

    pub(super) fn frac(&mut self, f: &parser::Frac, outer: Style) -> Box {
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

    // ── sqrt (KaTeX functions/sqrt.js + delimiter.makeSqrtImage) ────────

    pub(super) fn sqrt(&mut self, s: &parser::Sqrt, style: Style) -> Box {
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
}
