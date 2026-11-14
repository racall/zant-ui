//! Port of `src/pdf/annotations.zig` — WP-3.8.
//!
//! PDF annotations made in the viewer (EmbedPDF annotation plugin model):
//! text markup (highlight, underline, strikeout, squiggly) on selected text,
//! ink strokes, and sticky notes. The viewer keeps the list (with undo /
//! redo); apps read it to persist it and write it into a copy of the PDF with
//! `Viewer.saveAnnotated` (standard PDF annotations).
//!
//! Coordinates are points of the displayed page (top-left origin, y down).
//!
//! Ownership: Zig uses arena / allocator clones and an explicit `free`; Rust
//! annotations own their `Vec`s and `String`s (`Clone` replaces the custom
//! `clone`, `Drop` replaces `free`). `Store` calls that could only fail with
//! OOM (`!u64`, `!void`) return their value or `()` here.

pub use crate::pdf::text::Rect;

pub type Point = [f32; 2];

/// Annotation kind; the values are the native `ZANT_PDF_ANNOT_*` constants
/// (`native::annot_kind`), marshalled by the viewer's `save_annotated`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    #[default]
    Highlight = 0,
    Underline = 1,
    Strikeout = 2,
    Squiggly = 3,
    Ink = 4,
    Note = 5,
}

impl Kind {
    pub fn is_markup(self) -> bool {
        (self as u8) <= (Kind::Squiggly as u8)
    }
}

/// One annotation on a page.
#[derive(Clone, Debug)]
pub struct Annotation {
    pub id: u64,
    pub page: usize,
    pub kind: Kind,
    /// 0xAARRGGBB; alpha is the opacity.
    pub color: u32,
    /// Markup: one box per line. Note: the icon box.
    pub rects: Vec<Rect>,
    /// Ink strokes.
    pub strokes: Vec<Vec<Point>>,
    /// Ink stroke width (points).
    pub width: f32,
    /// Comment / note text.
    pub contents: String,
    /// Text under a markup annotation (for lists and search).
    pub text: String,
}

impl Default for Annotation {
    fn default() -> Annotation {
        Annotation {
            id: 0,
            page: 0,
            kind: Kind::Highlight,
            color: 0,
            rects: Vec::new(),
            strokes: Vec::new(),
            // Zig: `width: f32 = 2`
            width: 2.0,
            contents: String::new(),
            text: String::new(),
        }
    }
}

impl Annotation {
    /// Bounding box on the page.
    pub fn bounds(&self) -> Rect {
        let mut out: Option<Rect> = None;
        for r in &self.rects {
            out = Some(match out {
                Some(o) => o.unite(*r),
                None => *r,
            });
        }
        for stroke in &self.strokes {
            for p in stroke {
                let dot = Rect {
                    x: p[0] - self.width / 2.0,
                    y: p[1] - self.width / 2.0,
                    width: self.width,
                    height: self.width,
                };
                out = Some(match out {
                    Some(o) => o.unite(dot),
                    None => dot,
                });
            }
        }
        out.unwrap_or_default()
    }

    /// True when (x, y) (page points) is on the annotation.
    pub fn hit(&self, x: f32, y: f32) -> bool {
        if self.kind == Kind::Ink {
            let tolerance = 4.0f32.max(self.width * 2.0);
            for stroke in &self.strokes {
                let mut i: usize = 0;
                while i + 1 < stroke.len() {
                    if segment_distance(x, y, stroke[i], stroke[i + 1]) <= tolerance {
                        return true;
                    }
                    i += 1;
                }
                if stroke.len() == 1
                    && (stroke[0][0] - x).abs() + (stroke[0][1] - y).abs() <= tolerance
                {
                    return true;
                }
            }
            return false;
        }
        for r in &self.rects {
            if r.contains(x, y) {
                return true;
            }
        }
        false
    }
}

fn drop_stack(stack: &mut Vec<Vec<Annotation>>) {
    stack.clear();
}

fn segment_distance(x: f32, y: f32, a: Point, b: Point) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((x - a[0]) * dx + (y - a[1]) * dy) / len2).clamp(0.0, 1.0)
    };
    let px = a[0] + t * dx - x;
    let py = a[1] + t * dy - y;
    (px * px + py * py).sqrt()
}

/// Owned annotation list with snapshot undo / redo (EmbedPDF history
/// plugin: every change is one step).
#[derive(Debug)]
pub struct Store {
    items: Vec<Annotation>,
    undo_stack: Vec<Vec<Annotation>>,
    redo_stack: Vec<Vec<Annotation>>,
    next_id: u64,
    /// Bumped on every change (apps compare it to know when to save).
    pub revision: u64,
}

impl Default for Store {
    fn default() -> Store {
        Store::new()
    }
}

impl Store {
    pub fn new() -> Store {
        Store {
            items: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_id: 1,
            revision: 0,
        }
    }

    fn clear_items(&mut self) {
        self.items.clear();
    }

    fn snapshot(&self) -> Vec<Annotation> {
        self.items.clone()
    }

    fn restore(&mut self, snap: &[Annotation]) {
        self.clear_items();
        self.items.extend(snap.iter().cloned());
        self.revision += 1;
    }

    /// Records the current list as an undo step (before a change).
    fn checkpoint(&mut self) {
        self.undo_stack.push(self.snapshot());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        drop_stack(&mut self.redo_stack);
    }

    pub fn all(&self) -> &[Annotation] {
        &self.items
    }

    pub fn get(&self, id: u64) -> Option<&Annotation> {
        self.items.iter().find(|a| a.id == id)
    }

    /// Adds `annotation` (its id is assigned); returns the id.
    pub fn add(&mut self, annotation: Annotation) -> u64 {
        self.checkpoint();
        let mut copy = annotation;
        copy.id = self.next_id;
        self.next_id += 1;
        let id = copy.id;
        self.items.push(copy);
        self.revision += 1;
        id
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let Some(index) = self.items.iter().position(|a| a.id == id) else {
            return false;
        };
        self.checkpoint();
        self.items.remove(index);
        self.revision += 1;
        true
    }

    pub fn set_contents(&mut self, id: u64, contents: &str) {
        let Some(index) = self.items.iter().position(|a| a.id == id) else {
            return;
        };
        if self.items[index].contents == contents {
            return;
        }
        self.checkpoint();
        self.items[index].contents = contents.to_string();
        self.revision += 1;
    }

    pub fn set_color(&mut self, id: u64, color: u32) {
        let Some(index) = self.items.iter().position(|a| a.id == id) else {
            return;
        };
        if self.items[index].color == color {
            return;
        }
        self.checkpoint();
        self.items[index].color = color;
        self.revision += 1;
    }

    /// Replaces the whole list (loading saved annotations); clears history.
    pub fn load(&mut self, list: &[Annotation]) {
        self.clear_items();
        drop_stack(&mut self.undo_stack);
        drop_stack(&mut self.redo_stack);
        for annotation in list {
            self.items.push(annotation.clone());
            self.next_id = self.next_id.max(annotation.id + 1);
        }
        self.revision += 1;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.undo_stack.pop() else {
            return false;
        };
        let now = self.snapshot();
        self.redo_stack.push(now);
        self.restore(&snap);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.redo_stack.pop() else {
            return false;
        };
        let now = self.snapshot();
        self.undo_stack.push(now);
        self.restore(&snap);
        true
    }
}

/// Default colours (EmbedPDF / Acrobat-like palette).
pub mod colors {
    pub const YELLOW: u32 = 0x80ffcd45;
    pub const GREEN: u32 = 0x8057d975;
    pub const BLUE: u32 = 0x8052a8ff;
    pub const PINK: u32 = 0x80ff6b9e;
    pub const RED: u32 = 0xffe5484d;
    pub const INK: u32 = 0xffe5484d;
    pub const NOTE: u32 = 0xffffc53d;
}
