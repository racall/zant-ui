//! Headless smoke tests for the `zant-components` binary: the same state /
//! tree the window runs is built and painted on a raster canvas through
//! [`Runtime`], and a few real events exercise the `[event]` log.

use zant::core::events::{Event, PointerEvent};
use zant::core::node::NodeData;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::theme::tokens;
use zant_examples::components_app::{ComponentsApp, run_options};

const WIDTH: f32 = 1100.0;
const HEIGHT: f32 = 820.0;

/// The layout keys of every section of `examples/components.zig` (the
/// `ZANT_LAYOUT_FILE` keys of the Zig run, minus the `pdf-2002*` keys of the
/// not-yet-ported PDF viewer, see the WP-2.9 report).
const KEYS: [&str; 46] = [
    "page",
    "last-event",
    "btn-primary",
    "btn-default",
    "btn-text",
    "btn-link",
    "btn-disabled",
    "btn-small",
    "btn-large",
    "icon-btn",
    "link",
    "input",
    "input-password",
    "checkbox",
    "switch",
    "radio",
    "radio-0",
    "radio-1",
    "radio-2",
    "select",
    "segmented",
    "segmented-thumb",
    "segmented-0",
    "segmented-1",
    "segmented-2",
    "tabs",
    "tabs-0",
    "tabs-1",
    "tabs-ink",
    "progress",
    "progress-more",
    "image",
    "tooltip-btn",
    "menu-btn",
    "modal-btn",
    "message-btn",
    "dark-btn",
    "track",
    "list-box",
    "vlist",
    "vlist-viewport",
    "select-text",
    "widget-append",
    "md-2001",
    "md-2001-e1",
    "pdf-2002",
];

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("frame canvas");
    runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
    renderer.end();
}

fn app(dark: bool) -> (Runtime, Renderer) {
    let mut runtime = Runtime::init(ComponentsApp::new(dark, false, true), run_options());
    let mut renderer =
        Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
    frame(&mut runtime, &mut renderer);
    (runtime, renderer)
}

fn click(runtime: &mut Runtime, renderer: &mut Renderer, key: &str) {
    let node = runtime.find_key(key).expect("the keyed node");
    let bounds = runtime.tree.get(node).bounds;
    let point = PointerEvent {
        x: bounds.x + bounds.width * 0.5,
        y: bounds.y + bounds.height * 0.5,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
    frame(runtime, renderer);
}

#[test]
fn components_app_builds_every_section_and_paints() {
    let (runtime, _renderer) = app(false);
    for key in KEYS {
        assert!(
            runtime.find_key(key).is_some(),
            "the components example builds the {key:?} node"
        );
    }
    // the two lines of the Markdown block and the placeholder of the PDF slot
    assert!(runtime.find_key("md-2001").is_some());
    // the whole tree painted; 10 000-row virtual list only builds the window
    let text_nodes = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && matches!(node.data, NodeData::Text(_)))
        .count();
    assert!(
        text_nodes > 40,
        "the labels of every component are in the tree"
    );
}

#[test]
fn components_buttons_log_their_events() {
    let (mut runtime, mut renderer) = app(false);
    click(&mut runtime, &mut renderer, "btn-primary");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("click primary")
    );
    click(&mut runtime, &mut renderer, "btn-small");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("click primary")
    );
    click(&mut runtime, &mut renderer, "progress-more");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("progress 60")
    );
    // the disabled button never runs its callback
    click(&mut runtime, &mut renderer, "btn-disabled");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("progress 60")
    );
}

#[test]
fn components_toggles_and_segments_update_the_state() {
    let (mut runtime, mut renderer) = app(false);
    click(&mut runtime, &mut renderer, "checkbox");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("checkbox false")
    );
    click(&mut runtime, &mut renderer, "radio-1");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("radio 1")
    );
    click(&mut runtime, &mut renderer, "segmented-2");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("segmented 2")
    );
    click(&mut runtime, &mut renderer, "tabs-1");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("tab 1")
    );
}

#[test]
fn components_menu_modal_and_message_open_and_close() {
    let (mut runtime, mut renderer) = app(false);
    click(&mut runtime, &mut renderer, "menu-btn");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("menu open")
    );
    assert!(runtime.find_key("menu").is_some(), "the dropdown is built");
    click(&mut runtime, &mut renderer, "menu-new");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("menu new")
    );
    assert!(runtime.find_key("menu").is_none(), "choosing closes it");

    click(&mut runtime, &mut renderer, "modal-btn");
    assert!(runtime.find_key("modal").is_some(), "the modal is built");
    click(&mut runtime, &mut renderer, "modal-ok");
    assert_eq!(
        runtime.app_state_mut::<ComponentsApp>().last_event(),
        Some("modal close")
    );
    assert!(runtime.find_key("modal").is_none());

    click(&mut runtime, &mut renderer, "message-btn");
    assert!(
        runtime.find_key("message").is_some(),
        "the message is shown"
    );
}

#[test]
fn components_start_dark_when_asked() {
    let (runtime, _renderer) = app(true);
    assert!(tokens::is_dark(runtime.tree.theme));
    // the dark button flips the theme back on the next frame
    let (mut runtime, mut renderer) = app(true);
    click(&mut runtime, &mut renderer, "dark-btn");
    assert!(!tokens::is_dark(runtime.tree.theme));
}

thread_local! {
    /// Fake clock of the autoplay test (the runtime samples it per frame).
    static AUTOPLAY_NOW: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

fn autoplay_clock() -> i64 {
    AUTOPLAY_NOW.with(std::cell::Cell::get)
}

#[test]
fn components_autoplay_quiet_runs_without_stderr_events() {
    // ZANT_AUTOPLAY=quiet: the clock drives the menu / modal / message cycle
    // and the event lines are suppressed (the state still updates).
    let mut runtime = Runtime::init(ComponentsApp::new(false, true, true), run_options());
    runtime.clock = autoplay_clock;
    let mut renderer =
        Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
    for step in 0..6 {
        AUTOPLAY_NOW.with(|now| now.set(1000 + step * 700));
        frame(&mut runtime, &mut renderer);
    }
    let state = runtime.app_state_mut::<ComponentsApp>();
    assert!(state.last_event().is_some(), "autoplay changed state");
}
