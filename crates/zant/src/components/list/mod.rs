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
//!   [`PointerPhase::Scroll`](crate::ui::actions::PointerPhase::Scroll)
//!   with `dy`; the list scrolls by `-dy`.
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

mod build;
mod resize;
mod state;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use crate::Str;
use crate::core::geometry::{Edges, Size};
use crate::ui::actions::Action;
use crate::ui::element::Node;

pub use build::list;

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
