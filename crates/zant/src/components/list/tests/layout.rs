//! List defaults, row layout, metrics, and measurement tests.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::geometry::{Edges, Rect, Size};
use crate::ui::builders::text;
use crate::ui::context::{KeyBoundsHook, set_key_bounds_hook};
use crate::ui::element::Node;
use crate::ui::options::TextOptions;

use super::super::{Alignment, ListOptions, ListState, list};
use super::approx;

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
