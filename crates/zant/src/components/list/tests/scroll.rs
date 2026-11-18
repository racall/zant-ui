//! Scroll position, input handling, and declarative following tests.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::events::{Key, KeyEvent, Modifiers};
use crate::core::geometry::Size;
use crate::ui::actions::{Action, ActionCx, PointerInfo, PointerPhase};

use super::super::build::{run_key, run_pointer};
use super::super::{ListState, PendingScroll, ScrollPosition};
use super::approx;

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
