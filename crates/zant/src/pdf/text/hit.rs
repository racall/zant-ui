//! Hit testing glyphs and the highlight rectangles of a range.

use super::*;

impl PageText {
    /// Glyph under a point, with PDFium's tolerance fallback (EmbedPDF
    /// `glyphAt`): exact tight-box match first, then the nearest glyph whose
    /// box expanded by `tolerance_factor` × average glyph height contains it.
    pub fn glyph_at(&self, x: f32, y: f32, tolerance_factor: f32) -> Option<usize> {
        for run in &self.runs {
            if !run.rect.contains(x, y) {
                continue;
            }
            let start = run.char_start as usize;
            let end = start + run.len as usize;
            for (k, g) in self.glyphs[start..end].iter().enumerate() {
                if g.kind == GlyphKind::Empty {
                    continue;
                }
                if g.tight.contains(x, y) {
                    return Some(start + k);
                }
            }
        }
        if tolerance_factor <= 0.0 {
            return None;
        }
        let half = self.tolerance(tolerance_factor) / 2.0;
        let mut best: Option<usize> = None;
        let mut best_dist = f32::INFINITY;
        for run in &self.runs {
            let r = run.rect;
            if y < r.y - half || y > r.bottom() + half || x < r.x - half || x > r.right() + half {
                continue;
            }
            let start = run.char_start as usize;
            let end = start + run.len as usize;
            for (k, g) in self.glyphs[start..end].iter().enumerate() {
                if g.kind == GlyphKind::Empty {
                    continue;
                }
                let t = g.tight;
                if x < t.x - half || x > t.right() + half || y < t.y - half || y > t.bottom() + half
                {
                    continue;
                }
                let dx = (x - t.x).abs().min((x - t.right()).abs());
                let dy = (y - t.y).abs().min((y - t.bottom()).abs());
                if dx + dy < best_dist {
                    best_dist = dx + dy;
                    best = Some(start + k);
                }
            }
        }
        best
    }

    pub(super) fn tolerance(&self, factor: f32) -> f32 {
        let mut total: f32 = 0.0;
        let mut n: f32 = 0.0;
        for g in &self.glyphs {
            if g.kind == GlyphKind::Empty {
                continue;
            }
            total += g.r#box.height;
            n += 1.0;
        }
        if n == 0.0 { 0.0 } else { total / n * factor }
    }

    /// Nearest glyph boundary for a drag that is not over text: the glyph
    /// on the closest line, left or right of the point. Selections started
    /// or continued over margins snap to it (like Chromium / Preview).
    pub fn nearest_glyph(&self, x: f32, y: f32) -> Option<usize> {
        let mut best: Option<usize> = None;
        let mut best_score = f32::INFINITY;
        for (i, g) in self.glyphs.iter().enumerate() {
            if g.kind == GlyphKind::Empty {
                continue;
            }
            let b = g.r#box;
            let dy = if y < b.y {
                b.y - y
            } else if y > b.bottom() {
                y - b.bottom()
            } else {
                0.0
            };
            let dx = if x < b.x {
                b.x - x
            } else if x > b.right() {
                x - b.right()
            } else {
                0.0
            };
            // lines matter more than columns
            let score = dy * 4.0 + dx;
            if score < best_score {
                best_score = score;
                best = Some(i);
            }
        }
        best
    }

    /// Highlight rectangles for glyphs `from..to` (inclusive), split at big
    /// gaps and merged across runs on the same line (EmbedPDF
    /// `rectsWithinSlice` + Chromium `MergeAdjacentRects`).
    pub fn rects(&self, from: usize, to: usize) -> Vec<Rect> {
        const CHAR_DISTANCE_FACTOR: f32 = 2.5;
        let mut parts: Vec<Part> = Vec::new();
        for run in &self.runs {
            let run_start = run.char_start as usize;
            let run_end = run_start + run.len as usize;
            if run.len == 0 || run_end <= from || run_start > to {
                continue;
            }
            let s = from.max(run_start);
            let e = to.min(run_end - 1);
            let mut acc: Option<Rect> = None;
            let mut n: u32 = 0;
            let mut width_sum: f32 = 0.0;
            let mut prev_right: f32 = -f32::INFINITY;
            let mut i = s;
            while i <= e {
                let g = self.glyphs[i];
                if g.kind == GlyphKind::Empty {
                    i += 1;
                    continue;
                }
                if n > 0 {
                    let avg = width_sum / n as f32;
                    if avg > 0.0 && (g.r#box.x - prev_right).abs() > CHAR_DISTANCE_FACTOR * avg {
                        parts.push(Part {
                            rect: acc.expect("acc"),
                            chars: n,
                            font_size: run.font_size,
                        });
                        acc = None;
                        n = 0;
                        width_sum = 0.0;
                    }
                }
                acc = Some(match acc {
                    Some(a) => a.unite(g.r#box),
                    None => g.r#box,
                });
                n += 1;
                width_sum += g.r#box.width;
                prev_right = g.r#box.right();
                i += 1;
            }
            if let Some(a) = acc {
                parts.push(Part {
                    rect: a,
                    chars: n,
                    font_size: run.font_size,
                });
            }
        }
        let mut out: Vec<Rect> = Vec::new();
        let mut current: Option<Rect> = None;
        let mut previous: Option<Part> = None;
        for part in parts {
            match (previous, current) {
                (Some(previous), Some(rect)) => {
                    if should_merge(previous, part) {
                        current = Some(rect.unite(part.rect));
                    } else {
                        out.push(rect);
                        current = Some(part.rect);
                    }
                }
                _ => current = Some(part.rect),
            }
            previous = Some(part);
        }
        if let Some(c) = current
            && !c.is_empty()
        {
            out.push(c);
        }
        out
    }
}
