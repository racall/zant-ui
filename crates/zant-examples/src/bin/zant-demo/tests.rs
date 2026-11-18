//! Unit tests.

use super::*;
use zant::core::events::{Event, PointerEvent};
use zant::core::node::{NodeData, NodeId};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 700.0;

/// The keyed in-flow boxes `zant-demo` writes to `ZANT_LAYOUT_FILE` at
/// every window size (anchored to the top-left, so the keys are stable).
const LAYOUT_KEYS: [&str; 8] = [
    "theme",
    "theme-0",
    "theme-1",
    "theme-2",
    "preview-tabs",
    "preview-tabs-0",
    "preview-tabs-1",
    "preview-tabs-ink",
];

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

/// `State::init` normally decodes `crates/zant-examples/assets/zant.png`
/// relative to
/// the working directory (the repository root when the demo runs). Unit
/// tests run from the crate directory, so they decode the same file from
/// the manifest path explicitly.
fn demo_state() -> State {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/zant.png");
    let image = std::fs::read(path)
        .ok()
        .and_then(|bytes| Image::from_encoded(&bytes))
        .map(Rc::new);
    State {
        image,
        image_loaded: true,
        ..State::init()
    }
}

fn demo() -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(demo_state(), run_options());
    let mut renderer =
        Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer)
}

fn bounds(runtime: &Runtime, key: &str) -> zant::core::geometry::Rect {
    runtime.tree.get(runtime.find_key(key).expect(key)).bounds
}

fn find_button(runtime: &Runtime, label: &str) -> NodeId {
    for node in &runtime.tree.nodes {
        if node.alive
            && let NodeData::Button(data) = &node.data
            && data.label == label
        {
            return node.id;
        }
    }
    panic!("no button {label:?}");
}

fn find_input(runtime: &Runtime) -> NodeId {
    for node in &runtime.tree.nodes {
        if node.alive && matches!(node.data, NodeData::Input(_)) {
            return node.id;
        }
    }
    panic!("no input");
}

fn click(runtime: &mut Runtime, id: NodeId) {
    let box_ = runtime.tree.get(id).bounds;
    click_at(
        runtime,
        box_.x + box_.width * 0.5,
        box_.y + box_.height * 0.5,
    );
}

fn click_at(runtime: &mut Runtime, x: f32, y: f32) {
    let point = PointerEvent {
        x,
        y,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
}

#[test]
fn demo_has_the_layout_keys_and_fixed_boxes() {
    let (runtime, _renderer) = demo();
    let mut keys = Vec::new();
    for node in &runtime.tree.nodes {
        if node.alive
            && node.in_flow()
            && let Some(key) = node.key.as_deref()
        {
            keys.push(key.to_string());
        }
    }
    assert_eq!(keys, LAYOUT_KEYS, "the keyed boxes of the layout dump");

    // card 486 + padding 20 → the radio group starts at 32 + 20 and spans
    // the card's content width (layout dump: theme 52 319 446 24).
    let theme = bounds(&runtime, "theme");
    assert_eq!((theme.x, theme.width, theme.height), (52.0, 446.0, 24.0));
    // preview card starts at 32 + 486 + 18 and is 326 wide (layout dump:
    // preview-tabs 556 168 286 377).
    let tabs = bounds(&runtime, "preview-tabs");
    assert_eq!((tabs.x, tabs.width), (556.0, 286.0));
    let first_tab = bounds(&runtime, "preview-tabs-0");
    assert_eq!(first_tab.x, 556.0);
    // the ink bar sits on the active tab and is as wide as it
    let ink = bounds(&runtime, "preview-tabs-ink");
    assert_eq!(
        (ink.x, ink.width, ink.height),
        (556.0, first_tab.width, 2.0)
    );
    assert_eq!(
        ink.y + ink.height,
        tabs.y + 32.0,
        "ink bar at the bottom of the 32px tab strip"
    );
}

#[test]
fn demo_decodes_and_attaches_the_image() {
    let (runtime, _renderer) = demo();
    let mut found = false;
    for node in &runtime.tree.nodes {
        if node.alive
            && let NodeData::Image(data) = &node.data
        {
            assert!(data.handle.is_some(), "the image asset decoded");
            // the node box is fixed at 280x140, so the intrinsic size is
            // only informational here
            assert_eq!(data.fit, ImageFit::Cover);
            found = true;
        }
    }
    assert!(found, "the preview card contains the image");
}

#[test]
fn demo_toggle_callbacks_are_declared() {
    let (runtime, _renderer) = demo();
    let mut checkbox = 0;
    let mut switch = 0;
    let mut radio = 0;
    for node in &runtime.tree.nodes {
        if !node.alive {
            continue;
        }
        let NodeData::Toggle(data) = &node.data else {
            continue;
        };
        match (data.kind, data.index_action.is_some()) {
            (zant::core::node::ToggleKind::Checkbox, _) => {
                assert!(data.action.is_some(), "checkbox on_change");
                checkbox += 1;
            }
            (zant::core::node::ToggleKind::SwitchControl, _) => {
                assert!(data.action.is_some(), "switch on_change");
                switch += 1;
            }
            (zant::core::node::ToggleKind::Radio, true) => radio += 1,
            (zant::core::node::ToggleKind::Radio, false) => panic!("radio without on_change"),
        }
    }
    assert_eq!((checkbox, switch, radio), (1, 1, 3));
}

#[test]
fn demo_save_button_updates_the_state() {
    let (mut runtime, mut renderer) = demo();
    let save = find_button(&runtime, "Save Changes");
    click(&mut runtime, save);
    frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.app_state_mut::<State>().saved, 1);
    // the Details tab is not built, so the counter text stays out of the
    // tree; the state is the single source of truth
    let save = find_button(&runtime, "Save Changes");
    click(&mut runtime, save);
    frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.app_state_mut::<State>().saved, 2);
}

#[test]
fn demo_typing_updates_the_project_name() {
    let (mut runtime, mut renderer) = demo();
    let input = find_input(&runtime);
    let input_box = runtime.tree.get(input).bounds;
    // click at the right edge so the caret lands at the end of the value
    click_at(
        &mut runtime,
        input_box.x + input_box.width - 2.0,
        input_box.y + 12.0,
    );
    frame(&mut runtime, &mut renderer);
    runtime.dispatch(Event::TextInput(String::from("!")));
    frame(&mut runtime, &mut renderer);
    let state = runtime.app_state_mut::<State>();
    assert_eq!(state.name_len, 8, "one byte typed");
    assert!(state.name().contains('!'));
    let typed = state.name().into_owned();
    // the controlled input shows the new value
    let node = runtime.tree.get(find_input(&runtime));
    let NodeData::Input(data) = &node.data else {
        panic!("input node");
    };
    assert_eq!(data.text.as_str(), typed);
}
