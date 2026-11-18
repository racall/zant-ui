//! The text selection: reading, setting, clearing, select all, the selected text and copying it.

use super::*;

impl Inner {
    /// The selected glyph range, ordered (`None` when nothing is selected).
    pub(crate) fn selection(&self) -> Option<Range> {
        let a = self.anchor?;
        let f = self.focus?;
        Some(if Pos::less(f, a) {
            Range { start: f, end: a }
        } else {
            Range { start: a, end: f }
        })
    }

    pub(crate) fn set_selection(&mut self, start: Pos, end: Pos) {
        self.anchor = Some(start);
        self.focus = Some(end);
        self.anchor_slice = None;
    }

    pub(crate) fn clear_selection(&mut self) {
        self.anchor = None;
        self.focus = None;
        self.anchor_slice = None;
    }

    /// Selects every page's text.
    pub(crate) fn select_all(&mut self) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        let n = doc.page_count();
        if n == 0 {
            return;
        }
        let mut last = n;
        while last > 0 {
            last -= 1;
            let Some(text) = doc.page_text(last) else {
                continue;
            };
            if text.count() == 0 {
                continue;
            }
            self.anchor = Some(Pos { page: 0, index: 0 });
            self.focus = Some(Pos {
                page: last,
                index: text.count() - 1,
            });
            self.anchor_slice = None;
            return;
        }
    }

    /// UTF-8 text of the selection (pages separated by a line break).
    pub(crate) fn selected_text(&self) -> String {
        let mut out = String::new();
        let Some(doc) = self.doc.clone() else {
            return out;
        };
        let Some(range) = self.selection() else {
            return out;
        };
        let mut page = range.start.page;
        while page <= range.end.page {
            if let Some(text) = doc.page_text(page)
                && text.count() > 0
            {
                let from = if page == range.start.page {
                    range.start.index
                } else {
                    0
                };
                let to = if page == range.end.page {
                    range.end.index
                } else {
                    text.count() - 1
                };
                if page != range.start.page && !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                text.append_text(&mut out, from, to.min(text.count() - 1));
            }
            page += 1;
        }
        out
    }

    /// Copies the selection to the clipboard; false when nothing is selected.
    pub(crate) fn copy_selection(&self) -> bool {
        let value = self.selected_text();
        if value.is_empty() {
            return false;
        }
        crate::platform::platform::set_clipboard(&value)
    }
}
