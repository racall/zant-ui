//! The declarative runtime: build, input
//! editing, actions, select popup, password masking, `styled` overrides and
//! `:focus-visible` focus rings.

use zant::components::select;
use zant::core::events::{Event, Key, KeyEvent, PointerEvent};
use zant::core::node::{InputData, NodeData, NodeId, NodeKind};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::actions::{action, index_action, text_action};
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{
    BoxOptions, ButtonOptions, InputOptions, SelectOptions, TextOptions, ToggleOptions,
};
use zant::ui::{
    Style, button, checkbox, column, heading, input, inset, paragraph, select as select_ui, styled,
    r#switch, text,
};

struct State {
    saved: u32,
    name: String,
    plan: usize,
}

impl State {
    fn new() -> State {
        State {
            saved: 0,
            name: String::new(),
            plan: 0,
        }
    }
}

impl App for State {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            gap: 12.0,
            padding: inset(24.0),
            children: zant::nodes![
                heading(
                    "Settings",
                    zant::ui::HeadingOptions {
                        level: 1,
                        ..Default::default()
                    }
                ),
                paragraph(
                    "Declarative Zant UI runtime",
                    zant::ui::ParagraphOptions::default()
                ),
                input(InputOptions {
                    value: Some(self.name.clone().into()),
                    placeholder: "Project name".into(),
                    on_change: Some(text_action(|state: &mut State, value: &str| {
                        state.name = value.to_string();
                    })),
                    ..Default::default()
                }),
                button(ButtonOptions {
                    text: "Save Changes".into(),
                    variant: zant::core::node::ButtonType::Primary,
                    on_click: Some(action(|state: &mut State| state.saved += 1)),
                    ..Default::default()
                }),
                checkbox(ToggleOptions {
                    label: "Notifications".into(),
                    checked: true,
                    ..Default::default()
                }),
                r#switch(ToggleOptions {
                    label: "Beta updates".into(),
                    checked: false,
                    ..Default::default()
                }),
                select_ui(SelectOptions {
                    options: vec!["Starter".into(), "Pro".into(), "Enterprise".into()],
                    selected: self.plan,
                    width: Some(220.0),
                    on_change: Some(index_action(|state: &mut State, index: usize| {
                        state.plan = index;
                    })),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

fn find_kind(runtime: &Runtime, kind: NodeKind) -> Option<NodeId> {
    for node in &runtime.tree.nodes {
        if node.alive && node.data.kind() == kind {
            return Some(node.id);
        }
    }
    None
}

fn click(runtime: &mut Runtime, id: NodeId) {
    let b = runtime.tree.get(id).bounds;
    let p = PointerEvent {
        x: b.x + b.width * 0.5,
        y: b.y + b.height * 0.5,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerMove(p));
    runtime.dispatch(Event::PointerDown(p));
    runtime.dispatch(Event::PointerUp(p));
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, width: f32, height: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, width, height, 1.0);
    renderer.end();
}

fn window_options(width: f32, height: f32) -> RunOptions {
    RunOptions {
        title: "Test".into(),
        window: zant::platform::platform::WindowOptions {
            width,
            height,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn declarative_runtime_renders_edits_input_and_invokes_actions() {
    let mut runtime = Runtime::init(State::new(), window_options(520.0, 420.0));
    let mut renderer = Renderer::create_raster(520, 420).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 520.0, 420.0);
    // renderer.draw path was exercised by render_frame above
    assert!(runtime.tree.nodes.len() >= 6);

    let input_id = find_kind(&runtime, NodeKind::Input).expect("input");
    click(&mut runtime, input_id);
    assert_eq!(runtime.tree.focused, Some(input_id));
    runtime.dispatch(Event::TextInput("Zant".into()));
    assert_eq!(runtime.app_state_mut::<State>().name, "Zant");

    frame(&mut runtime, &mut renderer, 520.0, 420.0);
    let input_id = find_kind(&runtime, NodeKind::Input).expect("input");
    let NodeData::Input(data) = &runtime.tree.get(input_id).data else {
        panic!("expected an input node");
    };
    assert_eq!(data.text, "Zant");

    let button_id = find_kind(&runtime, NodeKind::Button).expect("button");
    click(&mut runtime, button_id);
    assert_eq!(runtime.app_state_mut::<State>().saved, 1);

    let toggle_id = find_kind(&runtime, NodeKind::Toggle).expect("toggle");
    let checked_before = {
        let NodeData::Toggle(data) = &runtime.tree.get(toggle_id).data else {
            panic!("expected a toggle node");
        };
        data.checked
    };
    click(&mut runtime, toggle_id);
    let NodeData::Toggle(data) = &runtime.tree.get(toggle_id).data else {
        panic!("expected a toggle node");
    };
    assert!(!data.checked || !checked_before);
    assert!(checked_before, "the first toggle is the checked checkbox");
    frame(&mut runtime, &mut renderer, 520.0, 420.0);

    let select_id = find_kind(&runtime, NodeKind::Select).expect("select");
    click(&mut runtime, select_id);
    let NodeData::Select(data) = &runtime.tree.get(select_id).data else {
        panic!("expected a select node");
    };
    assert!(data.open);
    let popup = select::popup_rect(runtime.tree.get(select_id), runtime.viewport);
    let p = PointerEvent {
        x: popup.x + 10.0,
        y: popup.y + select::POPUP_PADDING + select::ROW_HEIGHT * 2.0 + select::ROW_HEIGHT * 0.5,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerMove(p));
    runtime.dispatch(Event::PointerDown(p));
    runtime.dispatch(Event::PointerUp(p));
    assert_eq!(runtime.app_state_mut::<State>().plan, 2);
    frame(&mut runtime, &mut renderer, 520.0, 420.0);
    let select_id = find_kind(&runtime, NodeKind::Select).expect("select");
    let NodeData::Select(data) = &runtime.tree.get(select_id).data else {
        panic!("expected a select node");
    };
    assert_eq!(data.selected, 2);
    assert!(runtime.tree.nodes.len() >= 6);

    // screenshot smoke check, written to the temp dir so the repository
    // stays clean
    let png = std::env::temp_dir().join("zant-test-ui-runtime.png");
    assert!(renderer.write_png(png.to_string_lossy().as_ref()));
    let _ = std::fs::remove_file(&png);
}

#[test]
fn input_keeps_its_intrinsic_height_in_a_short_window() {
    let mut runtime = Runtime::init(State::new(), window_options(520.0, 200.0));
    let mut renderer = Renderer::create_raster(520, 200).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 520.0, 200.0);

    let input_id = find_kind(&runtime, NodeKind::Input).expect("input");
    assert!((runtime.tree.get(input_id).bounds.height - 32.0).abs() <= 0.01);
}

#[test]
fn password_input_masks_display_text() {
    let mut data = InputData {
        placeholder: "Password".into(),
        password: true,
        ..Default::default()
    };
    data.text.push_str("secret");
    data.cursor = data.text.len();
    data.anchor = data.cursor;
    data.rebuild_display();
    assert_eq!(data.display_text, "******");
}

#[test]
fn styled_overrides_common_visual_properties() {
    let node = styled(
        text("X", TextOptions::default()),
        Style {
            background: Some(0xff112233),
            border_color: Some(0xff445566),
            border_width: Some(2.0),
            radius: Some(5.0),
            padding: Some(inset(3.0)),
            ..Default::default()
        },
    );
    let e = node.element();
    assert_eq!(e.visual.background, Some(0xff112233));
    assert_eq!(e.visual.border_color, Some(0xff445566));
    assert_eq!(e.visual.border_width, 2.0);
    assert_eq!(e.visual.radius, 5.0);
    assert_eq!(e.layout.padding.left, 3.0);
}

struct Focus {
    clicks: u32,
}

impl App for Focus {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            padding: inset(10.0),
            gap: 10.0,
            children: zant::nodes![
                button(ButtonOptions {
                    key: Some("a".into()),
                    text: "A".into(),
                    on_click: Some(action(|state: &mut Focus| state.clicks += 1)),
                    ..Default::default()
                }),
                r#switch(ToggleOptions {
                    key: Some("s".into()),
                    label: "S".into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn focus_rings_follow_focus_visible_keyboard_focus_shows_one_a_click_does_not() {
    zant::motion::motion::set_enabled(false);
    let mut runtime = Runtime::init(Focus { clicks: 0 }, window_options(300.0, 200.0));
    let mut renderer = Renderer::create_raster(300, 200).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 300.0, 200.0);

    // mouse: focused (Space still activates it) but no ring
    let a = runtime.tree.id_of_key("a").expect("button a");
    click(&mut runtime, a);
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    assert_eq!(runtime.app_state_mut::<Focus>().clicks, 1);
    {
        let NodeData::Button(data) = &runtime.tree.get(a).data else {
            panic!("expected a button node");
        };
        assert!(data.focused);
        assert!(!data.focus_visible);
    }
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Character(' '),
        modifiers: Default::default(),
        repeat: false,
    }));
    assert_eq!(runtime.app_state_mut::<Focus>().clicks, 2);

    // keyboard: Tab moves focus with a ring
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Tab,
        modifiers: Default::default(),
        repeat: false,
    }));
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let s = runtime.tree.id_of_key("s").expect("switch s");
    {
        let NodeData::Toggle(data) = &runtime.tree.get(s).data else {
            panic!("expected a toggle node");
        };
        assert!(data.focused);
        assert!(data.focus_visible);
    }
    {
        let NodeData::Button(data) = &runtime.tree.get(a).data else {
            panic!("expected a button node");
        };
        assert!(!data.focus_visible);
    }

    // clicking the switch drops the ring again
    click(&mut runtime, s);
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let NodeData::Toggle(data) = &runtime.tree.get(s).data else {
        panic!("expected a toggle node");
    };
    assert!(!data.focus_visible);
    zant::motion::motion::set_enabled(true);
}
