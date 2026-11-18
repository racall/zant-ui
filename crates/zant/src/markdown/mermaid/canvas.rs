//! Shared drawing and text helpers for Mermaid diagrams.

use crate::Str;
use crate::core::vector::{self, Command, Drawing, Path, Point, Rect, Text};
use crate::renderer::text::Family;

// ── shared drawing helpers ────────────────────────────────────────────────

pub(super) struct Canvas {
    commands: Vec<Command>,
}

impl Canvas {
    pub(super) fn new() -> Canvas {
        Canvas {
            commands: Vec::new(),
        }
    }

    pub(super) fn text(&mut self, x: f32, baseline: f32, text: &str, size: f32, color: u32) {
        self.commands.push(Command::Text(Text {
            x,
            y: baseline,
            text: Str::copy(text),
            size,
            family: Family::Sans,
            weight: 400,
            italic: false,
            color,
            scale_y: 1.0,
        }));
    }

    pub(super) fn centered_text(&mut self, cx: f32, cy: f32, text: &str, size: f32, color: u32) {
        let m = vector::measure(text, size, Family::Sans, 400, false);
        self.text(
            cx - m.advance / 2.0,
            cy + (m.font_ascent - m.font_descent) / 2.0,
            text,
            size,
            color,
        );
    }

    pub(super) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32, radius: f32) {
        self.commands.push(Command::Rect(Rect {
            x,
            y,
            width: w,
            height: h,
            color,
            radius,
        }));
    }

    pub(super) fn poly(&mut self, points: &[Point], color: u32, stroke_width: f32, closed: bool) {
        self.commands.push(Command::Path(Path {
            points: points.to_vec(),
            color,
            stroke_width,
            closed,
        }));
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn line(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: u32,
        width: f32,
        dashed: bool,
    ) {
        if !dashed {
            return self.poly(&[[x0, y0], [x1, y1]], color, width, false);
        }
        let len = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
        if len < 0.1 {
            return;
        }
        let dash: f32 = 5.0;
        let mut t: f32 = 0.0;
        while t < len {
            let e = (t + dash).min(len);
            self.poly(
                &[
                    [x0 + (x1 - x0) * t / len, y0 + (y1 - y0) * t / len],
                    [x0 + (x1 - x0) * e / len, y0 + (y1 - y0) * e / len],
                ],
                color,
                width,
                false,
            );
            t += dash * 2.0;
        }
    }

    pub(super) fn arrow_head(&mut self, x: f32, y: f32, dx: f32, dy: f32, color: u32) {
        let len = 0.001f32.max((dx * dx + dy * dy).sqrt());
        let ux = dx / len;
        let uy = dy / len;
        let s: f32 = 8.0;
        self.poly(
            &[
                [x, y],
                [x - ux * s - uy * s * 0.5, y - uy * s + ux * s * 0.5],
                [x - ux * s + uy * s * 0.5, y - uy * s - ux * s * 0.5],
            ],
            color,
            0.0,
            true,
        );
    }

    pub(super) fn rounded_border(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
        // approximate rounded rectangle outline
        let mut points: Vec<Point> = Vec::new();
        let corners: [[f32; 3]; 4] = [
            [x + w - r, y + r, -std::f32::consts::FRAC_PI_2],
            [x + w - r, y + h - r, 0.0],
            [x + r, y + h - r, std::f32::consts::FRAC_PI_2],
            [x + r, y + r, std::f32::consts::PI],
        ];
        for corner in corners {
            let mut k: usize = 0;
            while k <= 6 {
                let a = corner[2] + k as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
                points.push([corner[0] + a.cos() * r, corner[1] + a.sin() * r]);
                k += 1;
            }
        }
        self.poly(&points, color, 1.2, true);
    }

    /// Finishes with the top-left at (0,0); `ascent` = full height.
    pub(super) fn finish(self, width: f32, height: f32) -> Drawing {
        Drawing {
            width,
            ascent: height,
            descent: 0.0,
            commands: self.commands,
            hash: 0,
        }
    }
}

/// Shifts every command up by `height` so the drawing's top is y = -ascent
/// (drawings are baseline-anchored; diagrams sit entirely above it).
pub(super) fn anchor_top(drawing: Drawing) -> Drawing {
    let ascent = drawing.ascent;
    let commands = drawing
        .commands
        .iter()
        .map(|command| vector::translate(command, 0.0, -ascent))
        .collect();
    Drawing {
        width: drawing.width,
        ascent: drawing.ascent,
        descent: drawing.descent,
        commands,
        hash: drawing.hash,
    }
}

pub(super) fn trim_line(value: &str) -> &str {
    value.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r')
}

pub(super) fn trim_spaces(value: &str) -> &str {
    value.trim_matches(' ')
}

pub(super) fn trim_leading(value: &str) -> &str {
    value.trim_start_matches([' ', '\t'])
}

pub(super) fn trim_rhs(value: &str) -> &str {
    value.trim_matches(|c: char| c == ' ' || c == '+' || c == '-')
}

pub(super) fn strip_quotes(text: &str) -> &str {
    let s = text.trim_matches(|c: char| c == ' ' || c == '\t');
    let bytes = s.as_bytes();
    if s.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') && bytes[s.len() - 1] == bytes[0] {
        return &s[1..s.len() - 1];
    }
    s
}
