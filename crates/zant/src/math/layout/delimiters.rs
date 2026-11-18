//! Delimiter sizing and layout for paired and standalone delimiters.

use crate::core::vector::{self, Command, Text as VectorText};
use crate::math::{
    parser::{self, Node},
    symbols::Class,
};
use crate::renderer::text::Family;

use super::{Box, Engine, Style, style::Metric};

impl Engine {
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
    pub(super) fn left_right_total(&self, height: f32, depth: f32, style: Style) -> f32 {
        let axis = self.metric(style, Metric::AxisHeight);
        let delimiter_factor: f32 = 901.0;
        let delimiter_extend = 0.5 * self.em(style);
        let max_dist = (height - axis).max(depth + axis);
        (max_dist / 500.0 * delimiter_factor).max(2.0 * max_dist - delimiter_extend)
    }

    pub(super) fn wrap_delims(
        &self,
        inner: Box,
        left: &str,
        right: &str,
        style: Style,
        total: f32,
    ) -> Box {
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

    pub(super) fn left_right(&mut self, lr: &parser::LeftRight, style: Style) -> Box {
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

    pub(super) fn sized_delim(&self, d: &parser::SizedDelim, style: Style) -> Box {
        let heights = [0.0f32, 1.2, 1.8, 2.4, 3.0];
        let total = if d.size == 0 {
            self.middle_total.unwrap_or(self.em(style))
        } else {
            heights[usize::from(d.size)] * self.options.size
        };
        self.delimiter(&d.delim, total, style, d.class)
    }
}
