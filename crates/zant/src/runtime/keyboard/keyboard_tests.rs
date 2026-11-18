//! Runtime tests for input editing, control activation, and modal dismissal.

use super::Runtime;
use crate::core::events::{Event, Key, KeyEvent, Modifiers, PointerButton, PointerEvent};
use crate::core::node::{FlexStyle, NodeData, NodeKind};
use crate::renderer::renderer::Renderer;
use crate::ui::RunOptions;
use crate::ui::actions::{action, bool_action, index_action, text_action};
use crate::ui::app::App;
use crate::ui::builders::{checkbox, column, select};
use crate::ui::element::{Element, Node};
use crate::ui::options::{BoxOptions, SelectOptions, ToggleOptions};

struct Form {
    text: String,
    edits: u32,
    submits: u32,
    undos: u32,
    redos: u32,
}

impl App for Form {
    fn render(&mut self) -> Node {
        let text = self.text.clone();
        let input = Node::new(Element {
            kind: NodeKind::Input,
            content: crate::Str::copy(&text),
            placeholder: "name".into(),
            focusable: true,
            layout: FlexStyle {
                width: Some(150.0),
                height: Some(32.0),
                shrink: 0.0,
                ..Default::default()
            },
            on_text: Some(text_action(|state: &mut Form, value| {
                state.text = value.to_string();
                state.edits += 1;
            })),
            on_click: Some(action(|state: &mut Form| state.submits += 1)),
            on_undo: Some(action(|state: &mut Form| state.undos += 1)),
            on_redo: Some(action(|state: &mut Form| state.redos += 1)),
            ..Element::default()
        });
        column(BoxOptions {
            children: crate::nodes![input],
            ..Default::default()
        })
    }
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, 200.0, 120.0, 1.0);
    renderer.end();
}

fn key(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers::default(),
        repeat: false,
    }
}

fn primary(key: Key, shift: bool) -> KeyEvent {
    let mut modifiers = Modifiers::default();
    if crate::core::keys::IS_MACOS {
        modifiers.meta = true;
    } else {
        modifiers.control = true;
    }
    modifiers.shift = shift;
    KeyEvent {
        key,
        modifiers,
        repeat: false,
    }
}

fn focused_form() -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(
        Form {
            text: String::new(),
            edits: 0,
            submits: 0,
            undos: 0,
            redos: 0,
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer);
    let id = runtime
        .tree
        .nodes
        .iter()
        .find(|node| node.alive && node.kind() == NodeKind::Input)
        .map(|node| node.id)
        .expect("input");
    let bounds = runtime.tree.get(id).bounds;
    let point = PointerEvent {
        x: bounds.x + 20.0,
        y: bounds.y + bounds.height * 0.5,
        button: PointerButton::Left,
        modifiers: Modifiers::default(),
    };
    runtime.dispatch(Event::PointerMove(point));
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
    assert_eq!(runtime.tree.focused, Some(id));
    (runtime, renderer)
}

#[test]
fn control_characters_are_never_typed_and_the_action_runs_once() {
    let (mut runtime, _renderer) = focused_form();
    assert_eq!(
        runtime.dispatch(Event::TextInput(String::from("\u{1}ab\u{7f}c"))),
        1
    );
    assert_eq!(runtime.app_state_mut::<Form>().text, "abc");
    assert_eq!(runtime.app_state_mut::<Form>().edits, 1);
    // a value of only control characters is dropped
    assert_eq!(
        runtime.dispatch(Event::TextInput(String::from("\u{1}\u{7f}"))),
        0
    );
    assert_eq!(runtime.app_state_mut::<Form>().text, "abc");
}

#[test]
fn backspace_and_delete_edit_the_focused_input_through_the_tree() {
    let (mut runtime, _renderer) = focused_form();
    runtime.dispatch(Event::TextInput(String::from("abc")));
    runtime.dispatch(Event::KeyDown(key(Key::Backspace)));
    assert_eq!(runtime.app_state_mut::<Form>().text, "ab");
    // Delete at the end of the text changes nothing
    runtime.dispatch(Event::KeyDown(key(Key::Delete)));
    assert_eq!(runtime.app_state_mut::<Form>().text, "ab");
    // Left moves the caret, Delete then removes the following character
    runtime.dispatch(Event::KeyDown(key(Key::Left)));
    runtime.dispatch(Event::KeyDown(key(Key::Delete)));
    assert_eq!(runtime.app_state_mut::<Form>().text, "a");
    assert_eq!(runtime.app_state_mut::<Form>().edits, 3);
}

#[test]
fn enter_submits_single_line_inputs() {
    let (mut runtime, _renderer) = focused_form();
    assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Enter))), 1);
    assert_eq!(runtime.app_state_mut::<Form>().submits, 1);
}

#[test]
fn undo_and_redo_hooks_run_for_the_platform_shortcuts() {
    let (mut runtime, _renderer) = focused_form();
    assert_eq!(
        runtime.dispatch(Event::KeyDown(primary(Key::Character('z'), false))),
        1
    );
    assert_eq!(runtime.app_state_mut::<Form>().undos, 1);
    assert_eq!(
        runtime.dispatch(Event::KeyDown(primary(Key::Character('z'), true))),
        1
    );
    assert_eq!(runtime.app_state_mut::<Form>().redos, 1);
    assert_eq!(
        runtime.dispatch(Event::KeyDown(primary(Key::Character('y'), false))),
        1
    );
    assert_eq!(runtime.app_state_mut::<Form>().redos, 2);
    // cmd/ctrl+A selects the whole text
    assert_eq!(
        runtime.dispatch(Event::KeyDown(primary(Key::Character('a'), false))),
        1
    );
    let id = runtime.tree.focused.expect("input");
    match &runtime.tree.get(id).data {
        NodeData::Input(data) => {
            assert_eq!(data.anchor, 0);
            assert_eq!(data.cursor, data.text.len());
        }
        _ => panic!("expected an input"),
    }
}

// ── keyboard activation of toggles and selects ─────────────────────────

#[derive(Default)]
struct Options {
    checked: bool,
    changes: u32,
    selected: usize,
    picks: u32,
}

impl App for Options {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            children: crate::nodes![
                checkbox(ToggleOptions {
                    key: Some("notify".into()),
                    label: "Notify".into(),
                    checked: self.checked,
                    on_change: Some(bool_action(|state: &mut Options, checked| {
                        state.checked = checked;
                        state.changes += 1;
                    })),
                    ..Default::default()
                }),
                select(SelectOptions {
                    key: Some("plan".into()),
                    options: ["Starter", "Pro", "Enterprise"]
                        .iter()
                        .map(|label| crate::Str::from(*label))
                        .collect(),
                    selected: self.selected,
                    width: Some(150.0),
                    on_change: Some(index_action(|state: &mut Options, index| {
                        state.selected = index;
                        state.picks += 1;
                    })),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

fn options_runtime() -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(Options::default(), RunOptions::default());
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer)
}

#[test]
fn space_and_enter_activate_a_focused_toggle_once_each() {
    let (mut runtime, _renderer) = options_runtime();
    assert_eq!(runtime.tree.focused, None);
    runtime.dispatch(Event::KeyDown(key(Key::Tab)));
    let toggle = runtime.tree.focused.expect("Tab focuses the checkbox");
    assert!(matches!(
        &runtime.tree.get(toggle).data,
        NodeData::Toggle(_)
    ));
    runtime.dispatch(Event::KeyDown(key(Key::Character(' '))));
    assert!(runtime.app_state_mut::<Options>().checked);
    assert_eq!(runtime.app_state_mut::<Options>().changes, 1);
    runtime.dispatch(Event::KeyDown(key(Key::Enter)));
    assert!(!runtime.app_state_mut::<Options>().checked);
    assert_eq!(
        runtime.app_state_mut::<Options>().changes,
        2,
        "one callback per key press"
    );
}

#[test]
fn enter_opens_the_popup_and_arrows_move_the_selection() {
    let (mut runtime, _renderer) = options_runtime();
    // Tab reaches the checkbox first and the select second
    runtime.dispatch(Event::KeyDown(key(Key::Tab)));
    runtime.dispatch(Event::KeyDown(key(Key::Tab)));
    let select_id = runtime.find_key("plan").expect("select");
    assert_eq!(runtime.tree.focused, Some(select_id));
    assert_eq!(
        runtime.dispatch(Event::KeyDown(key(Key::Enter))),
        1,
        "Enter opens the popup"
    );
    assert_eq!(runtime.open_select, Some(select_id));
    assert!(matches!(&runtime.tree.get(select_id).data, NodeData::Select(data) if data.open));
    // Down / Up move the selected option and fire the index action
    runtime.dispatch(Event::KeyDown(key(Key::Down)));
    assert_eq!(runtime.app_state_mut::<Options>().selected, 1);
    assert_eq!(runtime.app_state_mut::<Options>().picks, 1);
    runtime.dispatch(Event::KeyDown(key(Key::Down)));
    assert_eq!(runtime.app_state_mut::<Options>().selected, 2);
    runtime.dispatch(Event::KeyDown(key(Key::Up)));
    assert_eq!(runtime.app_state_mut::<Options>().selected, 1);
    assert_eq!(runtime.app_state_mut::<Options>().picks, 3);
    // Escape closes the popup
    runtime.dispatch(Event::KeyDown(key(Key::Escape)));
    assert_eq!(runtime.open_select, None);
    assert!(matches!(&runtime.tree.get(select_id).data, NodeData::Select(data) if !data.open));
}

struct Dialogs {
    outer: bool,
    inner: bool,
    closed: Vec<&'static str>,
}

impl App for Dialogs {
    fn render(&mut self) -> Node {
        let mut children = Vec::new();
        for (open, name) in [(self.outer, "outer"), (self.inner, "inner")] {
            if open {
                children.push(crate::components::modal::modal(
                    crate::components::modal::ModalOptions {
                        key: name.into(),
                        title: name.into(),
                        on_close: Some(action(move |s: &mut Dialogs| {
                            s.closed.push(name);
                            if name == "inner" {
                                s.inner = false;
                            } else {
                                s.outer = false;
                            }
                        })),
                        ..Default::default()
                    },
                ));
            }
        }
        column(BoxOptions {
            children,
            ..Default::default()
        })
    }
}

/// Esc closes the topmost open modal first, then the one below.
#[test]
fn escape_closes_the_topmost_modal() {
    let mut runtime = Runtime::init(
        Dialogs {
            outer: true,
            inner: true,
            closed: Vec::new(),
        },
        RunOptions::default(),
    );
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Escape))), 1);
    frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.app_state_mut::<Dialogs>().closed, ["inner"]);
    runtime.dispatch(Event::KeyDown(key(Key::Escape)));
    frame(&mut runtime, &mut renderer);
    assert_eq!(
        runtime.app_state_mut::<Dialogs>().closed,
        ["inner", "outer"]
    );
    // nothing open: Esc only drops the focus
    runtime.dispatch(Event::KeyDown(key(Key::Escape)));
    assert_eq!(runtime.app_state_mut::<Dialogs>().closed.len(), 2);
}
