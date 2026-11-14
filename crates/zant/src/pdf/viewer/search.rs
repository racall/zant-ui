//! Search of the PDF viewer — `src/pdf/viewer.zig`'s search section (WP-3.9).
//!
//! Results arrive incrementally: [`Inner::continue_search`] scans pages for at
//! most ~6 ms per build so a large document never blocks the UI, and the first
//! hit is revealed as soon as there is one. The toolbar shows the match count
//! and the app is notified through `on_change` when the search changes.

use super::{Inner, Match, Viewer};

impl Inner {
    /// Starts a search (empty clears it). Results arrive over the next
    /// builds (a few pages per frame); `matches` lists them.
    pub(crate) fn search(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query.clear();
        self.query.push_str(query);
        self.matches.clear();
        self.active_match = None;
        self.search_page = 0;
        self.search_done = query.is_empty();
    }

    pub(crate) fn search_results(&self) -> &[Match] {
        &self.matches
    }

    pub(crate) fn next_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.active_match = match self.active_match {
            Some(m) => Some((m + 1) % self.matches.len()),
            None => Some(self.first_match_from_current_page()),
        };
        self.reveal_match();
    }

    pub(crate) fn previous_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let n = self.matches.len();
        self.active_match = match self.active_match {
            Some(m) => Some((m + n - 1) % n),
            None => Some(self.first_match_from_current_page()),
        };
        self.reveal_match();
    }

    fn first_match_from_current_page(&self) -> usize {
        for (i, m) in self.matches.iter().enumerate() {
            if m.page >= self.current_page {
                return i;
            }
        }
        0
    }

    /// Searches pages for at most ~6 ms; true when more pages remain.
    pub(crate) fn continue_search(&mut self) -> bool {
        let Some(doc) = self.doc.clone() else {
            return false;
        };
        if self.search_done {
            return false;
        }
        let start = crate::platform::platform::monotonic_ms();
        let had = self.matches.len();
        while self.search_page < doc.page_count() {
            if let Some(text) = doc.page_text(self.search_page) {
                let mut found = Vec::new();
                text.search(&self.query, &mut found);
                for slice in found {
                    self.matches.push(Match {
                        page: self.search_page,
                        from: slice.from,
                        to: slice.to,
                    });
                }
            }
            self.search_page += 1;
            if crate::platform::platform::monotonic_ms() - start > 6 {
                break;
            }
        }
        if self.search_page >= doc.page_count() {
            self.search_done = true;
        }
        // jump to the first result as soon as there is one
        if had == 0 && !self.matches.is_empty() && self.active_match.is_none() {
            self.active_match = Some(self.first_match_from_current_page());
            self.reveal_match();
        }
        !self.search_done
    }

    fn reveal_match(&mut self) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        let Some(active) = self.active_match else {
            return;
        };
        let m = self.matches[active];
        let Some(text) = doc.page_text(m.page) else {
            return;
        };
        let rects = text.rects(m.from, m.to);
        if rects.is_empty() || self.tops.is_empty() {
            self.go_to_page(m.page, 0.0);
            return;
        }
        let r = rects[0];
        let origin = self.page_origin(m.page);
        let top = origin[1] + r.y * self.zoom;
        let left = origin[0] + r.x * self.zoom;
        // keep it visible with some room above (centre when far away)
        if top < 24.0 || top + r.height * self.zoom > self.view_h - 24.0 {
            self.scroll_y += top - self.view_h / 3.0;
        }
        if left < 16.0 || left + r.width * self.zoom > self.view_w - 16.0 {
            self.scroll_x += left - self.view_w / 3.0;
        }
        self.clamp_scroll();
        self.update_current_page();
    }
}

impl Viewer {
    /// Starts a search (empty clears it); results arrive over the next builds.
    pub fn search(&self, query: &str) {
        self.borrow_mut().search(query);
    }

    /// Hits found so far (all hits once the incremental search finished).
    pub fn search_results(&self) -> Vec<Match> {
        self.borrow().search_results().to_vec()
    }

    /// Index of the highlighted hit in [`Viewer::search_results`].
    pub fn active_match(&self) -> Option<usize> {
        self.borrow().active_match
    }

    pub fn next_match(&self) {
        self.borrow_mut().next_match();
    }

    pub fn previous_match(&self) {
        self.borrow_mut().previous_match();
    }

    /// True once the incremental search scanned every page.
    pub fn search_done(&self) -> bool {
        self.borrow().search_done
    }

    /// The query of the last search.
    pub fn query(&self) -> String {
        self.borrow().query.clone()
    }
}
