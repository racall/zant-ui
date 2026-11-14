//! Document statistics and outline.

use crate::markdown::ast;
use crate::markdown::scan;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Stats {
    /// Latin words + individual CJK characters (how 字数 is usually counted).
    pub words: usize,
    /// Non-whitespace characters.
    pub characters: usize,
    pub lines: usize,
    /// Estimated reading time (300 CJK chars or 200 words per minute).
    pub reading_minutes: usize,
}

pub fn compute(text: &str) -> Stats {
    let mut s = Stats::default();
    if text.is_empty() {
        return s;
    }
    s.lines = 1;
    let mut cjk: usize = 0;
    let mut latin_words: usize = 0;
    let mut in_word = false;
    for c in text.chars() {
        let cp = c as u32;
        if c == '\n' {
            s.lines += 1;
        }
        if scan::is_whitespace_cp(cp) {
            in_word = false;
            continue;
        }
        s.characters += 1;
        if scan::is_cjk(cp) {
            if !scan::is_unicode_punct(cp) {
                cjk += 1;
            }
            in_word = false;
        } else if scan::is_word_cp(cp) {
            if !in_word {
                latin_words += 1;
            }
            in_word = true;
        } else {
            in_word = false;
        }
    }
    s.words = cjk + latin_words;
    let minutes = cjk as f32 / 300.0 + latin_words as f32 / 200.0;
    s.reading_minutes = minutes.ceil() as usize;
    s
}

pub struct OutlineEntry {
    pub level: u8,
    pub text: String,
    pub id: String,
}

/// Headings of a document (for a table of contents).
pub fn outline(doc: &ast::Document) -> Vec<OutlineEntry> {
    let mut out = Vec::new();
    for block in &doc.blocks {
        if let ast::Block::Heading(h) = block {
            out.push(OutlineEntry {
                level: h.level,
                text: ast::plain_text(&h.spans),
                id: h.id.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_counts() {
        let s = compute("Hello world，你好世界。\nsecond line");
        assert_eq!(s.words, 4 + 4);
        assert_eq!(s.lines, 2);
    }
}
