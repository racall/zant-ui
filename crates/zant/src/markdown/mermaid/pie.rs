//! Pie chart parsing and drawing.

use super::canvas::{Canvas, anchor_top, strip_quotes, trim_line, trim_spaces};
use super::{Options, PALETTE};
use crate::Str;
use crate::core::vector::{self, Drawing, Point};
use crate::renderer::text::Family;

// ── pie charts ────────────────────────────────────────────────────────────

pub(super) fn pie(source: &str, options: &Options) -> Option<Drawing> {
    let mut title = "";
    let mut labels: Vec<Str> = Vec::new();
    let mut values: Vec<f32> = Vec::new();
    for raw in source.split('\n') {
        let line = trim_line(raw);
        if line.starts_with("pie") {
            if let Some(at) = line.find("title ") {
                title = trim_spaces(&line[at + 6..]);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            title = trim_spaces(rest);
            continue;
        }
        let Some(colon) = line.rfind(':') else {
            continue;
        };
        let Ok(value) = trim_spaces(&line[colon + 1..]).parse::<f32>() else {
            continue;
        };
        if value <= 0.0 {
            continue;
        }
        labels.push(Str::copy(strip_quotes(&line[..colon])));
        values.push(value);
    }
    if values.is_empty() {
        return None;
    }
    let mut total: f32 = 0.0;
    for value in &values {
        total += value;
    }
    let fs = options.font_size;
    let r: f32 = 90.0;
    let cx: f32 = 8.0 + r;
    let mut top: f32 = 8.0;
    let mut canvas = Canvas::new();
    if !title.is_empty() {
        canvas.text(8.0, top + fs, title, fs * 1.1, options.colors.text);
        top += fs * 2.0;
    }
    let cy = top + r;
    let mut angle: f32 = -std::f32::consts::FRAC_PI_2;
    let mut legend_w: f32 = 0.0;
    for (i, value) in values.iter().enumerate() {
        let sweep = value / total * std::f32::consts::TAU;
        let steps: usize = ((sweep / std::f32::consts::TAU * 72.0) as usize).max(2);
        let mut points: Vec<Point> = Vec::new();
        points.push([cx, cy]);
        for k in 0..=steps {
            let a = angle + sweep * k as f32 / steps as f32;
            points.push([cx + a.cos() * r, cy + a.sin() * r]);
        }
        let color = PALETTE[i % PALETTE.len()];
        canvas.poly(&points, color, 0.0, true);
        canvas.poly(&points, options.colors.label_bg, 1.5, true);
        angle += sweep;
        // legend
        let ly = top + 6.0 + i as f32 * fs * 1.8;
        canvas.rect(cx + r + 28.0, ly, fs * 0.9, fs * 0.9, color, 2.0);
        let pct = format!("{}  {:.1}%", labels[i], value / total * 100.0);
        canvas.text(
            cx + r + 28.0 + fs * 1.4,
            ly + fs * 0.8,
            &pct,
            fs * 0.92,
            options.colors.text,
        );
        legend_w = legend_w.max(vector::measure(&pct, fs * 0.92, Family::Sans, 400, false).advance);
    }
    let legend_h = values.len() as f32 * fs * 1.8;
    Some(anchor_top(canvas.finish(
        cx + r + 28.0 + fs * 1.4 + legend_w + 8.0,
        (cy + r).max(top + legend_h) + 8.0,
    )))
}
