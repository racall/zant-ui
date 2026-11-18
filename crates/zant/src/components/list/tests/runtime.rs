//! Whole-app tests for virtualization, tail following, and text selection.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::events::{Event, Modifiers, Scroll};
use crate::core::geometry::{Edges, Rect};
use crate::flex;
use crate::renderer::renderer::Renderer;
use crate::runtime::Runtime;
use crate::ui::app::{App, RunOptions};
use crate::ui::builders::{column, text};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, TextOptions};

use super::super::{Alignment, ItemBuilder, ListOptions, ListState, ScrollPosition, list};
use super::approx;

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
    builds: usize,
}

impl Chat {
    fn new(n: usize) -> Chat {
        Chat {
            state: Rc::new(RefCell::new(ListState::new())),
            lines: Rc::new(RefCell::new(vec![3; 64])),
            n,
            builds: 0,
        }
    }
}

impl App for Chat {
    fn render(&mut self) -> Node {
        self.builds += 1;
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

#[test]
fn list_a_streaming_tail_grows_in_place_without_rebuilding_the_app() {
    let chat = Chat::new(20);
    let lines = Rc::clone(&chat.lines);
    let mut runtime = Runtime::init(chat, RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 600).expect("raster renderer");
    for _ in 0..4 {
        frame(&mut runtime, &mut renderer);
    }
    let viewport = bounds(&runtime, "chat").expect("viewport");
    // one more line per frame, like a streaming reply: the tail is in view in
    // the same frame, and the frame builds the app once (the item's size
    // report does not rebuild it again)
    for grown in 4..16 {
        lines.borrow_mut()[19] = grown;
        runtime.invalidate();
        let before = runtime.app_state_mut::<Chat>().builds;
        frame(&mut runtime, &mut renderer);
        let last = bounds(&runtime, "chat-i19").expect("last message");
        approx(viewport.bottom(), last.bottom(), 1.0);
        assert_eq!(
            runtime.app_state_mut::<Chat>().builds - before,
            1,
            "{grown} lines: one build per streaming frame"
        );
    }
    // the rows above moved up with it, without gaps
    let above = bounds(&runtime, "chat-i18").expect("previous message");
    let last = bounds(&runtime, "chat-i19").expect("last message");
    approx(above.bottom() + 10.0, last.y, 1.0);
}
