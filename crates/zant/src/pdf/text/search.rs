//! Searching the page text (case- and accent-folded).

use super::*;

impl PageText {
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
    pub(super) fn matches_at(&self, start: usize, needle: &[u32]) -> Option<usize> {
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
