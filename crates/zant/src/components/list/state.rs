//! List state, height bookkeeping, and scroll handling.

use crate::Str;
use crate::core::events::{Key, KeyEvent};
use crate::core::geometry::{Edges, Size};
use crate::ui::actions::{Action, PointerInfo, PointerPhase};
use crate::ui::context::frame_string;

use super::{BarDrag, List, ListState, PendingScroll, ScrollPosition};

impl ListState {
    /// Creates an empty state.
    pub fn new() -> ListState {
        ListState {
            heights: Vec::new(),
            fen_sum: Vec::new(),
            fen_cnt: Vec::new(),
            measured_total: 0.0,
            measured_count: 0,
            anchor: 0,
            offset: 0.0,
            following: true,
            viewport: Size::default(),
            rendered_first: 0,
            rendered_end: 0,
            uniform: None,
            estimate_hint: 120.0,
            gap: 0.0,
            padding: Edges::default(),
            pending_scroll: None,
            followed_key: 0,
            followed_index: None,
            bar_drag: None,
            options_key: Str::default(),
            item_id: None,
            on_scroll: None,
        }
    }

    pub fn count(&self) -> usize {
        self.heights.len()
    }

    // ---- heights ------------------------------------------------------

    fn rebuild_fenwick(&mut self) {
        let n = self.heights.len();
        self.fen_sum.resize(n, 0.0);
        self.fen_cnt.resize(n, 0);
        self.fen_sum.fill(0.0);
        self.fen_cnt.fill(0);
        self.measured_total = 0.0;
        self.measured_count = 0;
        let mut i = 0;
        while i < n {
            let h = self.heights[i];
            if h >= 0.0 {
                self.fen_add(i, f64::from(h), 1);
            }
            i += 1;
        }
    }

    /// Adds `dh` / `dc` for `index` into both Fenwick trees and
    /// the measured totals.
    fn fen_add(&mut self, index: usize, dh: f64, dc: i32) {
        self.measured_total += dh;
        self.measured_count =
            usize::try_from(i64::try_from(self.measured_count).expect("int cast") + i64::from(dc))
                .expect("int cast");
        let len = self.fen_sum.len();
        let mut i = index + 1;
        while i <= len {
            self.fen_sum[i - 1] += dh;
            self.fen_cnt[i - 1] =
                u32::try_from(i64::from(self.fen_cnt[i - 1]) + i64::from(dc)).expect("int cast");
            // Advance by the lowest set bit.
            i += i & i.wrapping_neg();
        }
    }

    /// `(sum of measured heights, number measured)` of items `[0, n)`.
    fn fen_prefix(&self, n: usize) -> Prefix {
        let mut sum: f64 = 0.0;
        let mut cnt: usize = 0;
        let mut i = n.min(self.fen_sum.len());
        while i > 0 {
            sum += self.fen_sum[i - 1];
            cnt += self.fen_cnt[i - 1] as usize;
            i -= i & i.wrapping_neg();
        }
        Prefix { sum, cnt }
    }

    /// Height assumed for unmeasured items.
    pub fn estimate(&self) -> f32 {
        if let Some(h) = self.uniform {
            return h;
        }
        if self.measured_count == 0 {
            return self.estimate_hint;
        }
        (self.measured_total / self.measured_count as f64) as f32
    }

    pub fn item_height(&self, index: usize) -> f32 {
        if let Some(h) = self.uniform {
            return h;
        }
        let h = self.heights[index];
        if h >= 0.0 { h } else { self.estimate() }
    }

    pub(super) fn set_measured(&mut self, index: usize, h: f32) {
        if index >= self.heights.len() {
            return;
        }
        let old = self.heights[index];
        if old >= 0.0 && (old - h).abs() < 0.25 {
            return;
        }
        if old >= 0.0 {
            self.fen_add(index, f64::from(h - old), 0);
        } else {
            self.fen_add(index, f64::from(h), 1);
        }
        self.heights[index] = h;
    }

    /// Top of item `index` in content coordinates (padding included).
    pub fn item_top(&self, index: usize) -> f32 {
        let n = index.min(self.count());
        if let Some(h) = self.uniform {
            return self.padding.top + n as f32 * (h + self.gap);
        }
        let p = self.fen_prefix(n);
        let est = self.estimate();
        self.padding.top + p.sum as f32 + (n - p.cnt) as f32 * est + n as f32 * self.gap
    }

    pub fn content_height(&self) -> f32 {
        let n = self.count();
        if n == 0 {
            return self.padding.vertical();
        }
        self.item_top(n) - self.gap + self.padding.bottom
    }

    /// Item at content y (binary search on the prefix sums).
    pub(super) fn index_at(&self, y: f32) -> usize {
        let n = self.count();
        if n == 0 {
            return 0;
        }
        let mut lo: usize = 0;
        let mut hi: usize = n;
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.item_top(mid) <= y {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    // ---- mutations ------------------------------------------------------

    /// Matches the item count (appends unmeasured items, drops trailing ones).
    pub fn set_count(&mut self, n: usize) {
        let old = self.count();
        if n == old {
            return;
        }
        if n > old {
            self.heights.resize(n, -1.0);
        } else {
            self.heights.truncate(n);
            if self.anchor >= n {
                self.anchor = if n > 0 { n - 1 } else { 0 };
                self.offset = 0.0;
            }
        }
        self.rebuild_fenwick();
    }

    /// `removed` items at `at` were replaced by `inserted` new ones.
    pub fn splice(&mut self, at: usize, removed: usize, inserted: usize) {
        let n = self.count();
        let start = at.min(n);
        let end = (start + removed).min(n);
        self.heights
            .splice(start..end, std::iter::repeat_n(-1.0, inserted));
        if self.anchor >= end {
            self.anchor = self.anchor - (end - start) + inserted;
        } else if self.anchor >= start {
            self.anchor = start;
            self.offset = 0.0;
        }
        if self.anchor >= self.count() && self.count() > 0 {
            self.anchor = self.count() - 1;
        }
        self.rebuild_fenwick();
    }

    /// Item `index` must be measured again (its content changed).
    pub fn invalidate(&mut self, index: usize) {
        if index >= self.count() || self.heights[index] < 0.0 {
            return;
        }
        self.fen_add(index, -f64::from(self.heights[index]), -1);
        self.heights[index] = -1.0;
    }

    /// Every item must be measured again (e.g. the width or font changed).
    pub fn invalidate_all(&mut self) {
        self.heights.fill(-1.0);
        self.rebuild_fenwick();
    }

    // ---- scrolling ------------------------------------------------------

    pub fn scroll_top(&self) -> f32 {
        if self.count() == 0 {
            return 0.0;
        }
        self.item_top(self.anchor) + self.offset
    }

    pub(super) fn max_scroll_top(&self) -> f32 {
        (self.content_height() - self.viewport.height).max(0.0)
    }

    pub(super) fn set_scroll_top(&mut self, top: f32) {
        let t = top.clamp(0.0, self.max_scroll_top());
        if self.count() == 0 {
            self.anchor = 0;
            self.offset = 0.0;
            return;
        }
        self.anchor = self.index_at(t);
        self.offset = t - self.item_top(self.anchor);
    }

    pub fn is_at_end(&self) -> bool {
        self.scroll_top() >= self.max_scroll_top() - 1.0
    }

    /// Scrolls by `dy` px (positive = towards the end).
    pub fn scroll_by(&mut self, dy: f32) {
        self.set_scroll_top(self.scroll_top() + dy);
        self.following = self.is_at_end();
    }

    /// Brings item `index` into view at the next build.
    pub fn scroll_to_item(&mut self, index: usize, position: ScrollPosition) {
        self.pending_scroll = Some(PendingScroll { index, position });
        self.following = false;
    }

    /// Declarative scrolling (TSX `<List scrollToIndex layoutKey>`): brings
    /// `index` into view when it changed since the last call; a new
    /// `layout_key` (the rows changed: a search, a delete) re-measures every
    /// row and applies `index` again.
    pub fn follow(&mut self, layout_key: u64, index: Option<usize>, position: ScrollPosition) {
        let mut again = false;
        if layout_key != self.followed_key {
            self.followed_key = layout_key;
            self.invalidate_all();
            again = true;
        }
        let changed = match index {
            Some(i) => self.followed_index.is_none_or(|previous| previous != i),
            None => false,
        };
        self.followed_index = index;
        if let Some(i) = index
            && (changed || again)
        {
            self.scroll_to_item(i, position);
        }
    }

    /// Scrolls to the end and resumes following the tail.
    pub fn scroll_to_end(&mut self) {
        self.following = true;
        self.pending_scroll = None;
    }

    /// Indices built in the last frame `[first, end)` (visible + overdraw).
    pub fn rendered_range(&self) -> [usize; 2] {
        [self.rendered_first, self.rendered_end]
    }

    pub(super) fn item_key(&self, index: usize) -> Str {
        if let Some(item_id) = &self.item_id {
            frame_string(&format!(
                "{}-k{}",
                self.options_key.as_str(),
                item_id(index)
            ))
        } else {
            frame_string(&format!("{}-i{}", self.options_key.as_str(), index))
        }
    }

    // ---- events ---------------------------------------------------------

    /// Wheel / trackpad scrolling (the list takes only the
    /// wheel; presses reach the rows).
    pub(super) fn on_pointer(&mut self, info: PointerInfo) -> Handled {
        match info.phase {
            PointerPhase::Scroll => {
                if info.dy.abs() < 0.01 {
                    return Handled::ignored();
                }
                self.scroll_by(-info.dy);
                Handled::new(true, self.on_scroll.clone())
            }
            _ => Handled::ignored(),
        }
    }

    /// Keyboard scrolling: Up / Down / Page Up / Page Down / Home / End / Space.
    pub(super) fn on_key(&mut self, k: KeyEvent) -> Handled {
        let page = (self.viewport.height * 0.9).max(40.0);
        match k.key {
            Key::Up => self.scroll_by(-40.0),
            Key::Down => self.scroll_by(40.0),
            Key::PageUp => self.scroll_by(-page),
            Key::PageDown => self.scroll_by(page),
            Key::Home => self.set_scroll_top(0.0),
            Key::End => self.scroll_to_end(),
            Key::Character(c) => {
                if c != ' ' {
                    return Handled::ignored();
                }
                self.scroll_by(if k.modifiers.shift { -page } else { page });
            }
            _ => return Handled::ignored(),
        }
        if k.key == Key::Home {
            self.following = false;
        }
        Handled::new(true, self.on_scroll.clone())
    }

    /// Dragging the scroll-bar thumb.
    pub(super) fn on_bar(&mut self, info: PointerInfo) -> Handled {
        let content = self.content_height();
        if content <= self.viewport.height {
            return Handled::ignored();
        }
        match info.phase {
            PointerPhase::Down => {
                self.bar_drag = Some(BarDrag {
                    start: info.window_y,
                    top: self.scroll_top(),
                });
            }
            PointerPhase::Drag => {
                let Some(drag) = self.bar_drag else {
                    return Handled::ignored();
                };
                self.set_scroll_top(
                    drag.top
                        + (info.window_y - drag.start) * content / self.viewport.height.max(1.0),
                );
                self.following = self.is_at_end();
                return Handled::new(true, self.on_scroll.clone());
            }
            PointerPhase::Up => self.bar_drag = None,
            _ => return Handled::ignored(),
        }
        // down / up: handled (the thumb moves / the drag ends), nothing scrolled
        Handled::new(true, None)
    }

    /// Read-only metrics of this list for the runtime (see [`List`]).
    pub fn view(&self) -> List<'_> {
        List { state: self }
    }
}

impl Default for ListState {
    fn default() -> Self {
        ListState::new()
    }
}

/// Result of one Fenwick prefix query.
struct Prefix {
    sum: f64,
    cnt: usize,
}

/// What an event handler wants its closure to do after the `ListState` borrow
/// is released.
///
/// User code (the app's `on_scroll`) must not run while the `RefCell` is
/// borrowed, so the handler returns the callback and the closure calls it
/// after `borrow_mut()` ends.
pub(super) struct Handled {
    pub(super) handled: bool,
    pub(super) on_scroll: Option<Action>,
}

impl Handled {
    fn new(handled: bool, on_scroll: Option<Action>) -> Handled {
        Handled { handled, on_scroll }
    }

    fn ignored() -> Handled {
        Handled {
            handled: false,
            on_scroll: None,
        }
    }
}
