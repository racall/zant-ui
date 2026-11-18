//! Sinks that turn traced icons into canvas paths or drawings.

use super::*;

/// Stroke width in icon units for a given pixel size (≈1.5px at 16px).
pub fn default_stroke(size: f32) -> f32 {
    (size / 24.0 * 2.0).max(1.2)
}

// ── immediate painting (buttons) ──────────────────────────────────────────

/// [`Sink`] that strokes directly on a [`Canvas`].
pub(super) struct CanvasSink {
    canvas: Canvas,
    /// Top-left of the icon box, in canvas space.
    x: f32,
    y: f32,
    scale: f32,
    color: u32,
    width: f32,
}

impl Sink for CanvasSink {
    fn stroke(&mut self, points: &[Point], closed: bool) {
        // At most 128 points per polyline are painted.
        let count = points.len().min(128);
        let translated: Vec<Point> = points[..count]
            .iter()
            .map(|point| {
                [
                    self.x + point[0] * self.scale,
                    self.y + point[1] * self.scale,
                ]
            })
            .collect();
        self.canvas
            .stroke_path(&translated, self.color, self.width, closed);
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        let rr = r * self.scale + self.width * 0.25;
        let rect = Rect::init(
            self.x + x * self.scale - rr,
            self.y + y * self.scale - rr,
            rr * 2.0,
            rr * 2.0,
        );
        self.canvas.fill_round_rect(rect, rr, self.color);
    }
}

/// Paints `icon` centred at (cx, cy) with a `size`-px box.
pub fn paint(
    canvas: Canvas,
    icon: Icon,
    cx: f32,
    cy: f32,
    size: f32,
    color: u32,
    stroke: Option<f32>,
) {
    let width = stroke.unwrap_or_else(|| default_stroke(size));
    // A translucent colour (disabled / secondary text) would darken wherever
    // the strokes overlap and blur the glyph: draw it opaque into a layer and
    // apply the alpha once.
    let alpha = color >> 24;
    let layered = alpha > 0 && alpha < 0xff;
    if layered {
        let pad = width + 1.0;
        canvas.save_layer_alpha(
            Rect::init(
                cx - size / 2.0 - pad,
                cy - size / 2.0 - pad,
                size + pad * 2.0,
                size + pad * 2.0,
            ),
            alpha as f32 / 255.0,
        );
    }
    let mut sink = CanvasSink {
        canvas,
        x: cx - size / 2.0,
        y: cy - size / 2.0,
        scale: size / 24.0,
        color: if layered { color | 0xff00_0000 } else { color },
        width,
    };
    trace(icon, &mut sink);
    if layered {
        canvas.restore();
    }
}

// ── retained drawings (ui.icon) ───────────────────────────────────────────

/// [`Sink`] that collects [`vector::Command`]s.
pub(super) struct BuildSink {
    commands: Vec<Command>,
    scale: f32,
    top: f32,
    color: u32,
    width: f32,
}

impl Sink for BuildSink {
    fn stroke(&mut self, points: &[Point], closed: bool) {
        let translated: Vec<Point> = points
            .iter()
            .map(|point| [point[0] * self.scale, point[1] * self.scale - self.top])
            .collect();
        self.commands.push(Command::Path(Path {
            points: translated,
            color: self.color,
            stroke_width: self.width,
            closed,
        }));
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        let rr = r * self.scale + self.width * 0.25;
        self.commands.push(Command::Rect(DrawingRect {
            x: x * self.scale - rr,
            y: y * self.scale - self.top - rr,
            width: rr * 2.0,
            height: rr * 2.0,
            radius: rr,
            color: self.color,
        }));
    }
}

/// A `size`×`size` drawing whose box sits entirely above the baseline.
pub fn drawing(icon: Icon, size: f32, color: u32, stroke: Option<f32>) -> Drawing {
    let width = stroke.unwrap_or_else(|| default_stroke(size));
    let mut sink = BuildSink {
        commands: Vec::new(),
        scale: size / 24.0,
        top: size,
        color,
        width,
    };
    trace(icon, &mut sink);
    let BuildSink { commands, .. } = sink;

    // Wyhash (seed 0x69636f6e) over the discriminant as a `u32`
    // (`#[repr(u32)]`), `size`, `color` and the stroke width. This is an
    // internal cache key, so only stability matters.
    let mut hasher = Wyhash::init(0x6963_6f6e);
    hasher.update(&(icon as u32).to_le_bytes());
    hasher.update(&size.to_le_bytes());
    hasher.update(&color.to_le_bytes());
    hasher.update(&width.to_le_bytes());
    Drawing {
        width: size,
        ascent: size,
        descent: 0.0,
        commands,
        hash: hasher.finish(),
    }
}
