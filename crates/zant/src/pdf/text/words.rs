//! Word and line ranges, runs and the copied text of a range.

use super::*;

impl PageText {
    pub(super) fn is_boundary(&self, i: usize) -> bool {
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

    pub(super) fn run_of(&self, i: usize) -> Option<usize> {
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
}
