//! Port of `src/core/vector.zig` — WP-1.3 (drawn early so the ui builders and
//! math layout can use it).
//!
//! Retained vector drawings: glyph runs, rules, lines and paths positioned
//! relative to a baseline origin. Used by `ui::vector` nodes (math blocks,
//! diagrams) and by rich-text inline embeds (inline math).
//!
//! Coordinates: x grows right from the drawing's left edge, y grows down from
//! the baseline (so glyphs above the baseline have negative y).

use std::rc::Rc;

use crate::core::hash::Wyhash;
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, Family, InkMetrics};

pub type Point = [f32; 2];

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub x: f32,
    /// Baseline y.
    pub y: f32,
    pub text: crate::Str,
    pub size: f32,
    pub family: Family,
    pub weight: u16,
    pub italic: bool,
    pub color: u32,
    /// Vertical stretch around the baseline (stretchy delimiters).
    pub scale_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: u32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub color: u32,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub points: Vec<Point>,
    pub color: u32,
    /// 0 fills the polygon; > 0 strokes it.
    pub stroke_width: f32,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Text(Text),
    Rect(Rect),
    Line(Line),
    Path(Path),
}

impl Command {
    fn tag(&self) -> u8 {
        // Zig union(enum) declaration order: text, rect, line, path.
        match self {
            Command::Text(_) => 0,
            Command::Rect(_) => 1,
            Command::Line(_) => 2,
            Command::Path(_) => 3,
        }
    }
}

/// A retained drawing. `hash` is a content hash used to skip re-copying
/// unchanged drawings; 0 = compute on demand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing {
    pub width: f32,
    /// Extent above the baseline.
    pub ascent: f32,
    /// Extent below the baseline.
    pub descent: f32,
    pub commands: Vec<Command>,
    pub hash: u64,
}

impl Drawing {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Content hash (cached in `hash`). Zig hashed the raw struct bytes with
    /// Wyhash; the Rust port hashes field by field in the same order (the exact
    /// byte stream is internal to the framework, so only stability matters).
    pub fn content_hash(&self) -> u64 {
        if self.hash != 0 {
            return self.hash;
        }
        let mut hasher = Wyhash::init(0x5eed);
        hasher.update(&self.width.to_le_bytes());
        hasher.update(&self.ascent.to_le_bytes());
        hasher.update(&self.descent.to_le_bytes());
        for command in &self.commands {
            hash_command(&mut hasher, command);
        }
        hasher.finish()
    }

    /// The drawing uniformly scaled by `factor` (positions, sizes, stroke
    /// widths; command text and points are cloned).
    pub fn scaled(&self, factor: f32) -> Drawing {
        let commands = self
            .commands
            .iter()
            .map(|command| match command {
                Command::Text(text) => {
                    let mut text = text.clone();
                    text.x *= factor;
                    text.y *= factor;
                    text.size *= factor;
                    Command::Text(text)
                }
                Command::Rect(rect) => Command::Rect(Rect {
                    x: rect.x * factor,
                    y: rect.y * factor,
                    width: rect.width * factor,
                    height: rect.height * factor,
                    color: rect.color,
                    radius: rect.radius * factor,
                }),
                Command::Line(line) => Command::Line(Line {
                    x0: line.x0 * factor,
                    y0: line.y0 * factor,
                    x1: line.x1 * factor,
                    y1: line.y1 * factor,
                    color: line.color,
                    width: (line.width * factor).max(0.5),
                }),
                Command::Path(path) => {
                    let mut path = path.clone();
                    path.points = path
                        .points
                        .iter()
                        .map(|point| [point[0] * factor, point[1] * factor])
                        .collect();
                    path.stroke_width = if path.stroke_width > 0.0 {
                        (path.stroke_width * factor).max(0.5)
                    } else {
                        0.0
                    };
                    Command::Path(path)
                }
            })
            .collect();
        Drawing {
            width: self.width * factor,
            ascent: self.ascent * factor,
            descent: self.descent * factor,
            commands,
            hash: 0,
        }
    }

    /// Deep copy with the content hash pinned (Zig: `clone(allocator)`).
    pub fn deep_clone(&self) -> Drawing {
        Drawing {
            width: self.width,
            ascent: self.ascent,
            descent: self.descent,
            commands: self.commands.clone(),
            hash: self.content_hash(),
        }
    }
}

fn hash_command(hasher: &mut Wyhash, command: &Command) {
    hasher.update(&[command.tag()]);
    match command {
        Command::Text(text) => {
            for value in [text.x, text.y, text.size, text.scale_y] {
                hasher.update(&value.to_le_bytes());
            }
            hasher.update(&text.color.to_le_bytes());
            hasher.update(&text.weight.to_le_bytes());
            hasher.update(&[text.family as u8, u8::from(text.italic)]);
            hasher.update(text.text.as_bytes());
        }
        Command::Rect(rect) => {
            for value in [rect.x, rect.y, rect.width, rect.height, rect.radius] {
                hasher.update(&value.to_le_bytes());
            }
            hasher.update(&rect.color.to_le_bytes());
        }
        Command::Line(line) => {
            for value in [line.x0, line.y0, line.x1, line.y1, line.width] {
                hasher.update(&value.to_le_bytes());
            }
            hasher.update(&line.color.to_le_bytes());
        }
        Command::Path(path) => {
            hasher.update(&path.color.to_le_bytes());
            hasher.update(&path.stroke_width.to_le_bytes());
            hasher.update(&[u8::from(path.closed)]);
            for point in &path.points {
                hasher.update(&point[0].to_le_bytes());
                hasher.update(&point[1].to_le_bytes());
            }
        }
    }
}

/// Glyph-ink metrics of a single-line run (see `zant_text_measure_ink`).
pub use text::InkMetrics as Ink;

/// Measures a single-line run (math layout uses this for stretchy delimiters).
pub fn measure(text: &str, size: f32, family: Family, weight: u16, italic: bool) -> InkMetrics {
    text::measure_ink(text, size, family, weight, italic)
}

/// Paints `drawing` with its baseline origin at (`x`, `baseline`).
pub fn paint(canvas: Canvas, drawing: &Drawing, x: f32, baseline: f32) {
    for command in &drawing.commands {
        match command {
            Command::Text(text) => canvas.draw_simple_text(
                &text.text,
                x + text.x,
                baseline + text.y,
                text.size,
                text.family,
                i32::from(text.weight),
                text.italic,
                text.color,
                text.scale_y,
            ),
            Command::Rect(rect) => {
                let bounds = crate::core::geometry::Rect::init(
                    x + rect.x,
                    baseline + rect.y,
                    rect.width,
                    rect.height,
                );
                if rect.radius > 0.0 {
                    canvas.fill_round_rect(bounds, rect.radius, rect.color);
                } else {
                    canvas.fill_rect(bounds, rect.color);
                }
            }
            Command::Line(line) => canvas.draw_line(
                x + line.x0,
                baseline + line.y0,
                x + line.x1,
                baseline + line.y1,
                line.color,
                line.width,
            ),
            Command::Path(path) => {
                // Zig painted at most 128 points per path (a stack buffer).
                let count = path.points.len().min(128);
                let points: Vec<Point> = path.points[..count]
                    .iter()
                    .map(|point| [x + point[0], baseline + point[1]])
                    .collect();
                if path.stroke_width > 0.0 {
                    canvas.stroke_path(&points, path.color, path.stroke_width, path.closed);
                } else {
                    canvas.fill_path(&points, path.color);
                }
            }
        }
    }
}

/// Builder that collects commands; the caller supplies the final extent.
#[derive(Default)]
pub struct Builder {
    commands: Vec<Command>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder::default()
    }

    pub fn add(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Appends `commands` translated by (dx, dy).
    pub fn add_translated(&mut self, commands: &[Command], dx: f32, dy: f32) {
        for command in commands {
            self.commands.push(translate(command, dx, dy));
        }
    }

    pub fn finish(self, width: f32, ascent: f32, descent: f32) -> Drawing {
        Drawing {
            width,
            ascent,
            descent,
            commands: self.commands,
            hash: 0,
        }
    }
}

/// `command` translated by (dx, dy).
pub fn translate(command: &Command, dx: f32, dy: f32) -> Command {
    match command {
        Command::Text(text) => {
            let mut text = text.clone();
            text.x += dx;
            text.y += dy;
            Command::Text(text)
        }
        Command::Rect(rect) => {
            let mut rect = *rect;
            rect.x += dx;
            rect.y += dy;
            Command::Rect(rect)
        }
        Command::Line(line) => Command::Line(Line {
            x0: line.x0 + dx,
            y0: line.y0 + dy,
            x1: line.x1 + dx,
            y1: line.y1 + dy,
            color: line.color,
            width: line.width,
        }),
        Command::Path(path) => {
            let mut path = path.clone();
            path.points = path
                .points
                .iter()
                .map(|point| [point[0] + dx, point[1] + dy])
                .collect();
            Command::Path(path)
        }
    }
}

/// Shares a drawing between nodes without copying it (`Element::drawing`).
pub type DrawingRef = Rc<Drawing>;

#[cfg(test)]
mod tests {
    use super::*;

    fn text_command() -> Command {
        Command::Text(Text {
            x: 1.0,
            y: -2.0,
            text: "x".into(),
            size: 12.0,
            family: Family::Math,
            weight: 400,
            italic: false,
            color: 0xff00_0000,
            scale_y: 1.0,
        })
    }

    #[test]
    fn heights_and_hashes_are_stable() {
        let drawing = Drawing {
            width: 10.0,
            ascent: 8.0,
            descent: 2.0,
            commands: vec![text_command()],
            hash: 0,
        };
        assert_eq!(drawing.height(), 10.0);
        let hash = drawing.content_hash();
        assert_ne!(hash, 0);
        let pinned = Drawing {
            hash,
            ..drawing.clone()
        };
        assert_eq!(
            pinned.content_hash(),
            hash,
            "a pinned hash is returned as is"
        );
        let deep = drawing.deep_clone();
        assert_eq!(deep.content_hash(), hash);
    }

    #[test]
    fn scaling_multiplies_positions_sizes_and_outlines() {
        let drawing = Drawing {
            width: 10.0,
            ascent: 8.0,
            descent: 2.0,
            commands: vec![
                text_command(),
                Command::Line(Line {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 1.0,
                    y1: 1.0,
                    color: 0xff00_0000,
                    width: 0.2,
                }),
            ],
            hash: 0,
        };
        let scaled = drawing.scaled(2.0);
        assert_eq!(scaled.width, 20.0);
        assert_eq!(scaled.ascent, 16.0);
        match &scaled.commands[0] {
            Command::Text(text) => {
                assert_eq!(text.x, 2.0);
                assert_eq!(text.size, 24.0);
            }
            other => panic!("expected text, got {other:?}"),
        }
        match &scaled.commands[1] {
            Command::Line(line) => assert_eq!(line.width, 0.5, "strokes stay visible"),
            other => panic!("expected line, got {other:?}"),
        }
    }

    #[test]
    fn the_builder_translates_and_finishes() {
        let mut builder = Builder::new();
        builder.add(text_command());
        builder.add_translated(&[text_command()], 5.0, 6.0);
        let drawing = builder.finish(20.0, 10.0, 2.0);
        assert_eq!(drawing.commands.len(), 2);
        match &drawing.commands[1] {
            Command::Text(text) => {
                assert_eq!(text.x, 6.0);
                assert_eq!(text.y, 4.0);
            }
            other => panic!("expected text, got {other:?}"),
        }
        match translate(&text_command(), -1.0, -2.0) {
            Command::Text(text) => {
                assert_eq!(text.x, 0.0);
                assert_eq!(text.y, -4.0);
            }
            other => panic!("expected text, got {other:?}"),
        }
    }
}
