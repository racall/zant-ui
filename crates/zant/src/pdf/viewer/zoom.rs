//! Zoom of the PDF viewer.
//!
//! Fit width / fit page recompute the zoom from the viewport and the largest
//! page; manual zoom steps through EmbedPDF's presets. A zoom change keeps the
//! page point under a viewport point (pointer for ⌘/Ctrl + wheel and pinch,
//! viewport centre for the buttons) in place.

use super::{Anchor, Fit, Inner, MAX_ZOOM, MIN_ZOOM, Viewer, ZOOM_STEPS};

impl Inner {
    /// Zoom of a fit mode, or `None` for manual zoom / no viewport.
    pub(crate) fn fit_zoom(&self, fit: Fit) -> Option<f32> {
        let doc = self.doc.as_ref()?;
        if self.view_w <= 0.0 || doc.page_count() == 0 {
            return None;
        }
        let mut max_w: f32 = 1.0;
        let mut max_h: f32 = 1.0;
        for page in 0..doc.page_count() {
            let size = doc.page_size(page);
            max_w = max_w.max(size.width);
            max_h = max_h.max(size.height);
        }
        let avail_w = (self.view_w - self.padding * 2.0).max(40.0);
        let by_width = avail_w / max_w;
        match fit {
            Fit::Width => Some(by_width.clamp(MIN_ZOOM, MAX_ZOOM)),
            Fit::Page => Some(
                by_width
                    .min((self.view_h - self.padding * 2.0).max(40.0) / max_h)
                    .clamp(MIN_ZOOM, MAX_ZOOM),
            ),
            Fit::None => None,
        }
    }

    /// The page point under the viewport centre (kept in place across zoom).
    pub(crate) fn anchor_center(&mut self) -> Option<Anchor> {
        self.anchor_at(self.view_w / 2.0, self.view_h / 2.0)
    }

    /// The page point under viewport point (vx, vy), to keep it in place
    /// across a zoom change.
    pub(crate) fn anchor_at(&self, vx: f32, vy: f32) -> Option<Anchor> {
        let doc = self.doc.as_ref()?;
        if self.tops.len() != doc.page_count() || self.layout_zoom <= 0.0 {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        Some(Anchor {
            page,
            px: (vx - origin[0]) / self.layout_zoom,
            py: (vy - origin[1]) / self.layout_zoom,
            vx,
            vy,
        })
    }

    pub(crate) fn apply_fit(&mut self, anchor: Option<Anchor>) {
        let Some(zoom) = self.fit_zoom(self.fit) else {
            return;
        };
        self.set_zoom_raw(zoom, anchor);
    }

    /// Zooms keeping the page point under (vx, vy) fixed (pinch / ⌘ wheel).
    pub(crate) fn zoom_at(&mut self, zoom: f32, vx: f32, vy: f32) {
        self.set_zoom_raw(zoom.clamp(MIN_ZOOM, MAX_ZOOM), self.anchor_at(vx, vy));
    }

    pub(crate) fn set_zoom_raw(&mut self, zoom: f32, anchor: Option<Anchor>) {
        if (zoom - self.zoom).abs() < 0.0001 && self.layout_zoom == zoom {
            return;
        }
        self.zoom = zoom;
        self.relayout();
        if let Some(a) = anchor {
            // place the anchored page point back under the same viewport point
            let origin = self.page_origin(a.page);
            self.scroll_x += origin[0] + a.px * self.zoom - a.vx;
            self.scroll_y += origin[1] + a.py * self.zoom - a.vy;
        }
        self.clamp_scroll();
        self.update_current_page();
    }

    /// Manual zoom (1 = 100%), keeping the viewport centre in place.
    pub(crate) fn set_zoom(&mut self, zoom: f32) {
        self.fit = Fit::None;
        self.zoom_at(zoom, self.view_w / 2.0, self.view_h / 2.0);
    }

    pub(crate) fn zoom_in(&mut self) {
        for z in ZOOM_STEPS {
            if z > self.zoom + 0.001 {
                self.set_zoom(z);
                return;
            }
        }
        self.set_zoom(MAX_ZOOM);
    }

    pub(crate) fn zoom_out(&mut self) {
        let mut i = ZOOM_STEPS.len();
        while i > 0 {
            i -= 1;
            if ZOOM_STEPS[i] < self.zoom - 0.001 {
                self.set_zoom(ZOOM_STEPS[i]);
                return;
            }
        }
        self.set_zoom(MIN_ZOOM);
    }

    pub(crate) fn set_fit_mode(&mut self, fit: Fit) {
        self.fit = fit;
        if fit != Fit::None {
            let anchor = self.anchor_center();
            self.apply_fit(anchor);
        }
    }
}

impl Viewer {
    /// Current zoom (1 = 100%).
    pub fn zoom(&self) -> f32 {
        self.borrow().zoom
    }

    /// Sets the fit mode; `Fit::None` keeps the manual zoom.
    pub fn set_fit(&self, fit: Fit) {
        self.borrow_mut().set_fit_mode(fit);
    }

    /// Sets a manual zoom (1 = 100%), keeping the viewport centre in place.
    pub fn set_zoom(&self, zoom: f32) {
        self.borrow_mut().set_zoom(zoom);
    }

    pub fn zoom_in(&self) {
        self.borrow_mut().zoom_in();
    }

    pub fn zoom_out(&self) {
        self.borrow_mut().zoom_out();
    }
}
