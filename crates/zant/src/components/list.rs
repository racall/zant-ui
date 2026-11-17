//! The virtual list ([`list`]) and the app-owned [`ListState`].
//!
//! Only the items in and near the viewport are built and laid out, so a list of
//! thousands of rows — or a chat with long messages — opens and scrolls at the
//! cost of one screen. Modelled on GPUI's `list` / `uniform_list`:
//!
//! - item heights are measured when items are laid out and remembered;
//!   unmeasured items count with an estimate (the running average);
//! - the scroll position is logical — item index + offset into it — so a
//!   corrected estimate above the viewport never moves what is on screen;
//! - `overdraw` px above and below the viewport are built too, so scrolling
//!   never shows a gap;
//! - [`Alignment::Bottom`] + `follow_tail` for chats: new content stays in view
//!   until the user scrolls up, and following resumes at the end.
//!
//! ## Ownership
//!
//! [`ListOptions`] takes the item builder as a closure
//! (`Rc<dyn Fn(usize) -> Node>`). The event callbacks need the state, and a
//! `Fn` callback cannot capture `&mut ListState`, so the app owns
//! `Rc<RefCell<ListState>>` and [`list`] clones that `Rc` into the pointer /
//! key / resize callbacks. `ListState`'s methods themselves take `&mut self`.
//!
//! ## Runtime integration points (the runtime needs no list-specific code)
//!
//! The list is built out of the ordinary declarative pieces. The runtime
//! paths it relies on are:
//!
//! - **wheel / trackpad**: the outer `pointer_area` sets `capture_scroll`, so
//!   `runtime::dispatch::Runtime::handle_scroll` (`wheel_target_at`) delivers
//!   [`PointerPhase::Scroll`] with `dy`; the list scrolls by `-dy`.
//! - **keys**: the outer area is `focusable`; a click focuses it even though
//!   `clicks = false` (presses still reach the rows, for text selection and
//!   buttons) and `runtime::keyboard::handle_key_down` forwards keys to the
//!   container's `key_action` (arrows, Page Up / Down, Home / End, Space).
//! - **measurement**: item wrappers and the viewport carry `on_resize`.
//!   `runtime::frame::Runtime::notify_resizes` rebuilds and re-lays out in the
//!   same frame (up to three passes) when a box changed, and
//!   `runtime::frame::Runtime::refresh_key_bounds` publishes the new boxes to
//!   [`crate::ui::key_bounds`]; the next build of [`list`] reads them with
//!   `key_bounds` and calls `ListState::set_measured`. That is how a row's
//!   real height replaces the estimate without ever showing a stale frame.
//! - **more rows**: when only part of the wanted range fitted the per-pass
//!   measurement budget, [`crate::ui::wake_after`] asks the runtime for the next
//!   frame (`Runtime::compute_next_delay`).
//! - **[`List`]**: the read-only metrics facade a runtime that paints / scrolls
//!   lists itself can query — total height, first / last visible index and the
//!   rows to paint. The declarative builder is self-contained, so the current
//!   runtime does not call it; it is the documented hook for such a runtime.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::Str;
use crate::core::events::{Key, KeyEvent};
use crate::core::geometry::{Edges, Size};
use crate::flex;
use crate::theme::tokens;
use crate::ui::actions::{Action, ActionCx, KeyAction, PointerAction, PointerInfo, PointerPhase};
use crate::ui::builders::{column, pointer_area};
use crate::ui::context::{current_theme, frame_string, key_bounds, wake_after};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, PointerAreaOptions};
use crate::ui::styled::{Style, styled};

/// Builds one row; called only for the indices of the rendered range.
pub type ItemBuilder = Rc<dyn Fn(usize) -> Node>;

/// Stable id of one row (see [`ListOptions::item_id`]).
pub type ItemId = Rc<dyn Fn(usize) -> u64>;

/// Vertical placement of a short list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    /// Items start at the top (most lists).
    #[default]
    Top,
    /// A short list sits at the bottom of the viewport (chats).
    Bottom,
}

/// Where [`ListState::scroll_to_item`] puts the item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollPosition {
    #[default]
    Start,
    Center,
    End,
}

/// A `scroll_to_item` request applied by the next build.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingScroll {
    pub index: usize,
    pub position: ScrollPosition,
}

/// A scroll-bar drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarDrag {
    /// `window_y` at the press.
    pub start: f32,
    /// `scroll_top` at the press.
    pub top: f32,
}

/// Scroll position, heights and follow state of one list (app-owned).
///
/// Own it as `Rc<RefCell<ListState>>` and pass it to [`list`]; methods take
/// `&mut self`. Height bookkeeping is a Fenwick tree over measured heights
/// plus a second one over measured counts, so prefix sums and `index_at` are
/// O(log n).
pub struct ListState {
    /// Measured height per item (< 0: not measured yet).
    pub heights: Vec<f32>,
    /// Fenwick trees over measured heights and measured counts (prefix sums).
    pub fen_sum: Vec<f64>,
    pub fen_cnt: Vec<u32>,
    pub measured_total: f64,
    pub measured_count: usize,
    /// Viewport top = top of item `anchor` + `offset` (offset may be negative
    /// within the top padding).
    pub anchor: usize,
    pub offset: f32,
    pub following: bool,
    pub viewport: Size,
    pub rendered_first: usize,
    pub rendered_end: usize,
    pub uniform: Option<f32>,
    pub estimate_hint: f32,
    pub gap: f32,
    pub padding: Edges,
    pub pending_scroll: Option<PendingScroll>,
    /// What `follow` last applied (declarative scrolling from TSX).
    pub followed_key: u64,
    pub followed_index: Option<usize>,
    pub bar_drag: Option<BarDrag>,
    pub options_key: Str,
    pub item_id: Option<ItemId>,
    pub on_scroll: Option<Action>,
}

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

    fn set_measured(&mut self, index: usize, h: f32) {
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
    fn index_at(&self, y: f32) -> usize {
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

    fn max_scroll_top(&self) -> f32 {
        (self.content_height() - self.viewport.height).max(0.0)
    }

    fn set_scroll_top(&mut self, top: f32) {
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

    fn item_key(&self, index: usize) -> Str {
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
    fn on_pointer(&mut self, info: PointerInfo) -> Handled {
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
    fn on_key(&mut self, k: KeyEvent) -> Handled {
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
    fn on_bar(&mut self, info: PointerInfo) -> Handled {
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
struct Handled {
    handled: bool,
    on_scroll: Option<Action>,
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

/// Runs a list event handler and then the app's `on_scroll` callback, with the
/// state borrow released (see [`Handled`]).
fn run_handled(handled: Handled, cx: &mut ActionCx<'_>) -> bool {
    if let Some(callback) = handled.on_scroll {
        callback.call(cx);
    }
    handled.handled
}

fn run_pointer(state: &Rc<RefCell<ListState>>, cx: &mut ActionCx<'_>, info: PointerInfo) -> bool {
    let handled = state.borrow_mut().on_pointer(info);
    run_handled(handled, cx)
}

fn run_key(state: &Rc<RefCell<ListState>>, cx: &mut ActionCx<'_>, key: KeyEvent) -> bool {
    let handled = state.borrow_mut().on_key(key);
    run_handled(handled, cx)
}

fn run_bar(state: &Rc<RefCell<ListState>>, cx: &mut ActionCx<'_>, info: PointerInfo) -> bool {
    let handled = state.borrow_mut().on_bar(info);
    run_handled(handled, cx)
}

/// Read-only metrics of one virtual list — the runtime integration point
/// ("the runtime must be able to ask: total height, first / last visible index,
/// paint rows").
///
/// [`list`] paints its own rows declaratively, so the current
/// runtime never needs this; it exists for a runtime that scrolls and paints
/// lists itself. Get it with [`ListState::view`].
///
/// Positions are content coordinates (padding included, see
/// [`ListState::item_top`]); subtract [`ListState::scroll_top`] for
/// viewport-relative pixels.
pub struct List<'a> {
    state: &'a ListState,
}

impl List<'_> {
    /// Height of the whole content.
    pub fn total_height(&self) -> f32 {
        self.state.content_height()
    }

    /// The first index with any part in the viewport; `0` for an empty list.
    pub fn first_visible_index(&self) -> usize {
        self.visible_range().start
    }

    /// The last index with any part in the viewport (inclusive); `0` when the
    /// list is empty. An item whose top is exactly at the viewport bottom still
    /// counts (its first line is on screen).
    pub fn last_visible_index(&self) -> usize {
        let range = self.visible_range();
        if range.is_empty() { 0 } else { range.end - 1 }
    }

    /// `first .. end` indices in the viewport (no overdraw).
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let n = self.state.count();
        if n == 0 || self.state.viewport.height <= 0.0 {
            return 0..0;
        }
        let top = self.state.scroll_top().max(0.0);
        let first = self.state.index_at(top);
        let end = (self.state.index_at(top + self.state.viewport.height) + 1).min(n);
        first..end
    }

    /// `first .. end` indices built in the last frame (visible + overdraw).
    pub fn rendered_range(&self) -> std::ops::Range<usize> {
        self.state.rendered_first..self.state.rendered_end
    }

    /// Calls `paint(index, top)` for every row [`list`] built last frame, so a
    /// runtime-owned painter can place them; `top` is the content coordinate.
    pub fn paint_rows(&self, mut paint: impl FnMut(usize, f32)) {
        for index in self.rendered_range() {
            paint(index, self.state.item_top(index));
        }
    }
}

/// Options of [`list`].
#[derive(Clone, zant_macros::TsxProps)]
pub struct ListOptions {
    /// Key of the list's viewport (node keys of items derive from it).
    pub key: Str,
    pub count: usize,
    pub item: ItemBuilder,
    /// Stable ids for item keys. Without it items are keyed by index, which is
    /// right when items are only appended (chats, logs); give ids when items
    /// are inserted or removed in the middle. Called while the list state is
    /// borrowed, so it must not borrow the same `ListState`.
    pub item_id: Option<ItemId>,
    pub alignment: Alignment,
    /// Keep the end in view as items grow or are added, until the user scrolls
    /// away from it; scrolling back to the end resumes following.
    pub follow_tail: bool,
    /// Extra px built above and below the viewport.
    pub overdraw: f32,
    /// Every item has this height (no measuring; `uniform_list`).
    pub item_height: Option<f32>,
    /// Height assumed for items not laid out yet (until some are measured,
    /// after which their average is used).
    pub estimated_height: f32,
    pub gap: f32,
    pub padding: Edges,
    pub background: Option<u32>,
    /// Viewport size: fixed, or grow inside a sized parent (default).
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: f32,
    /// Text in the items can be selected and copied (`BoxOptions::selectable`).
    pub selectable: bool,
    /// Take keyboard focus on click: arrows, Page Up / Down, Home / End.
    pub focusable: bool,
    pub scrollbar: bool,
    /// Called after the user scrolls (wheel, keys, scroll bar).
    pub on_scroll: Option<Action>,
}

impl Default for ListOptions {
    /// `count` / `item` have no meaningful default and get neutral values so
    /// `..Default::default()` works.
    fn default() -> Self {
        ListOptions {
            key: "list".into(),
            count: 0,
            item: Rc::new(|_| Node::default()),
            item_id: None,
            alignment: Alignment::Top,
            follow_tail: false,
            overdraw: 600.0,
            item_height: None,
            estimated_height: 120.0,
            gap: 0.0,
            padding: Edges::default(),
            background: None,
            width: None,
            height: None,
            grow: 1.0,
            selectable: false,
            focusable: true,
            scrollbar: true,
            on_scroll: None,
        }
    }
}

/// The virtual list (see the module documentation).
///
/// The app owns the state as `Rc<RefCell<ListState>>` and passes it here; the
/// returned node's pointer / key / resize callbacks share that state.
pub fn list(state: &Rc<RefCell<ListState>>, options: ListOptions) -> Node {
    let theme = current_theme();
    let key = options.key.clone();
    let viewport_key: Str = frame_string(&format!("{}-viewport", key.as_str()));
    crate::runtime::view_state::register_list(viewport_key.as_str(), state);
    let content_key: Str = frame_string(&format!("{}-content", key.as_str()));
    let bar_key: Str = frame_string(&format!("{}-bar", key.as_str()));

    // ---- state of this build: options, measured heights, scroll position
    let mut item_keys: Vec<Str> = Vec::new();
    let mut first_index: usize = 0;
    let mut end_index: usize = 0;
    let mut content_top: f32 = 0.0;
    let mut rows_built = false;
    let mut bar: Option<(f32, f32, u32)> = None;
    {
        let mut state = state.borrow_mut();
        state.options_key = key.clone();
        state.item_id = options.item_id.clone();
        state.on_scroll = options.on_scroll.clone();
        state.uniform = options.item_height;
        state.estimate_hint = options.estimated_height;
        state.gap = options.gap;
        state.padding = options.padding;
        state.set_count(options.count);
        if !options.follow_tail {
            state.following = false;
        }

        // the viewport as laid out last time (on_resize rebuilds when it
        // changes), read from a child: the list's own key may be replaced by
        // its parent (`ui::with_key`, a TSX component whose root is a list)
        if let Some(bounds) = key_bounds(viewport_key.as_str()) {
            if (bounds.width - state.viewport.width).abs() > 0.5
                && state.viewport.width > 0.0
                && state.uniform.is_none()
            {
                // heights depend on the width: measure again as items show
                state.invalidate_all();
            }
            state.viewport = Size {
                width: bounds.width,
                height: bounds.height,
            };
        }
        // measure the items built last time
        if state.uniform.is_none() {
            let mut i = state.rendered_first;
            while i < state.rendered_end && i < state.count() {
                let item_key = state.item_key(i);
                if let Some(bounds) = key_bounds(item_key.as_str()) {
                    state.set_measured(i, bounds.height);
                }
                i += 1;
            }
        }

        let n = state.count();
        let viewport_height = state.viewport.height;
        if viewport_height > 0.0 && n > 0 {
            if let Some(pending) = state.pending_scroll.take() {
                let index = pending.index.min(n - 1);
                let top = state.item_top(index);
                let height = state.item_height(index);
                let padding = state.padding;
                let target = match pending.position {
                    ScrollPosition::Start => top - padding.top,
                    ScrollPosition::Center => top + height / 2.0 - viewport_height / 2.0,
                    ScrollPosition::End => top + height - viewport_height + padding.bottom,
                };
                state.set_scroll_top(target);
            }
            if state.following {
                let max_top = state.max_scroll_top();
                state.set_scroll_top(max_top);
            }
            // keep the anchor inside the scroll range (heights may have changed)
            let current = state.scroll_top();
            state.set_scroll_top(current);

            let content_height = state.content_height();
            // a short list sits at the bottom for `.bottom` alignment
            let shift =
                if content_height < viewport_height && options.alignment == Alignment::Bottom {
                    viewport_height - content_height
                } else {
                    0.0
                };
            let top = state.scroll_top();
            let want_first = state.index_at((top - options.overdraw).max(0.0));
            let mut want_end = state.index_at(top + viewport_height + options.overdraw) + 1;
            if want_end > n {
                want_end = n;
            }
            // Estimates of unmeasured items can be far off (a 120 px guess for a
            // 1500 px message), so build at most a few of them per pass, growing
            // outward from the anchor; the same-frame layout passes measure them
            // and the next pass fills the rest of the range (GPUI measures items
            // one by one the same way).
            const MAX_UNMEASURED: usize = 3;
            let mut unmeasured: usize = 0;
            let anchor = state.anchor.min(n - 1);
            let mut first = want_first.max(anchor.min(want_end - 1));
            let mut end = first + 1;
            if state.uniform.is_none() && state.heights[first] < 0.0 {
                unmeasured += 1;
            }
            loop {
                let mut grew = false;
                if end < want_end
                    && (state.uniform.is_some()
                        || state.heights[end] >= 0.0
                        || unmeasured < MAX_UNMEASURED)
                {
                    if state.uniform.is_none() && state.heights[end] < 0.0 {
                        unmeasured += 1;
                    }
                    end += 1;
                    grew = true;
                }
                if first > want_first
                    && (state.uniform.is_some()
                        || state.heights[first - 1] >= 0.0
                        || unmeasured < MAX_UNMEASURED)
                {
                    first -= 1;
                    if state.uniform.is_none() && state.heights[first] < 0.0 {
                        unmeasured += 1;
                    }
                    grew = true;
                }
                if !grew {
                    break;
                }
            }
            // the rest of the range comes in the next frame
            if first > want_first || end < want_end {
                wake_after(1);
            }
            state.rendered_first = first;
            state.rendered_end = end;

            first_index = first;
            end_index = end;
            content_top = shift + state.item_top(first) - top;
            let mut i = first;
            while i < end {
                item_keys.push(state.item_key(i));
                i += 1;
            }
            rows_built = true;

            if options.scrollbar && content_height > viewport_height + 0.5 {
                let track = viewport_height - 8.0;
                let thumb = (track * viewport_height / content_height).max(28.0);
                let max_top = (content_height - viewport_height).max(1.0);
                let thumb_y = 4.0 + (track - thumb) * (top / max_top).clamp(0.0, 1.0);
                let color = tokens::with_alpha(
                    theme.color_text,
                    if state.bar_drag.is_some() { 0x70 } else { 0x40 },
                );
                bar = Some((thumb_y, thumb, color));
            }
        } else {
            state.rendered_first = 0;
            state.rendered_end = 0;
        }
    }

    // ---- children: an empty box filling the list (its bounds are the
    // viewport), the built rows placed so the anchor lines up, the scroll bar
    let mut children: Vec<Node> = Vec::new();
    children.push(styled(
        column(BoxOptions {
            key: Some(viewport_key),
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            ..Default::default()
        },
    ));
    if rows_built {
        let resize_action = if options.item_height.is_none() {
            Some(Action::new(|_cx| ()))
        } else {
            None
        };
        let mut items: Vec<Node> = Vec::with_capacity(end_index - first_index);
        for i in first_index..end_index {
            let built = (options.item)(i);
            items.push(column(BoxOptions {
                key: Some(item_keys[i - first_index].clone()),
                width_percent: Some(100.0),
                align_items: flex::Align::Stretch,
                height: options.item_height,
                on_resize: resize_action.clone(),
                children: vec![built],
                ..Default::default()
            }));
        }
        // the built items as one column, placed so the anchor lines up
        children.push(styled(
            column(BoxOptions {
                key: Some(content_key),
                gap: options.gap,
                padding: Edges {
                    top: 0.0,
                    right: options.padding.right,
                    bottom: 0.0,
                    left: options.padding.left,
                },
                align_items: flex::Align::Stretch,
                children: items,
                ..Default::default()
            }),
            Style {
                absolute: Some(true),
                left: Some(0.0),
                right: Some(0.0),
                top: Some(content_top),
                ..Default::default()
            },
        ));
        if let Some((thumb_y, thumb, color)) = bar {
            let bar_state = Rc::clone(state);
            children.push(pointer_area(PointerAreaOptions {
                r#box: BoxOptions {
                    key: Some(bar_key),
                    absolute: true,
                    right: Some(0.0),
                    top: Some(thumb_y),
                    width: Some(12.0),
                    height: Some(thumb),
                    align_items: flex::Align::Center,
                    children: vec![styled(
                        column(BoxOptions::default()),
                        Style {
                            width: Some(6.0),
                            height: Some(thumb),
                            radius: Some(3.0),
                            background: Some(color),
                            ..Default::default()
                        },
                    )],
                    ..Default::default()
                },
                on_pointer: PointerAction::new(move |cx, info| run_bar(&bar_state, cx, info)),
                ..Default::default()
            }));
        }
    }

    // ---- the viewport: wheel + keyboard; presses reach the rows
    let pointer_state = Rc::clone(state);
    let key_state = Rc::clone(state);
    pointer_area(PointerAreaOptions {
        r#box: BoxOptions {
            key: Some(key),
            width: options.width,
            height: options.height,
            grow: if options.height.is_none() {
                options.grow
            } else {
                0.0
            },
            shrink: 1.0,
            basis: if options.height.is_none() {
                Some(0.0)
            } else {
                None
            },
            width_percent: if options.width.is_none() {
                Some(100.0)
            } else {
                None
            },
            clip: true,
            background: options.background,
            selectable: options.selectable,
            on_resize: Some(Action::new(|_cx| ())),
            children,
            ..Default::default()
        },
        on_pointer: PointerAction::new(move |cx, info| run_pointer(&pointer_state, cx, info)),
        capture_scroll: true,
        // the list takes the wheel only: presses reach the items (text
        // selection, buttons) and focus the list for its keys
        clicks: false,
        focusable: options.focusable,
        on_key: Some(KeyAction::new(move |cx, key| run_key(&key_state, cx, key))),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    use crate::core::events::{Event, Key, KeyEvent, Modifiers, Scroll};
    use crate::core::geometry::Rect;
    use crate::renderer::renderer::Renderer;
    use crate::runtime::Runtime;
    use crate::ui::app::{App, RunOptions};
    use crate::ui::builders::text;
    use crate::ui::context::{KeyBoundsHook, set_key_bounds_hook};
    use crate::ui::options::TextOptions;

    fn key(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    fn scroll_info(dy: f32) -> PointerInfo {
        PointerInfo {
            phase: PointerPhase::Scroll,
            dy,
            ..Default::default()
        }
    }

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 400.0, 600.0, 1.0);
        renderer.end();
    }

    fn built_rows(runtime: &Runtime, prefix: &str) -> usize {
        runtime
            .tree
            .nodes
            .iter()
            .filter(|node| {
                node.alive
                    && node
                        .key
                        .as_ref()
                        .is_some_and(|key| key.as_str().starts_with(prefix))
            })
            .count()
    }

    fn bounds(runtime: &Runtime, key: &str) -> Option<Rect> {
        runtime.find_key(key).map(|id| runtime.tree.get(id).bounds)
    }

    fn approx(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} !~= {expected} (tolerance {tolerance})"
        );
    }

    // ---- whole-app list behaviour ---------------------------------------

    struct Uniform {
        state: Rc<RefCell<ListState>>,
    }

    impl App for Uniform {
        fn render(&mut self) -> Node {
            let state = Rc::clone(&self.state);
            column(BoxOptions {
                grow: 1.0,
                align_items: flex::Align::Stretch,
                children: vec![list(
                    &state,
                    ListOptions {
                        key: "rows".into(),
                        count: 10_000,
                        item: Rc::new(|i| text(format!("Row {i}"), TextOptions::default())),
                        item_height: Some(30.0),
                        overdraw: 60.0,
                        ..Default::default()
                    },
                )],
                ..Default::default()
            })
        }
    }

    #[test]
    fn list_10k_uniform_rows_build_only_the_visible_range_wheel_and_scroll_to_item() {
        let state = Rc::new(RefCell::new(ListState::new()));
        let mut runtime = Runtime::init(
            Uniform {
                state: Rc::clone(&state),
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(400, 600).expect("raster renderer");
        frame(&mut runtime, &mut renderer);
        frame(&mut runtime, &mut renderer);
        // 600 px viewport + 60 px overdraw below → 22 rows, not 10 000
        let built = built_rows(&runtime, "rows-i");
        assert!((20..=26).contains(&built), "built {built} rows");
        assert_eq!(state.borrow().rendered_range()[0], 0);
        // wheel down 3000 px
        for _ in 0..30 {
            runtime.dispatch(Event::Scroll(Scroll {
                x: 200.0,
                y: 300.0,
                dx: 0.0,
                dy: -100.0,
                modifiers: Modifiers::default(),
            }));
        }
        frame(&mut runtime, &mut renderer);
        approx(state.borrow().scroll_top(), 3000.0, 0.5);
        assert!(runtime.find_key("rows-i100").is_some());
        assert!(runtime.find_key("rows-i0").is_none());
        // jump to row 5000 (start of the viewport)
        state
            .borrow_mut()
            .scroll_to_item(5000, ScrollPosition::Start);
        runtime.invalidate();
        frame(&mut runtime, &mut renderer);
        let row = bounds(&runtime, "rows-i5000").expect("row 5000 built");
        let viewport = bounds(&runtime, "rows").expect("viewport");
        approx(viewport.y, row.y, 0.5);
    }

    struct Chat {
        state: Rc<RefCell<ListState>>,
        lines: Rc<RefCell<Vec<usize>>>,
        n: usize,
    }

    impl Chat {
        fn new(n: usize) -> Chat {
            Chat {
                state: Rc::new(RefCell::new(ListState::new())),
                lines: Rc::new(RefCell::new(vec![3; 64])),
                n,
            }
        }
    }

    impl App for Chat {
        fn render(&mut self) -> Node {
            let state = Rc::clone(&self.state);
            let lines = Rc::clone(&self.lines);
            let item: ItemBuilder = Rc::new(move |i| {
                let count = lines.borrow().get(i).copied().unwrap_or(0);
                let mut rows = Vec::with_capacity(count);
                for k in 0..count {
                    rows.push(text(
                        format!("Message {i} line {k}"),
                        TextOptions {
                            size: 14.0,
                            ..Default::default()
                        },
                    ));
                }
                column(BoxOptions {
                    padding: Edges::all(8.0),
                    align_items: flex::Align::Stretch,
                    children: rows,
                    ..Default::default()
                })
            });
            column(BoxOptions {
                grow: 1.0,
                align_items: flex::Align::Stretch,
                children: vec![list(
                    &state,
                    ListOptions {
                        key: "chat".into(),
                        count: self.n,
                        item,
                        alignment: Alignment::Bottom,
                        follow_tail: true,
                        gap: 10.0,
                        selectable: true,
                        ..Default::default()
                    },
                )],
                ..Default::default()
            })
        }
    }

    #[test]
    fn list_chat_follows_the_tail_stops_when_scrolled_up_keeps_the_anchor_when_heights_change() {
        let chat = Chat::new(20);
        let state = Rc::clone(&chat.state);
        let lines = Rc::clone(&chat.lines);
        let mut runtime = Runtime::init(chat, RunOptions::default());
        let mut renderer = Renderer::create_raster(400, 600).expect("raster renderer");
        for _ in 0..4 {
            frame(&mut runtime, &mut renderer);
        }
        // the last message's bottom is at the viewport bottom
        let viewport = bounds(&runtime, "chat").expect("viewport");
        let mut last = bounds(&runtime, "chat-i19").expect("last message");
        approx(viewport.bottom(), last.bottom(), 1.0);
        assert!(state.borrow().is_at_end());
        // a growing last message stays in view (streaming)
        lines.borrow_mut()[19] = 12;
        runtime.invalidate();
        for _ in 0..3 {
            frame(&mut runtime, &mut renderer);
        }
        last = bounds(&runtime, "chat-i19").expect("last message");
        approx(viewport.bottom(), last.bottom(), 1.0);
        // scroll up: following stops, and a new message does not pull the view
        for _ in 0..5 {
            runtime.dispatch(Event::Scroll(Scroll {
                x: 200.0,
                y: 300.0,
                dx: 0.0,
                dy: 80.0,
                modifiers: Modifiers::default(),
            }));
        }
        frame(&mut runtime, &mut renderer);
        assert!(!state.borrow().following);
        let anchor = state.borrow().anchor;
        let anchor_key = format!("chat-i{anchor}");
        let before = bounds(&runtime, &anchor_key).expect("anchor built").y;
        runtime.app_state_mut::<Chat>().n = 21;
        // and a message above the anchor growing does not move it either
        if anchor > 0 {
            lines.borrow_mut()[anchor - 1] = 6;
        }
        runtime.invalidate();
        for _ in 0..3 {
            frame(&mut runtime, &mut renderer);
        }
        approx(
            bounds(&runtime, &anchor_key).expect("anchor built").y,
            before,
            1.0,
        );
        // End resumes following
        state.borrow_mut().scroll_to_end();
        runtime.invalidate();
        for _ in 0..3 {
            frame(&mut runtime, &mut renderer);
        }
        approx(
            viewport.bottom(),
            bounds(&runtime, "chat-i20").expect("new message").bottom(),
            1.0,
        );
        // text in items selects (the list takes only the wheel)
        let b = bounds(&runtime, "chat-i20").expect("new message");
        runtime.dispatch(Event::PointerDown(crate::core::events::PointerEvent {
            x: b.x + 9.0,
            y: b.y + 14.0,
            button: crate::core::events::PointerButton::Left,
            modifiers: Modifiers::default(),
        }));
        runtime.dispatch(Event::PointerMove(crate::core::events::PointerEvent {
            x: b.x + 300.0,
            y: b.y + 14.0,
            button: crate::core::events::PointerButton::Left,
            modifiers: Modifiers::default(),
        }));
        runtime.dispatch(Event::PointerUp(crate::core::events::PointerEvent {
            x: b.x + 300.0,
            y: b.y + 14.0,
            button: crate::core::events::PointerButton::Left,
            modifiers: Modifiers::default(),
        }));
        let selection = runtime.selected_text();
        assert!(
            selection.starts_with("Message 20 line 0"),
            "selection {selection:?}"
        );
    }

    #[test]
    fn list_a_short_list_sits_at_the_bottom_with_bottom_alignment() {
        let chat = Chat::new(2);
        let state = Rc::clone(&chat.state);
        let mut runtime = Runtime::init(chat, RunOptions::default());
        let mut renderer = Renderer::create_raster(400, 600).expect("raster renderer");
        for _ in 0..3 {
            frame(&mut runtime, &mut renderer);
        }
        let viewport = bounds(&runtime, "chat").expect("viewport");
        approx(
            viewport.bottom(),
            bounds(&runtime, "chat-i1")
                .expect("second message")
                .bottom(),
            1.0,
        );
        assert!(bounds(&runtime, "chat-i0").expect("first message").y > viewport.y + 100.0);
        assert_eq!(state.borrow().count(), 2);
    }

    // ---- options, viewport and callbacks ---------------------------------

    fn viewport_hook(height: f32) -> KeyBoundsHook {
        Rc::new(move |key: &str| match key {
            "rows-viewport" => Some(Rect::init(0.0, 0.0, 400.0, height)),
            _ => None,
        })
    }

    #[test]
    fn list_option_defaults() {
        let options = ListOptions::default();
        assert_eq!(options.key.as_str(), "list");
        assert_eq!(options.count, 0);
        assert_eq!(options.alignment, Alignment::Top);
        assert!(!options.follow_tail);
        assert_eq!(options.overdraw, 600.0);
        assert_eq!(options.item_height, None);
        assert_eq!(options.estimated_height, 120.0);
        assert_eq!(options.gap, 0.0);
        assert_eq!(options.padding, Edges::default());
        assert_eq!(options.background, None);
        assert_eq!(options.width, None);
        assert_eq!(options.height, None);
        assert_eq!(options.grow, 1.0);
        assert!(!options.selectable);
        assert!(options.focusable);
        assert!(options.scrollbar);
        assert!(options.on_scroll.is_none());
    }

    #[test]
    fn list_empty_list_has_no_rows_and_padding_only_height() {
        let mut state = ListState::new();
        state.set_count(0);
        assert_eq!(state.count(), 0);
        assert_eq!(state.rendered_range(), [0, 0]);
        assert_eq!(state.scroll_top(), 0.0);
        assert!(state.is_at_end());
        assert_eq!(state.item_top(0), 0.0);
        assert_eq!(state.content_height(), 0.0);
        state.padding = Edges::all(4.0);
        assert_eq!(state.content_height(), 8.0);

        // the builder renders only the viewport box for an empty list
        set_key_bounds_hook(Some(viewport_hook(600.0)));
        let shared = Rc::new(RefCell::new(state));
        let node = list(
            &shared,
            ListOptions {
                key: "rows".into(),
                count: 0,
                item: Rc::new(|_| Node::default()),
                padding: Edges::all(4.0),
                ..Default::default()
            },
        );
        set_key_bounds_hook(None);
        let element = node.element();
        assert_eq!(element.children.len(), 1, "only the viewport box");
        assert_eq!(
            element.children[0].element().key.as_deref(),
            Some("rows-viewport")
        );
        assert_eq!(shared.borrow().rendered_range(), [0, 0]);
        assert_eq!(shared.borrow().content_height(), 8.0);
    }

    #[test]
    fn list_few_items_are_all_built_inside_their_wrappers() {
        set_key_bounds_hook(Some(viewport_hook(600.0)));
        let shared = Rc::new(RefCell::new(ListState::new()));
        let node = list(
            &shared,
            ListOptions {
                key: "rows".into(),
                count: 3,
                item: Rc::new(|i| text(format!("Row {i}"), TextOptions::default())),
                item_height: Some(30.0),
                ..Default::default()
            },
        );
        set_key_bounds_hook(None);
        let element = node.element();
        assert_eq!(element.key.as_deref(), Some("rows"));
        let pointer = element.pointer.as_ref().expect("pointer area");
        assert!(pointer.capture_scroll);
        assert!(!pointer.clicks);
        assert!(element.focusable);
        assert!(element.key_action.is_some());
        // viewport box + the content column (short list: no scroll bar)
        assert_eq!(element.children.len(), 2);
        let content = &element.children[1];
        assert_eq!(content.element().key.as_deref(), Some("rows-content"));
        let rows = &content.element().children;
        assert_eq!(rows.len(), 3);
        for (i, row) in rows.iter().enumerate() {
            let expected_key = format!("rows-i{i}");
            assert_eq!(row.element().key.as_deref(), Some(expected_key.as_str()));
            assert_eq!(row.element().layout.height, Some(30.0));
            assert_eq!(row.element().children.len(), 1);
        }
        assert_eq!(shared.borrow().rendered_range(), [0, 3]);
        assert_eq!(shared.borrow().content_height(), 90.0);
    }

    #[test]
    fn list_item_keys_come_from_the_stable_id_when_given() {
        set_key_bounds_hook(Some(viewport_hook(600.0)));
        let shared = Rc::new(RefCell::new(ListState::new()));
        let node = list(
            &shared,
            ListOptions {
                key: "rows".into(),
                count: 2,
                item: Rc::new(|_| Node::default()),
                item_id: Some(Rc::new(|i| 1000 + i as u64)),
                item_height: Some(30.0),
                ..Default::default()
            },
        );
        set_key_bounds_hook(None);
        let element = node.element();
        let rows = &element.children[1].element().children;
        assert_eq!(rows[0].element().key.as_deref(), Some("rows-k1000"));
        assert_eq!(rows[1].element().key.as_deref(), Some("rows-k1001"));
    }

    #[test]
    fn list_1e5_items_visible_range_math() {
        let mut state = ListState::new();
        state.viewport = Size {
            width: 400.0,
            height: 600.0,
        };
        state.uniform = Some(30.0);
        state.set_count(100_000);
        assert_eq!(state.count(), 100_000);
        // 600 px / 30 px rows = 20 rows fit; scroll to row 50 000
        state.set_scroll_top(50_000.0 * 30.0);
        assert_eq!(state.anchor, 50_000);
        let view = state.view();
        approx(view.total_height(), 3_000_000.0, 0.01);
        assert_eq!(view.first_visible_index(), 50_000);
        // the row starting exactly at the viewport bottom still counts
        assert_eq!(view.last_visible_index(), 50_020);
        assert_eq!(view.visible_range(), 50_000..50_021);
        // prefix-sum math at the far end
        approx(state.item_top(99_999), 2_999_970.0, 0.01);
        approx(state.item_top(100_000), 3_000_000.0, 0.01);
        state.rendered_first = 49_999;
        state.rendered_end = 50_002;
        let mut painted = Vec::new();
        state.view().paint_rows(|i, top| painted.push((i, top)));
        assert_eq!(
            painted,
            vec![
                (49_999, 1_499_970.0),
                (50_000, 1_500_000.0),
                (50_001, 1_500_030.0)
            ]
        );
        assert_eq!(state.view().rendered_range(), 49_999..50_002);
    }

    #[test]
    fn list_scroll_is_clamped_to_the_content() {
        let mut state = ListState::new();
        state.uniform = Some(30.0);
        state.set_count(10);
        state.viewport = Size {
            width: 400.0,
            height: 600.0,
        };
        // content (300) is shorter than the viewport: nothing scrolls
        assert_eq!(state.max_scroll_top(), 0.0);
        state.scroll_by(100.0);
        assert_eq!(state.scroll_top(), 0.0);
        assert!(state.is_at_end());
        // a 100 px viewport leaves 200 px of scroll range
        state.viewport.height = 100.0;
        assert_eq!(state.max_scroll_top(), 200.0);
        state.scroll_by(1000.0);
        assert_eq!(state.scroll_top(), 200.0);
        assert!(state.is_at_end());
        state.scroll_by(-100.0);
        approx(state.scroll_top(), 100.0, 0.001);
        assert!(!state.is_at_end());
        state.scroll_by(-1000.0);
        assert_eq!(state.scroll_top(), 0.0);
        state.set_scroll_top(500.0);
        assert_eq!(state.scroll_top(), 200.0);
    }

    #[test]
    fn list_keyboard_navigation_scrolls_and_drives_following() {
        let mut state = ListState::new();
        state.uniform = Some(30.0);
        state.set_count(10);
        state.viewport = Size {
            width: 400.0,
            height: 100.0,
        };
        let fired = Rc::new(Cell::new(0u32));
        let flag = Rc::clone(&fired);
        state.on_scroll = Some(Action::new(move |_| flag.set(flag.get() + 1)));
        let shared = Rc::new(RefCell::new(state));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        // Down / Up move 40 px and fire on_scroll
        assert!(run_key(&shared, &mut cx, key(Key::Down)));
        approx(shared.borrow().scroll_top(), 40.0, 0.001);
        assert!(run_key(&shared, &mut cx, key(Key::Down)));
        approx(shared.borrow().scroll_top(), 80.0, 0.001);
        assert!(run_key(&shared, &mut cx, key(Key::Up)));
        approx(shared.borrow().scroll_top(), 40.0, 0.001);
        // Page Down / Up use the viewport-based page size (max(40, 0.9*100))
        assert!(run_key(&shared, &mut cx, key(Key::PageDown)));
        approx(shared.borrow().scroll_top(), 130.0, 0.001);
        assert!(run_key(&shared, &mut cx, key(Key::PageUp)));
        approx(shared.borrow().scroll_top(), 40.0, 0.001);
        // Space pages down, Shift+Space pages up
        assert!(run_key(&shared, &mut cx, key(Key::Character(' '))));
        approx(shared.borrow().scroll_top(), 130.0, 0.001);
        assert!(run_key(
            &shared,
            &mut cx,
            KeyEvent {
                key: Key::Character(' '),
                modifiers: Modifiers {
                    shift: true,
                    ..Default::default()
                },
                repeat: false,
            }
        ));
        approx(shared.borrow().scroll_top(), 40.0, 0.001);
        // Home jumps to the top and stops following
        assert!(run_key(&shared, &mut cx, key(Key::Home)));
        assert_eq!(shared.borrow().scroll_top(), 0.0);
        assert!(!shared.borrow().following);
        // End resumes following; the next build snaps to the end
        assert!(run_key(&shared, &mut cx, key(Key::End)));
        assert!(shared.borrow().following);
        {
            let mut state = shared.borrow_mut();
            let max = state.max_scroll_top();
            state.set_scroll_top(max);
            assert_eq!(state.scroll_top(), 200.0);
        }
        assert!(shared.borrow().is_at_end());
        // an unrelated character is not consumed
        assert!(!run_key(&shared, &mut cx, key(Key::Character('a'))));
        assert_eq!(fired.get(), 9, "one on_scroll per handled key");
    }

    #[test]
    fn list_wheel_and_scroll_bar_share_the_scroll_position() {
        let mut state = ListState::new();
        state.uniform = Some(30.0);
        state.set_count(10);
        state.viewport = Size {
            width: 400.0,
            height: 100.0,
        };
        let shared = Rc::new(RefCell::new(state));
        let mut root = ();
        let mut cx = ActionCx::new(&mut root);
        // wheel up (negative dy) scrolls down
        assert!(run_pointer(&shared, &mut cx, scroll_info(-100.0)));
        approx(shared.borrow().scroll_top(), 100.0, 0.001);
        // a scroll smaller than the epsilon is ignored
        assert!(!run_pointer(&shared, &mut cx, scroll_info(0.005)));
        assert!(run_pointer(&shared, &mut cx, scroll_info(100.0)));
        approx(shared.borrow().scroll_top(), 0.0, 0.001);
        // the scroll-bar thumb: press at window_y 0, drag 20 px down
        {
            let mut state = shared.borrow_mut();
            state.set_scroll_top(100.0);
        }
        let bar = |phase: PointerPhase, window_y: f32| {
            shared
                .borrow_mut()
                .on_bar(PointerInfo {
                    phase,
                    window_y,
                    ..Default::default()
                })
                .handled
        };
        assert!(bar(PointerPhase::Down, 10.0));
        assert!(bar(PointerPhase::Drag, 20.0));
        // 10 px of thumb = 30 px of content (300 content / 100 viewport)
        approx(shared.borrow().scroll_top(), 130.0, 0.001);
        assert!(bar(PointerPhase::Up, 20.0));
        assert!(shared.borrow().bar_drag.is_none());
    }

    #[test]
    fn list_measurement_and_splice_keep_the_height_bookkeeping_consistent() {
        let mut state = ListState::new();
        state.set_count(5);
        for i in 0..5 {
            state.set_measured(i, 10.0);
        }
        assert_eq!(state.measured_count, 5);
        approx(state.estimate(), 10.0, 0.001);
        // replacing items 1..3 with 3 new ones keeps the measured prefix sums
        state.set_scroll_top(20.0);
        assert_eq!(state.anchor, 2);
        state.splice(1, 2, 3);
        assert_eq!(state.count(), 6);
        assert_eq!(&state.heights, &[10.0, -1.0, -1.0, -1.0, 10.0, 10.0]);
        assert_eq!(state.measured_count, 3);
        assert_eq!(state.anchor, 1);
        assert_eq!(state.offset, 0.0);
        // invalidate drops one measured height from the tree
        state.invalidate(0);
        assert_eq!(state.heights[0], -1.0);
        assert_eq!(state.measured_count, 2);
        state.invalidate_all();
        assert!(state.heights.iter().all(|h| *h < 0.0));
        assert_eq!(state.measured_count, 0);
        approx(state.estimate(), 120.0, 0.001);
        // setCount grows / shrinks and keeps the anchor in range
        state.set_count(8);
        assert_eq!(state.count(), 8);
        assert!(state.heights[7] < 0.0);
        state.set_count(1);
        assert_eq!(state.count(), 1);
        assert_eq!(state.anchor, 0);
    }

    #[test]
    fn list_logical_anchor_survives_measured_height_changes_above_it() {
        let mut state = ListState::new();
        state.set_count(10);
        for i in 0..10 {
            state.set_measured(i, 100.0);
        }
        state.viewport = Size {
            width: 400.0,
            height: 100.0,
        };
        state.set_scroll_top(500.0);
        assert_eq!(state.anchor, 5);
        approx(
            state.scroll_top() - state.item_top(state.anchor),
            0.0,
            0.001,
        );
        // item 2 (above the anchor) grows: the content coordinate of the anchor
        // moves, the viewport content does not (the offset into the anchor is
        // unchanged)
        state.set_measured(2, 400.0);
        approx(
            state.scroll_top() - state.item_top(state.anchor),
            0.0,
            0.001,
        );
        approx(state.scroll_top(), 800.0, 0.001);
    }

    #[test]
    fn list_follow_rescrolls_only_when_the_index_or_layout_key_changes() {
        let mut state = ListState::new();
        state.set_count(100);
        state.viewport = Size {
            width: 400.0,
            height: 300.0,
        };
        state.uniform = Some(20.0);
        state.follow(7, Some(10), ScrollPosition::Start);
        assert_eq!(
            state.pending_scroll,
            Some(PendingScroll {
                index: 10,
                position: ScrollPosition::Start
            })
        );
        // same layout key and index: no new request (the user may have scrolled)
        state.pending_scroll = None;
        state.follow(7, Some(10), ScrollPosition::Center);
        assert!(state.pending_scroll.is_none());
        // a changed index scrolls again
        state.follow(7, Some(20), ScrollPosition::Center);
        assert_eq!(state.pending_scroll.map(|p| p.index), Some(20));
        // a new layout key re-measures every row and applies the index again
        state.pending_scroll = None;
        state.set_measured(3, 25.0);
        state.follow(8, Some(20), ScrollPosition::End);
        assert!(state.heights.iter().all(|h| *h < 0.0));
        assert_eq!(
            state.pending_scroll,
            Some(PendingScroll {
                index: 20,
                position: ScrollPosition::End
            })
        );
        // clearing the followed index keeps `follow(None)` quiet
        state.pending_scroll = None;
        state.follow(8, None, ScrollPosition::End);
        assert!(state.pending_scroll.is_none());
        assert_eq!(state.followed_index, None);
    }

    #[test]
    fn list_short_bottom_aligned_content_places_rows_at_the_bottom() {
        set_key_bounds_hook(Some(viewport_hook(600.0)));
        let shared = Rc::new(RefCell::new(ListState::new()));
        let node = list(
            &shared,
            ListOptions {
                key: "rows".into(),
                count: 2,
                item: Rc::new(|_| Node::default()),
                item_height: Some(40.0),
                gap: 10.0,
                alignment: Alignment::Bottom,
                ..Default::default()
            },
        );
        set_key_bounds_hook(None);
        // content = 2 * 40 + 10 = 90 px, so the content column starts 510 px down
        let element = node.element();
        let content = &element.children[1];
        approx(content.element().layout.top.unwrap(), 510.0, 0.001);
        assert_eq!(shared.borrow().content_height(), 90.0);
        // and the scroll bar is not shown when everything fits
        assert_eq!(element.children.len(), 2);
    }
}
