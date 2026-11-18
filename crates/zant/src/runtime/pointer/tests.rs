//! Pointer dispatch tests for areas, click counting, toggles, and selects.

use super::Runtime;
use crate::Str;
use crate::core::events::{Event, Modifiers, PointerEvent};
use crate::core::node::{FlexStyle, NodeData, NodeKind, PointerArea};
use crate::platform::platform::Cursor;
use crate::renderer::renderer::Renderer;
use crate::ui::RunOptions;
use crate::ui::actions::{
    KeyAction, PointerAction, PointerInfo, PointerPhase, bool_action, index_action,
};
use crate::ui::app::App;
use crate::ui::builders::{checkbox, column, radio_group, select};
use crate::ui::element::{Element, Node};
use crate::ui::options::{
    BoxOptions, RadioGroupOptions, RadioOption, SelectOptions, ToggleOptions,
};
use std::sync::atomic::{AtomicI64, Ordering};

fn pointer(x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        x,
        y,
        button: crate::core::events::PointerButton::Left,
        modifiers: Modifiers::default(),
    }
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, 400.0, 300.0, 1.0);
    renderer.end();
}

#[derive(Default)]
struct Log {
    phases: Vec<PointerPhase>,
    infos: Vec<PointerInfo>,
    keys: u32,
}

impl Log {
    fn on_pointer(&mut self, info: PointerInfo) -> bool {
        self.phases.push(info.phase);
        self.infos.push(info);
        info.phase != PointerPhase::Hover
    }
}

impl App for Log {
    fn render(&mut self) -> Node {
        let area = PointerArea {
            action: PointerAction::new(|cx, info| cx.state::<Log>().on_pointer(info)),
            capture_scroll: true,
            clicks: true,
        };
        let key_action = KeyAction::new(|cx, key| {
            cx.state::<Log>().keys += 1;
            let _ = key;
            true
        });
        let target = Node::new(Element {
            kind: NodeKind::Container,
            layout: FlexStyle {
                width: Some(200.0),
                height: Some(100.0),
                ..Default::default()
            },
            pointer: Some(area),
            key_action: Some(key_action),
            cursor: Some(Cursor::Crosshair),
            focusable: true,
            ..Element::default()
        });
        column(BoxOptions {
            children: crate::nodes![target],
            ..Default::default()
        })
    }
}

#[test]
fn pointer_area_gets_local_coordinates_capture_and_wheel() {
    let mut runtime = Runtime::init(Log::default(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
    frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.dispatch(Event::PointerMove(pointer(30.0, 40.0))), 1);
    assert_eq!(runtime.dispatch(Event::PointerDown(pointer(30.0, 40.0))), 1);
    assert_eq!(
        runtime.dispatch(Event::PointerMove(pointer(390.0, 290.0))),
        1
    );
    assert_eq!(runtime.dispatch(Event::PointerUp(pointer(390.0, 290.0))), 1);
    assert_eq!(
        runtime.dispatch(Event::Scroll(crate::core::events::Scroll {
            x: 40.0,
            y: 40.0,
            dx: 0.0,
            dy: -12.0,
            modifiers: Modifiers::default(),
        })),
        1
    );
    {
        // the guard borrows the shared app state: release it before the
        // next dispatch
        let log = runtime.app_state_mut::<Log>();
        assert_eq!(
            log.phases,
            [
                PointerPhase::Hover,
                PointerPhase::Down,
                PointerPhase::Drag,
                PointerPhase::Up,
                PointerPhase::Scroll
            ]
        );
        let down = log.infos[1];
        assert_eq!((down.x, down.y), (30.0, 40.0), "local to the area");
        assert_eq!((down.window_x, down.window_y), (30.0, 40.0));
        assert_eq!(down.click_count, 1);
        let drag = log.infos[2];
        assert_eq!(
            (drag.x, drag.y),
            (390.0, 290.0),
            "captured outside the area"
        );
        let scroll = log.infos[4];
        assert_eq!((scroll.dx, scroll.dy), (0.0, -12.0));
    }
    // leaving the area sends `leave`
    runtime.dispatch(Event::PointerMove(pointer(390.0, 290.0)));
    runtime.dispatch(Event::PointerMove(pointer(390.0, 290.0)));
    let log = runtime.app_state_mut::<Log>();
    assert_eq!(log.phases.last(), Some(&PointerPhase::Leave));
}

#[test]
fn double_clicks_are_counted_in_the_pointer_info() {
    let mut runtime = Runtime::init(Log::default(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
    frame(&mut runtime, &mut renderer);
    for _ in 0..2 {
        runtime.dispatch(Event::PointerDown(pointer(50.0, 50.0)));
        runtime.dispatch(Event::PointerUp(pointer(50.0, 50.0)));
    }
    let log = runtime.app_state_mut::<Log>();
    let downs: Vec<u8> = log
        .infos
        .iter()
        .filter(|info| info.phase == PointerPhase::Down)
        .map(|info| info.click_count)
        .collect();
    assert_eq!(downs, [1, 2], "the second press is a double click");
}

#[test]
fn focused_pointer_area_sees_keys() {
    let mut runtime = Runtime::init(Log::default(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
    frame(&mut runtime, &mut renderer);
    runtime.dispatch(Event::PointerDown(pointer(10.0, 10.0)));
    runtime.dispatch(Event::PointerUp(pointer(10.0, 10.0)));
    assert!(runtime.tree.focused.is_some(), "a click focuses the area");
    runtime.dispatch(Event::KeyDown(crate::core::events::KeyEvent {
        key: crate::core::events::Key::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    assert_eq!(runtime.app_state_mut::<Log>().keys, 1);
}

// ── toggle activation and the select popup ─────────────────────────────

#[derive(Default)]
struct Form {
    checked: bool,
    changes: u32,
}

impl App for Form {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            children: crate::nodes![checkbox(ToggleOptions {
                key: Some("notify".into()),
                label: "Notify".into(),
                checked: self.checked,
                on_change: Some(bool_action(|state: &mut Form, checked| {
                    state.checked = checked;
                    state.changes += 1;
                })),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn a_click_activates_a_toggle_and_calls_its_callback_once() {
    crate::motion::motion::set_enabled(true);
    let mut runtime = Runtime::init(Form::default(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
    frame(&mut runtime, &mut renderer);
    let id = runtime.find_key("notify").expect("toggle");
    let bounds = runtime.tree.get(id).bounds;
    let point = pointer(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
    assert_eq!(runtime.app_state_mut::<Form>().changes, 1, "once per click");
    assert!(runtime.app_state_mut::<Form>().checked);
    assert!(
        matches!(&runtime.tree.get(id).data, NodeData::Toggle(data) if data.checked),
        "the retained state follows the callback"
    );
    // the toggle is queued for its check / mark transition
    assert!(runtime.animating_contains(id));
    frame(&mut runtime, &mut renderer);
    // a second click flips it back and fires the callback again
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
    assert_eq!(runtime.app_state_mut::<Form>().changes, 2);
    assert!(!runtime.app_state_mut::<Form>().checked);
}

#[derive(Default)]
struct Radios {
    selected: usize,
    changes: u32,
}

impl App for Radios {
    fn render(&mut self) -> Node {
        let options = ["S", "M", "L"]
            .iter()
            .map(|label| RadioOption {
                label: Str::from(*label),
                disabled: false,
            })
            .collect();
        column(BoxOptions {
            children: crate::nodes![radio_group(RadioGroupOptions {
                key: Some("size".into()),
                options,
                selected: self.selected,
                on_change: Some(index_action(|state: &mut Radios, index| {
                    state.selected = index;
                    state.changes += 1;
                })),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn a_radio_only_turns_on_and_calls_its_index_action_when_it_was_off() {
    let mut runtime = Runtime::init(Radios::default(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
    frame(&mut runtime, &mut renderer);
    let click_key = |runtime: &mut Runtime, key: &str| {
        let bounds = runtime.tree.get(runtime.find_key(key).expect(key)).bounds;
        let point = pointer(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
    };
    click_key(&mut runtime, "size-1");
    assert_eq!(runtime.app_state_mut::<Radios>().changes, 1);
    assert_eq!(runtime.app_state_mut::<Radios>().selected, 1);
    assert!(
        matches!(&runtime.tree.get(runtime.find_key("size-1").expect("radio")).data,
            NodeData::Toggle(data) if data.checked)
    );
    // a click on the already checked radio must not fire the callback
    frame(&mut runtime, &mut renderer);
    click_key(&mut runtime, "size-1");
    assert_eq!(runtime.app_state_mut::<Radios>().changes, 1);
    // another radio fires it again
    click_key(&mut runtime, "size-2");
    assert_eq!(runtime.app_state_mut::<Radios>().changes, 2);
    assert_eq!(runtime.app_state_mut::<Radios>().selected, 2);
}

#[derive(Default)]
struct Picker {
    selected: usize,
    changes: u32,
}

impl App for Picker {
    fn render(&mut self) -> Node {
        let options = ["Starter", "Pro", "Enterprise"]
            .iter()
            .map(|label| Str::from(*label))
            .collect();
        column(BoxOptions {
            children: crate::nodes![select(SelectOptions {
                key: Some("plan".into()),
                options,
                selected: self.selected,
                width: Some(200.0),
                on_change: Some(index_action(|state: &mut Picker, index| {
                    state.selected = index;
                    state.changes += 1;
                })),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn the_select_popup_opens_hovers_and_selects_an_option() {
    crate::motion::motion::set_enabled(true);
    static NOW: AtomicI64 = AtomicI64::new(1_000);
    fn clock() -> i64 {
        NOW.load(Ordering::Relaxed)
    }
    NOW.store(1_000, Ordering::Relaxed);
    let theme = crate::theme::tokens::ant_light();
    let mut runtime = Runtime::init(Picker::default(), RunOptions::default());
    runtime.clock = clock;
    let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
    frame(&mut runtime, &mut renderer);
    let id = runtime.find_key("plan").expect("select");
    let bounds = runtime.tree.get(id).bounds;
    let field = pointer(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    runtime.dispatch(Event::PointerDown(field));
    runtime.dispatch(Event::PointerUp(field));
    assert_eq!(runtime.open_select, Some(id));
    assert!(matches!(&runtime.tree.get(id).data, NodeData::Select(data) if data.open));
    // the opening transition runs and the popup is painted after the tree
    frame(&mut runtime, &mut renderer);
    NOW.store(1_400, Ordering::Relaxed);
    frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.popup_select, Some(id));
    let rect = crate::components::select::popup_rect(runtime.tree.get(id), runtime.viewport);
    let popup_pixel = renderer
        .pixel_at(
            (rect.x as usize) + (rect.width as usize) - 8,
            (rect.y as usize) + 10,
        )
        .expect("popup pixel");
    assert_eq!(
        popup_pixel, theme.color_bg_elevated,
        "the dropdown panel covers the page below the field"
    );
    // hovering an option highlights it (pointer hit test runs before the
    // tree while the popup is open)
    let row = |index: usize| {
        pointer(
            rect.x + rect.width * 0.5,
            rect.y
                + crate::components::select::POPUP_PADDING
                + index as f32 * crate::components::select::ROW_HEIGHT
                + 15.0,
        )
    };
    runtime.dispatch(Event::PointerMove(row(1)));
    assert!(
        matches!(&runtime.tree.get(id).data, NodeData::Select(data) if data.hovered_option == Some(1))
    );
    // clicking an option selects it, fires the index action once and
    // closes the popup
    runtime.dispatch(Event::PointerDown(row(2)));
    assert_eq!(runtime.app_state_mut::<Picker>().changes, 1);
    assert_eq!(runtime.app_state_mut::<Picker>().selected, 2);
    assert!(
        matches!(&runtime.tree.get(id).data, NodeData::Select(data) if data.selected == 2 && !data.open)
    );
    assert_eq!(runtime.open_select, None);
    runtime.dispatch(Event::PointerUp(row(2)));
}
