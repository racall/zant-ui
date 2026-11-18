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

mod hit;
mod search;
mod words;

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
