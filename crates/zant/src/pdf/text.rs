//! Page text geometry: glyph runs, hit testing, selection rectangles, word /
//! line expansion, text extraction and search.
//!
//! Ported from EmbedPDF's selection plugin (`plugin-selection/src/lib/
//! utils.ts`, MIT), which adapts PDFium's `CPDF_TextPage::GetIndexAtPos` and
//! Chromium's PDF text selection (`pdf/pdfium/pdfium_range.cc`,
//! `PDFEngine::OnMultipleClick`, BSD-3-Clause). Coordinates are points of the
//! displayed page (top-left origin, y down).
//!
//! Codepoints are `u32`: the native stream may carry surrogate
//! values (0 for the second half of a UTF-16 pair), which `char` cannot hold.

use crate::pdf::native;

/// A rectangle in page points (top-left origin).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Smallest rectangle containing both.
    pub fn unite(self, b: Rect) -> Rect {
        let left = self.x.min(b.x);
        let top = self.y.min(b.y);
        Rect {
            x: left,
            y: top,
            width: self.right().max(b.right()) - left,
            height: self.bottom().max(b.bottom()) - top,
        }
    }

    /// Overlap of both rectangles (empty when they do not overlap).
    pub fn intersect(self, b: Rect) -> Rect {
        let left = self.x.max(b.x);
        let top = self.y.max(b.y);
        Rect {
            x: left,
            y: top,
            width: (self.right().min(b.right()) - left).max(0.0),
            height: (self.bottom().min(b.bottom()) - top).max(0.0),
        }
    }
}

/// Glyph class from the native `flags`: 1 = space, 2 = empty box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum GlyphKind {
    #[default]
    Text = 0,
    Space = 1,
    Empty = 2,
}

/// One glyph of a page (`box` is a keyword, hence `r#box`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Glyph {
    /// Loose box (line height): highlight rectangles.
    pub r#box: Rect,
    /// Tight (ink) box: hit testing.
    pub tight: Rect,
    /// Unicode scalar value (0: none / second UTF-16 half).
    pub codepoint: u32,
    pub kind: GlyphKind,
}

/// A visual run of glyphs (one text object, or one line by geometry).
#[derive(Clone, Copy, Debug, Default)]
pub struct Run {
    pub rect: Rect,
    pub char_start: u32,
    pub len: u32,
    pub font_size: f32,
}

/// Inclusive glyph index range on one page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slice {
    pub from: usize,
    pub to: usize,
}

/// Text geometry of one page: glyphs plus visual runs.
#[derive(Clone, Debug, Default)]
pub struct PageText {
    pub glyphs: Vec<Glyph>,
    pub runs: Vec<Run>,
}

impl PageText {
    /// Builds glyphs and visual runs from the native glyph stream
    /// (EmbedPDF `buildRunsFromGlyphs`). Backends that report text objects
    /// (`run >= 0`, PDFium) group by object; others group by geometry.
    pub fn build(raw: &[native::Glyph]) -> PageText {
        let glyphs: Vec<Glyph> = raw
            .iter()
            .map(|g| Glyph {
                r#box: Rect {
                    x: g.x,
                    y: g.y,
                    width: g.width,
                    height: g.height,
                },
                tight: Rect {
                    x: g.tight_x,
                    y: g.tight_y,
                    width: g.tight_width,
                    height: g.tight_height,
                },
                codepoint: if g.codepoint <= 0x10FFFF {
                    g.codepoint
                } else {
                    0xFFFD
                },
                kind: match g.flags {
                    1 => GlyphKind::Space,
                    2 => GlyphKind::Empty,
                    _ => GlyphKind::Text,
                },
            })
            .collect();
        let mut runs: Vec<Run> = Vec::new();
        let mut cur: Option<usize> = None;
        let mut cur_obj: i32 = i32::MIN;
        let mut has_box = false;
        for (i, (g, glyph)) in raw.iter().zip(&glyphs).enumerate() {
            let start_new = 'blk: {
                let Some(c) = cur else { break 'blk true };
                if g.run >= 0 {
                    break 'blk g.run != cur_obj;
                }
                // geometry: a new visual line or a jump back starts a run
                if glyph.kind == GlyphKind::Empty || !has_box {
                    break 'blk false;
                }
                let r = runs[c].rect;
                if vertical_overlap(r, glyph.r#box) < 0.5 {
                    break 'blk true;
                }
                let avg = r.width / (runs[c].len.max(1) as f32);
                if glyph.r#box.x + avg * 0.5 < r.x {
                    break 'blk true;
                }
                if g.font_size > 0.0
                    && runs[c].font_size > 0.0
                    && g.font_size.max(runs[c].font_size) / g.font_size.min(runs[c].font_size) > 1.5
                {
                    break 'blk true;
                }
                break 'blk false;
            };
            if start_new {
                runs.push(Run {
                    rect: Rect::default(),
                    char_start: i as u32,
                    len: 0,
                    font_size: g.font_size,
                });
                cur = Some(runs.len() - 1);
                cur_obj = g.run;
                has_box = false;
            }
            let run = &mut runs[cur.expect("run")];
            run.len += 1;
            if glyph.kind == GlyphKind::Empty {
                continue;
            }
            run.rect = if has_box {
                run.rect.unite(glyph.r#box)
            } else {
                glyph.r#box
            };
            if run.font_size <= 0.0 {
                run.font_size = g.font_size;
            }
            has_box = true;
        }
        PageText { glyphs, runs }
    }

    pub fn count(&self) -> usize {
        self.glyphs.len()
    }

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

    fn tolerance(&self, factor: f32) -> f32 {
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

    fn is_boundary(&self, i: usize) -> bool {
        let g = self.glyphs[i];
        g.kind != GlyphKind::Text || is_punctuation_boundary(g.codepoint)
    }

    /// Word around glyph `i` (double click; EmbedPDF `expandToWordBoundary`).
    pub fn word_at(&self, i: usize) -> Option<Slice> {
        if i >= self.glyphs.len() {
            return None;
        }
        if self.is_boundary(i) {
            return Some(Slice { from: i, to: i });
        }
        let mut from = i;
        while from > 0 && !self.is_boundary(from - 1) {
            from -= 1;
        }
        let mut to = i;
        while to + 1 < self.glyphs.len() && !self.is_boundary(to + 1) {
            to += 1;
        }
        Some(Slice { from, to })
    }

    /// Visual line around glyph `i` (triple click; `expandToLineBoundary`).
    pub fn line_at(&self, i: usize) -> Option<Slice> {
        let r = self.run_of(i)?;
        let anchor = self.runs[r].rect;
        let mut from: usize = self.runs[r].char_start as usize;
        let mut to: usize = from + self.runs[r].len as usize - 1;
        let mut k = r;
        while k > 0 {
            k -= 1;
            let run = self.runs[k];
            if run.rect.is_empty() {
                continue;
            }
            if vertical_overlap(run.rect, anchor) < 0.5 {
                break;
            }
            from = run.char_start as usize;
        }
        k = r + 1;
        while k < self.runs.len() {
            let run = self.runs[k];
            if run.rect.is_empty() {
                k += 1;
                continue;
            }
            if vertical_overlap(run.rect, anchor) < 0.5 {
                break;
            }
            to = run.char_start as usize + run.len as usize - 1;
            k += 1;
        }
        Some(Slice { from, to })
    }

    fn run_of(&self, i: usize) -> Option<usize> {
        for (k, run) in self.runs.iter().enumerate() {
            let start = run.char_start as usize;
            if i >= start && i < start + run.len as usize {
                return Some(k);
            }
        }
        None
    }

    /// UTF-8 text of glyphs `from..to` (inclusive). Generated line breaks
    /// become "\n" (CR dropped, as on macOS).
    pub fn append_text(&self, out: &mut String, from: usize, to: usize) {
        if self.glyphs.is_empty() || from > to {
            return;
        }
        let end = to.min(self.glyphs.len() - 1);
        for i in from..=end {
            let cp = self.glyphs[i].codepoint;
            if cp == 0 || cp == u32::from(b'\r') {
                continue;
            }
            let Some(ch) = char::from_u32(cp) else {
                continue;
            };
            out.push(ch);
        }
    }

    /// Case-insensitive matches of `query` (whitespace runs match any
    /// whitespace, including line breaks). Appends inclusive glyph ranges.
    pub fn search(&self, query: &str, out: &mut Vec<Slice>) {
        let mut needle: Vec<u32> = Vec::new();
        let mut last_space = true;
        for ch in query.chars() {
            let space = is_space(ch as u32);
            if space && last_space {
                continue;
            }
            needle.push(if space {
                u32::from(b' ')
            } else {
                fold(ch as u32)
            });
            last_space = space;
        }
        while needle.last() == Some(&u32::from(b' ')) {
            needle.pop();
        }
        if needle.is_empty() {
            return;
        }
        let mut start: usize = 0;
        while start < self.glyphs.len() {
            if let Some(end) = self.matches_at(start, &needle) {
                out.push(Slice {
                    from: start,
                    to: end,
                });
                start = end + 1;
            } else {
                start += 1;
            }
        }
    }

    /// Last glyph of a match of `needle` starting at glyph `start`.
    fn matches_at(&self, start: usize, needle: &[u32]) -> Option<usize> {
        let first = self.glyphs[start].codepoint;
        if first == 0 || is_space(first) {
            return None;
        }
        let mut i = start;
        let mut k: usize = 0;
        while k < needle.len() {
            if i >= self.glyphs.len() {
                return None;
            }
            let cp = self.glyphs[i].codepoint;
            if cp == 0 {
                // second half of a UTF-16 pair
                i += 1;
                continue;
            }
            if needle[k] == u32::from(b' ') {
                if !is_space(cp) {
                    return None;
                }
                while i < self.glyphs.len()
                    && (self.glyphs[i].codepoint == 0 || is_space(self.glyphs[i].codepoint))
                {
                    i += 1;
                }
                k += 1;
                continue;
            }
            if fold(cp) != needle[k] {
                return None;
            }
            i += 1;
            k += 1;
        }
        Some(i - 1)
    }
}

#[derive(Clone, Copy)]
struct Part {
    rect: Rect,
    chars: u32,
    font_size: f32,
}

fn should_merge(a: Part, b: Part) -> bool {
    if a.font_size > 0.0
        && b.font_size > 0.0
        && a.font_size.max(b.font_size) / a.font_size.min(b.font_size) > 1.5
    {
        return false;
    }
    if vertical_overlap(a.rect, b.rect) < 0.8 {
        return false;
    }
    let aw = a.rect.width / a.chars as f32;
    let bw = b.rect.width / b.chars as f32;
    a.rect.x - aw < b.rect.right() + bw && a.rect.right() + aw > b.rect.x - bw
}

fn vertical_overlap(a: Rect, b: Rect) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let u = a.unite(b);
    if u.height == a.height || u.height == b.height {
        return 1.0;
    }
    a.intersect(b).height / u.height
}

fn is_space(cp: u32) -> bool {
    cp == u32::from(b' ')
        || cp == u32::from(b'\t')
        || cp == u32::from(b'\n')
        || cp == u32::from(b'\r')
        || cp == 0xA0
        || cp == 0x3000
}

/// Word boundaries besides spaces (PDFium `IsWordBoundary` style):
/// `. , ; : ! ? ( ) [ ] { } " ' / \ < > |` and CJK punctuation.
fn is_punctuation_boundary(cp: u32) -> bool {
    matches!(
        cp,
        0x2E | 0x2C
            | 0x3B
            | 0x3A
            | 0x21
            | 0x3F
            | 0x28
            | 0x29
            | 0x5B
            | 0x5D
            | 0x7B
            | 0x7D
            | 0x22
            | 0x27
            | 0x2F
            | 0x5C
            | 0x3C
            | 0x3E
            | 0x7C
            | 0x3001
            | 0x3002
            | 0xFF0C
            | 0xFF0E
            | 0xFF1A
            | 0xFF1B
            | 0xFF01
            | 0xFF1F
    )
}

/// Simple case folding: ASCII, Latin-1, Greek and Cyrillic, full-width ASCII.
pub fn fold(cp: u32) -> u32 {
    match cp {
        0x41..=0x5A => cp + 32,
        0xC0..=0xD6 | 0xD8..=0xDE => cp + 32,
        0x391..=0x3A1 | 0x3A3..=0x3AB => cp + 32,
        0x410..=0x42F => cp + 32,
        0x400..=0x40F => cp + 80,
        0xFF21..=0xFF3A => cp - 0xFF21 + 0x61,
        0xFF41..=0xFF5A => cp - 0xFF41 + 0x61,
        0xFF10..=0xFF19 => cp - 0xFF10 + 0x30,
        _ => cp,
    }
}
