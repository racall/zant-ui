//! Virtual list builder and its event callback adapters.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Str;
use crate::core::events::KeyEvent;
use crate::core::geometry::{Edges, Size};
use crate::flex;
use crate::theme::tokens;
use crate::ui::actions::{Action, ActionCx, KeyAction, PointerAction, PointerInfo};
use crate::ui::builders::{column, pointer_area};
use crate::ui::context::{current_theme, frame_string, key_bounds, wake_after};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, PointerAreaOptions};
use crate::ui::styled::{Style, styled};

use super::state::Handled;
use super::{Alignment, ListOptions, ListState, ScrollPosition};

/// Runs a list event handler and then the app's `on_scroll` callback, with the
/// state borrow released (see [`Handled`]).
fn run_handled(handled: Handled, cx: &mut ActionCx<'_>) -> bool {
    if let Some(callback) = handled.on_scroll {
        callback.call(cx);
    }
    handled.handled
}

pub(super) fn run_pointer(
    state: &Rc<RefCell<ListState>>,
    cx: &mut ActionCx<'_>,
    info: PointerInfo,
) -> bool {
    let handled = state.borrow_mut().on_pointer(info);
    run_handled(handled, cx)
}

pub(super) fn run_key(
    state: &Rc<RefCell<ListState>>,
    cx: &mut ActionCx<'_>,
    key: KeyEvent,
) -> bool {
    let handled = state.borrow_mut().on_key(key);
    run_handled(handled, cx)
}

fn run_bar(state: &Rc<RefCell<ListState>>, cx: &mut ActionCx<'_>, info: PointerInfo) -> bool {
    let handled = state.borrow_mut().on_bar(info);
    run_handled(handled, cx)
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
