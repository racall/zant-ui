//! Positions and links under the pointer, slices and extending the selection.

use super::*;

impl Inner {
    /// Glyph position under a viewport point: exact / tolerant hit on the page
    /// under the point, else the nearest glyph of the nearest page.
    pub(crate) fn pos_at(&self, vx: f32, vy: f32, exact: bool) -> Option<Pos> {
        let doc = self.doc.clone()?;
        if self.tops.is_empty() {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        let px = (vx - origin[0]) / self.zoom;
        let py = (vy - origin[1]) / self.zoom;
        let text = doc.page_text(page)?;
        if let Some(index) = text.glyph_at(px, py, 1.5) {
            return Some(Pos { page, index });
        }
        if exact {
            return None;
        }
        text.nearest_glyph(px, py).map(|index| Pos { page, index })
    }

    pub(crate) fn link_at(&self, vx: f32, vy: f32) -> Option<Link> {
        let doc = self.doc.clone()?;
        if self.tops.is_empty() {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        let px = (vx - origin[0]) / self.zoom;
        let py = (vy - origin[1]) / self.zoom;
        doc.page_links(page)
            .iter()
            .find(|link| link.rect.contains(px, py))
            .cloned()
    }

    pub(super) fn slice_for(&self, pos: Pos, mode: DragMode) -> [Pos; 2] {
        let Some(text) = self.doc.as_ref().and_then(|doc| doc.page_text(pos.page)) else {
            return [pos, pos];
        };
        let slice = match mode {
            DragMode::Char => return [pos, pos],
            DragMode::Word => text.word_at(pos.index),
            DragMode::Line => text.line_at(pos.index),
        };
        let Some(slice) = slice else {
            return [pos, pos];
        };
        [
            Pos {
                page: pos.page,
                index: slice.from,
            },
            Pos {
                page: pos.page,
                index: slice.to,
            },
        ]
    }

    pub(super) fn extend_to(&mut self, pos: Pos) {
        let mode = self.drag.unwrap_or(DragMode::Char);
        if mode == DragMode::Char || self.anchor_slice.is_none() {
            self.focus = Some(pos);
            return;
        }
        // word / line granularity: grow from the anchored word or line
        let a = self.anchor_slice.expect("anchor slice");
        let b = self.slice_for(pos, mode);
        if Pos::less(b[0], a[0]) {
            self.anchor = Some(a[1]);
            self.focus = Some(b[0]);
        } else {
            self.anchor = Some(a[0]);
            self.focus = Some(if Pos::less(b[1], a[1]) { a[1] } else { b[1] });
        }
    }

    /// Page and page point under a viewport point (nearest page by y).
    pub(crate) fn page_point(&self, vx: f32, vy: f32) -> Option<PagePoint> {
        if self.tops.is_empty() {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        Some(PagePoint {
            page,
            x: (vx - origin[0]) / self.zoom,
            y: (vy - origin[1]) / self.zoom,
        })
    }
}
