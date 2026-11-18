//! Continuous scrolling and page layout of the PDF viewer: page tops,
//! navigation and the toolbar chrome.
//!
//! Page tops are computed in content pixels at `layout_zoom`; the viewport
//! scrolls vertically through the stacked pages, and horizontally only when a
//! page is wider than the viewport (then the pages are centred on the widest
//! one, like EmbedPDF's scroll plugin). The toolbar (`toolbar`), its icon
//! buttons and its page / zoom / search actions live here as well: the viewer
//! draws its own chrome with the ordinary `ui::*` builders.

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonSize, ButtonType, Icon};
use crate::flex;
use crate::theme::tokens::Theme;
use crate::ui::actions::Action;
use crate::ui::builders::{button, icon_button, input, row, space, text, tooltip};
use crate::ui::element::Node;
use crate::ui::options::{
    BoxOptions, ButtonOptions, IconButtonOptions, InputOptions, SpaceOptions, TextOptions,
    TooltipOptions,
};
use crate::ui::{Style, styled};

use super::{BarDrag, Fit, Inner, ViewOptions, Viewer};

mod toolbar;

pub(crate) use self::toolbar::*;

impl Inner {
    /// Recomputes the page tops and the content box at `zoom`.
    pub(crate) fn relayout(&mut self) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        let count = doc.page_count();
        self.tops.clear();
        self.tops.resize(count, 0.0);
        let mut y = self.padding;
        let mut max_w: f32 = 0.0;
        for (i, top) in self.tops.iter_mut().enumerate() {
            let size = doc.page_size(i);
            *top = y;
            y += size.height * self.zoom + self.page_gap;
            max_w = max_w.max(size.width * self.zoom);
        }
        self.max_page_w = max_w;
        self.content_h = if count > 0 {
            y - self.page_gap + self.padding
        } else {
            0.0
        };
        self.content_w = max_w + self.padding * 2.0;
        self.layout_zoom = self.zoom;
    }

    pub(crate) fn max_scroll_x(&self) -> f32 {
        (self.content_w - self.view_w).max(0.0)
    }

    pub(crate) fn max_scroll_y(&self) -> f32 {
        (self.content_h - self.view_h).max(0.0)
    }

    pub(crate) fn clamp_scroll(&mut self) {
        self.scroll_x = self.scroll_x.clamp(0.0, self.max_scroll_x());
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll_y());
    }

    pub(crate) fn page_size_px(&self, page: usize) -> [f32; 2] {
        let size = self.doc.as_ref().expect("open document").page_size(page);
        [size.width * self.zoom, size.height * self.zoom]
    }

    /// Top-left of a page in viewport coordinates.
    pub(crate) fn page_origin(&self, page: usize) -> [f32; 2] {
        let size = self.page_size_px(page);
        let x = if self.content_w <= self.view_w {
            (self.view_w - size[0]) / 2.0
        } else {
            self.padding + (self.max_page_w - size[0]) / 2.0 - self.scroll_x
        };
        [x, self.tops[page] - self.scroll_y]
    }

    /// Page whose vertical span (with half the gap) contains viewport y.
    pub(crate) fn page_near_y(&self, vy: f32) -> usize {
        let y = vy + self.scroll_y;
        let n = self.tops.len();
        if n == 0 {
            return 0;
        }
        let (mut lo, mut hi) = (0usize, n);
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.tops[mid] - self.page_gap / 2.0 <= y {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The page near the top third of the viewport becomes `current_page`.
    pub(crate) fn update_current_page(&mut self) {
        if self.tops.is_empty() {
            return;
        }
        let page = self.page_near_y(self.view_h / 3.0);
        if page != self.current_page {
            self.current_page = page;
            self.page_input_page = None;
        }
    }

    /// Scrolls to a page (0-based); `y` is a point offset from its top.
    pub(crate) fn go_to_page(&mut self, page: usize, y: f32) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        if doc.page_count() == 0 {
            return;
        }
        let p = page.min(doc.page_count() - 1);
        if self.tops.len() != doc.page_count() || self.view_h <= 0.0 {
            self.pending_goto = Some((p, y));
            return;
        }
        self.scroll_y = self.tops[p] + y * self.zoom - self.padding;
        self.clamp_scroll();
        self.current_page = p;
        self.page_input_page = None;
    }

    pub(crate) fn next_page(&mut self) {
        if self.current_page + 1 < self.page_count() {
            self.go_to_page(self.current_page + 1, 0.0);
        }
    }

    pub(crate) fn previous_page(&mut self) {
        if self.current_page > 0 {
            self.go_to_page(self.current_page - 1, 0.0);
        }
    }

    pub(crate) fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.scroll_x += dx;
        self.scroll_y += dy;
        self.clamp_scroll();
        self.update_current_page();
    }

    pub(crate) fn page_count(&self) -> usize {
        self.doc.as_ref().map_or(0, |doc| doc.page_count())
    }

    /// Scrollbar drag; `true` while dragging.
    pub(crate) fn on_scrollbar(
        &mut self,
        info: crate::ui::actions::PointerInfo,
        horizontal: bool,
    ) -> bool {
        let track = if horizontal { self.view_w } else { self.view_h };
        let content = if horizontal {
            self.content_w
        } else {
            self.content_h
        };
        if content <= track {
            return false;
        }
        let pos = if horizontal {
            info.window_x
        } else {
            info.window_y
        };
        match info.phase {
            crate::ui::actions::PointerPhase::Down => {
                self.bar_drag = Some(BarDrag {
                    start: pos,
                    scroll: if horizontal {
                        self.scroll_x
                    } else {
                        self.scroll_y
                    },
                    horizontal,
                })
            }
            crate::ui::actions::PointerPhase::Drag => {
                let Some(drag) = self.bar_drag else {
                    return false;
                };
                let ratio = content / track;
                if horizontal {
                    self.scroll_x = drag.scroll + (pos - drag.start) * ratio;
                } else {
                    self.scroll_y = drag.scroll + (pos - drag.start) * ratio;
                }
                self.clamp_scroll();
                self.update_current_page();
            }
            crate::ui::actions::PointerPhase::Up => self.bar_drag = None,
            _ => return false,
        }
        true
    }

    /// Vertical scrollbar drag.
    pub(crate) fn on_vbar(&mut self, info: crate::ui::actions::PointerInfo) -> bool {
        self.on_scrollbar(info, false)
    }

    /// Horizontal scrollbar drag.
    pub(crate) fn on_hbar(&mut self, info: crate::ui::actions::PointerInfo) -> bool {
        self.on_scrollbar(info, true)
    }
}

impl Viewer {
    /// Number of pages of the open document.
    pub fn page_count(&self) -> usize {
        self.borrow().page_count()
    }

    /// Current page (0-based), the page near the top third of the viewport.
    pub fn current_page(&self) -> usize {
        self.borrow().current_page
    }

    /// Scrolls to a page (0-based); `y` is a point offset from its top.
    pub fn go_to_page(&self, page: usize, y: f32) {
        self.borrow_mut().go_to_page(page, y);
    }

    pub fn next_page(&self) {
        self.borrow_mut().next_page();
    }

    pub fn previous_page(&self) {
        self.borrow_mut().previous_page();
    }

    /// Scroll offset of the viewport (content px).
    pub fn scroll(&self) -> [f32; 2] {
        let inner = self.borrow();
        [inner.scroll_x, inner.scroll_y]
    }

    /// Moves the viewport to a scroll offset (content px, clamped to the
    /// content). `on_change` reports the move like a user scroll.
    pub fn scroll_to(&self, x: f32, y: f32) {
        let mut inner = self.borrow_mut();
        let (dx, dy) = (x - inner.scroll_x, y - inner.scroll_y);
        inner.scroll_by(dx, dy);
        inner.changed();
    }

    /// Size of the viewport (px); zero before the first layout.
    pub fn viewport_size(&self) -> [f32; 2] {
        let inner = self.borrow();
        [inner.view_w, inner.view_h]
    }

    /// Content size of the stacked pages (px at the current zoom).
    pub fn content_size(&self) -> [f32; 2] {
        let inner = self.borrow();
        [inner.content_w, inner.content_h]
    }

    /// The zoom mode (fit width / fit page / manual).
    pub fn fit(&self) -> Fit {
        self.borrow().fit
    }
}
