//! Glyphs, text, and vector drawing primitives.

use crate::core::vector::{self, Command, Point, Rect as VectorRect, Text as VectorText};
use crate::math::{fonts, parser, symbols::Class};
use crate::renderer::text::Family;

use super::{Box, Engine, Style};

impl Engine {
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

    pub(super) fn text_box(&self, text: &str, size: f32, color: u32) -> Box {
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

    pub(super) fn translated(&self, box_: &Box, dx: f32, dy: f32, out: &mut Vec<Command>) {
        for cmd in &box_.cmds {
            out.push(vector::translate(cmd, dx, dy));
        }
    }

    pub(super) fn rect(&self, x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Command>) {
        out.push(Command::Rect(VectorRect {
            x,
            y,
            width: w,
            height: h,
            color: self.color,
            radius: 0.0,
        }));
    }

    pub(super) fn stroke(&self, points: &[Point], width: f32, out: &mut Vec<Command>) {
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

    pub(super) fn atom(&self, a: &parser::Atom, style: Style) -> Box {
        let mapped = fonts::map_text(a.variant, &a.text);
        let italic = a.variant == fonts::Variant::Normal && mapped != a.text;
        self.glyph(&mapped, self.em(style), a.class, italic)
    }

    pub(super) fn text_node(&self, t: &parser::Text, style: Style) -> Box {
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
}
