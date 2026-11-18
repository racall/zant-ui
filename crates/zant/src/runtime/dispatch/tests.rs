//! Unit tests.

use super::*;
use crate::core::events::{
    Composition, Event, Key, KeyEvent, Modifiers, PointerButton, PointerEvent,
};
use crate::core::node::{FlexStyle, NodeKind};
use crate::renderer::renderer::Renderer;
use crate::ui::RunOptions;
use crate::ui::actions::{action, text_action};
use crate::ui::app::App;
use crate::ui::builders::{button, column};
use crate::ui::element::{Element, Node};
use crate::ui::options::{BoxOptions, ButtonOptions};

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, width: f32, height: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, width, height, 1.0);
    renderer.end();
}

fn pointer(x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        x,
        y,
        button: PointerButton::Left,
        modifiers: Modifiers::default(),
    }
}

fn key(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers::default(),
        repeat: false,
    }
}

struct Clicks {
    clicks: u32,
}

impl App for Clicks {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            children: crate::nodes![button(ButtonOptions {
                key: Some("ok".into()),
                text: "OK".into(),
                on_click: Some(action(|state: &mut Clicks| state.clicks += 1)),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn pointer_click_runs_the_button_action_once() {
    let mut runtime = Runtime::init(Clicks { clicks: 0 }, RunOptions::default());
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    let id = runtime.find_key("ok").expect("button");
    let bounds = runtime.tree.get(id).bounds;
    let center = pointer(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    assert_eq!(runtime.dispatch(Event::PointerMove(center)), 1);
    assert_eq!(runtime.dispatch(Event::PointerDown(center)), 1);
    assert!(runtime.needs_build, "a press rebuilds");
    assert_eq!(runtime.dispatch(Event::PointerUp(center)), 1);
    assert_eq!(runtime.app_state_mut::<Clicks>().clicks, 1);
    // the next frame rebuilds and must not click again
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    assert_eq!(runtime.app_state_mut::<Clicks>().clicks, 1);
    assert_eq!(
        runtime.tree.hit_test(bounds.x + 1.0, bounds.y + 1.0),
        Some(id)
    );
}

#[test]
fn hover_over_static_ui_does_not_request_a_redraw() {
    struct Plain;
    impl App for Plain {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![crate::ui::text("hi", Default::default())],
                ..Default::default()
            })
        }
    }
    let mut runtime = Runtime::init(Plain, RunOptions::default());
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    assert_eq!(runtime.dispatch(Event::PointerMove(pointer(5.0, 5.0))), 0);
    assert!(!runtime.needs_build);
}

#[test]
fn tab_moves_focus_and_enter_activates_the_focused_button() {
    struct Two {
        first: u32,
        second: u32,
    }
    impl App for Two {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![
                    button(ButtonOptions {
                        key: Some("a".into()),
                        text: "A".into(),
                        on_click: Some(action(|state: &mut Two| state.first += 1)),
                        ..Default::default()
                    }),
                    button(ButtonOptions {
                        key: Some("b".into()),
                        text: "B".into(),
                        on_click: Some(action(|state: &mut Two| state.second += 1)),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            })
        }
    }
    let mut runtime = Runtime::init(
        Two {
            first: 0,
            second: 0,
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    let first = runtime.find_key("a").expect("a");
    let second = runtime.find_key("b").expect("b");
    assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Tab))), 1);
    assert_eq!(
        runtime.tree.focused,
        Some(first),
        "Tab focuses the first button"
    );
    assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Tab))), 1);
    assert_eq!(runtime.tree.focused, Some(second));
    assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Enter))), 1);
    assert_eq!(runtime.app_state_mut::<Two>().second, 1);
    assert_eq!(runtime.app_state_mut::<Two>().first, 0);
    // Escape drops the focus
    assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Escape))), 1);
    assert_eq!(runtime.tree.focused, None);
}

/// An input built from a raw `Element`, without the `ui::input` builder.
fn input_node(value: &str, on_text: crate::ui::TextAction) -> Node {
    Node::new(Element {
        kind: NodeKind::Input,
        content: crate::Str::copy(value),
        placeholder: "name".into(),
        layout: FlexStyle {
            width: Some(150.0),
            height: Some(32.0),
            shrink: 0.0,
            ..Default::default()
        },
        on_text: Some(on_text),
        // The `ui::input` builder marks the element focusable; the raw
        // element used here has to say it.
        focusable: true,
        ..Element::default()
    })
}

struct Form {
    text: String,
    edits: u32,
}

impl App for Form {
    fn render(&mut self) -> Node {
        let text = self.text.clone();
        column(BoxOptions {
            children: crate::nodes![input_node(
                &text,
                text_action(|state: &mut Form, value| {
                    state.text = value.to_string();
                    state.edits += 1;
                })
            )],
            ..Default::default()
        })
    }
}

fn click_into_input(runtime: &mut Runtime) {
    let id = runtime
        .tree
        .nodes
        .iter()
        .find(|node| node.alive && node.kind() == NodeKind::Input)
        .map(|node| node.id)
        .expect("input");
    let bounds = runtime.tree.get(id).bounds;
    let point = pointer(bounds.x + 20.0, bounds.y + bounds.height * 0.5);
    runtime.dispatch(Event::PointerMove(point));
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
    assert_eq!(runtime.tree.focused, Some(id));
}

#[test]
fn text_input_reaches_the_focused_input_and_its_action() {
    let mut runtime = Runtime::init(
        Form {
            text: String::new(),
            edits: 0,
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    click_into_input(&mut runtime);
    assert_eq!(runtime.dispatch(Event::TextInput(String::from("Zant"))), 1);
    assert_eq!(runtime.app_state_mut::<Form>().text, "Zant");
    assert_eq!(runtime.app_state_mut::<Form>().edits, 1);
    // a second event appends and reports through the action again
    assert_eq!(runtime.dispatch(Event::TextInput(String::from(" UI"))), 1);
    assert_eq!(runtime.app_state_mut::<Form>().text, "Zant UI");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    let id = runtime.tree.focused.expect("still focused");
    match &runtime.tree.get(id).data {
        NodeData::Input(data) => assert_eq!(data.text, "Zant UI"),
        _ => panic!("expected an input"),
    }
}

/// Handlers built with the values of their render (React's closures).
struct Snapshot {
    text: String,
    submitted: Vec<String>,
    renders: u32,
}

impl App for Snapshot {
    fn render(&mut self) -> Node {
        self.renders += 1;
        let seen = self.text.clone();
        column(BoxOptions {
            children: crate::nodes![crate::ui::input(crate::ui::options::InputOptions {
                key: Some("name".into()),
                value: Some(crate::Str::copy(&self.text)),
                on_change: Some(text_action(|state: &mut Snapshot, value| {
                    state.text = value.to_string();
                })),
                on_submit: Some(action(move |state: &mut Snapshot| {
                    state.submitted.push(seen.clone());
                })),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn a_key_after_typing_in_the_same_frame_sees_the_typed_text() {
    let mut runtime = Runtime::init(
        Snapshot {
            text: String::new(),
            submitted: Vec::new(),
            renders: 0,
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    click_into_input(&mut runtime);
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    let renders = runtime.app_state_mut::<Snapshot>().renders;
    // typing and Enter arrive before the next frame
    runtime.dispatch(Event::TextInput(String::from("abc")));
    runtime.dispatch(Event::KeyDown(key(Key::Enter)));
    let state = runtime.app_state_mut::<Snapshot>();
    assert_eq!(state.submitted, vec![String::from("abc")]);
    assert_eq!(state.renders, renders + 1, "one render between the events");
}

#[test]
fn events_without_app_code_do_not_render_early() {
    let mut runtime = Runtime::init(
        Snapshot {
            text: String::new(),
            submitted: Vec::new(),
            renders: 0,
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    let renders = runtime.app_state_mut::<Snapshot>().renders;
    // presses on the empty background, then keys nothing handles
    let empty = pointer(190.0, 110.0);
    runtime.dispatch(Event::PointerDown(empty));
    runtime.dispatch(Event::PointerUp(empty));
    runtime.dispatch(Event::KeyDown(key(Key::Escape)));
    assert_eq!(runtime.app_state_mut::<Snapshot>().renders, renders);
}

#[test]
fn composition_updates_the_input_and_an_empty_text_cancels_it() {
    let mut runtime = Runtime::init(
        Form {
            text: String::new(),
            edits: 0,
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer, 200.0, 120.0);
    click_into_input(&mut runtime);
    let id = runtime.tree.focused.expect("focused input");
    assert_eq!(
        runtime.dispatch(Event::Composition(Composition {
            text: String::from("zhong"),
            selection_start: 0,
            selection_end: 3,
        })),
        1
    );
    match &runtime.tree.get(id).data {
        NodeData::Input(data) => {
            assert!(data.has_composition);
            assert_eq!(data.composition, "zhong");
            assert_eq!(data.display_text, "zhong");
        }
        _ => panic!("expected an input"),
    }
    // an empty composition cancels the marked text
    assert_eq!(
        runtime.dispatch(Event::Composition(Composition {
            text: String::new(),
            selection_start: 0,
            selection_end: 0,
        })),
        1
    );
    match &runtime.tree.get(id).data {
        NodeData::Input(data) => assert!(!data.has_composition),
        _ => panic!("expected an input"),
    }
}

#[test]
fn close_and_quit_requests_are_vetoable_by_the_app() {
    struct Veto {
        closes: u32,
    }
    impl App for Veto {
        fn render(&mut self) -> Node {
            column(BoxOptions::default())
        }
        fn on_close_request(&mut self) -> bool {
            self.closes += 1;
            false
        }
    }
    let mut runtime = Runtime::init(Veto { closes: 0 }, RunOptions::default());
    assert_eq!(
        runtime.dispatch(Event::WindowCloseRequest),
        1,
        "veto redraws"
    );
    assert_eq!(runtime.app_state_mut::<Veto>().closes, 1);
    assert_eq!(runtime.dispatch(Event::AppQuitRequest), 1);
    assert_eq!(
        runtime.app_state_mut::<Veto>().closes,
        2,
        "quit asks on_close_request by default"
    );
}
