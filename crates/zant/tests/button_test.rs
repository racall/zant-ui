//! antd button `danger` / `block` / `loading`: painted colours per state in
//! both themes, the block width, the loading click guard and its spinner
//! frames (repaint only, no rebuild).
//!
//! The motion switch is process-wide, so every test here runs with motion off
//! except `the_loading_spinner_turns_without_rebuilding`, which owns it under
//! `MOTION`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

use zant::core::events::{Event, Key, KeyEvent, Modifiers, PointerEvent};
use zant::core::node::{ButtonType, NodeData};
use zant::motion::motion;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::theme::tokens::{self, Theme, ant_dark, ant_light};
use zant::ui::actions::action;
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{BoxOptions, ButtonOptions};
use zant::ui::{button, column};

static MOTION: Mutex<()> = Mutex::new(());

const W: f32 = 400.0;
const H: f32 = 240.0;

struct Panel {
    clicks: usize,
    loading: bool,
}

impl App for Panel {
    fn render(&mut self) -> Node {
        let item = |key: &'static str, variant: ButtonType, danger: bool| {
            button(ButtonOptions {
                key: Some(key.into()),
                text: "Delete".into(),
                variant,
                danger,
                ..Default::default()
            })
        };
        column(BoxOptions {
            gap: 8.0,
            padding: zant::core::geometry::Edges::all(8.0),
            children: zant::nodes![
                item("primary", ButtonType::Primary, true),
                item("default", ButtonType::Default, true),
                button(ButtonOptions {
                    key: Some("block".into()),
                    text: "Wide".into(),
                    block: true,
                    ..Default::default()
                }),
                button(ButtonOptions {
                    key: Some("load".into()),
                    text: "Save".into(),
                    variant: ButtonType::Primary,
                    loading: self.loading,
                    on_click: Some(action(|s: &mut Panel| s.clicks += 1)),
                    ..Default::default()
                }),
                button(ButtonOptions {
                    key: Some("only".into()),
                    icon: zant::ui::Icon::Search,
                    loading: self.loading,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

fn runtime(theme: Theme, loading: bool) -> (Runtime, Renderer) {
    let runtime = Runtime::init(
        Panel { clicks: 0, loading },
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: W,
                height: H,
                ..Default::default()
            },
            theme,
            ..Default::default()
        },
    );
    let renderer = Renderer::create_raster(W as u32, H as u32).expect("raster renderer");
    (runtime, renderer)
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, W, H, 1.0);
    renderer.end();
}

fn point(x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        x,
        y,
        ..Default::default()
    }
}

fn close(actual: u32, expected: u32) -> bool {
    [16, 8, 0]
        .iter()
        .all(|shift| ((actual >> shift) & 255).abs_diff((expected >> shift) & 255) <= 3)
}

/// The opaque pixel `color` shows over `page`.
fn over(page: u32, color: u32) -> u32 {
    let k = f32::from(tokens::alpha(color)) / 255.0;
    tokens::lerp(page, color | 0xff00_0000, k)
}

#[test]
fn danger_buttons_paint_the_error_colours_per_state_in_both_themes() {
    let _guard = MOTION.lock().unwrap_or_else(|e| e.into_inner());
    motion::set_enabled(false);
    for theme in [ant_light(), ant_dark()] {
        let (mut runtime, mut renderer) = runtime(theme, false);
        frame(&mut runtime, &mut renderer);
        let primary = runtime.tree.bounds_of_key("primary").expect("primary");
        // a point of the fill left of the label
        let (px, py) = ((primary.x + 4.0) as usize, (primary.y + 6.0) as usize);
        let pixel = renderer.pixel_at(px, py).expect("pixel");
        assert!(close(pixel, theme.color_error), "rest {pixel:08x}");
        runtime.dispatch(Event::PointerMove(point(primary.x + 4.0, primary.y + 6.0)));
        frame(&mut runtime, &mut renderer);
        let pixel = renderer.pixel_at(px, py).expect("pixel");
        assert!(close(pixel, theme.color_error_hover), "hover {pixel:08x}");
        runtime.dispatch(Event::PointerDown(point(primary.x + 4.0, primary.y + 6.0)));
        frame(&mut runtime, &mut renderer);
        let pixel = renderer.pixel_at(px, py).expect("pixel");
        assert!(close(pixel, theme.color_error_active), "active {pixel:08x}");
        runtime.dispatch(Event::PointerUp(point(primary.x + 4.0, primary.y + 6.0)));

        // default danger: a red 1px border on the container background
        let default = runtime.tree.bounds_of_key("default").expect("default");
        frame(&mut runtime, &mut renderer);
        let edge = renderer
            .pixel_at(
                default.x as usize,
                (default.y + default.height * 0.5) as usize,
            )
            .expect("pixel");
        assert!(close(edge, theme.color_error), "border {edge:08x}");
        let inside = renderer
            .pixel_at((default.x + 4.0) as usize, (default.y + 6.0) as usize)
            .expect("pixel");
        assert!(close(inside, theme.color_bg_container), "fill {inside:08x}");
    }
    motion::set_enabled(true);
}

#[test]
fn a_block_button_is_as_wide_as_its_parent() {
    let _guard = MOTION.lock().unwrap_or_else(|e| e.into_inner());
    motion::set_enabled(false);
    let (mut runtime, mut renderer) = runtime(ant_light(), false);
    frame(&mut runtime, &mut renderer);
    let block = runtime.tree.bounds_of_key("block").expect("block");
    let primary = runtime.tree.bounds_of_key("primary").expect("primary");
    // the column's content box: 400px minus 8px padding on each side
    assert_eq!(block.width, W - 16.0);
    assert_eq!(block.height, 32.0);
    assert!(primary.width < 100.0, "the others keep their own width");
    // antd icon-only buttons are square
    let only = runtime.tree.bounds_of_key("only").expect("only");
    assert_eq!((only.width, only.height), (32.0, 32.0));
    motion::set_enabled(true);
}

#[test]
fn a_loading_button_ignores_clicks_and_keys_and_looks_loading() {
    let _guard = MOTION.lock().unwrap_or_else(|e| e.into_inner());
    motion::set_enabled(false);
    let theme = ant_light();
    let (mut runtime, mut renderer) = runtime(theme, true);
    frame(&mut runtime, &mut renderer);
    let id = runtime.find_key("load").expect("load");
    let bounds = runtime.tree.bounds_of_key("load").expect("bounds");
    let center = point(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    );
    runtime.dispatch(Event::PointerMove(center));
    runtime.dispatch(Event::PointerDown(center));
    runtime.dispatch(Event::PointerUp(center));
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    assert_eq!(runtime.app_state_mut::<Panel>().clicks, 0);
    match &runtime.tree.get(id).data {
        NodeData::Button(data) => {
            assert!(data.loading);
            assert_eq!(data.wave_t, 1.0, "no click wave while loading");
        }
        _ => panic!("expected a button"),
    }
    // antd opacityLoading: the primary fill at 65 % over the page
    runtime.dispatch(Event::PointerMove(point(1.0, H - 1.0)));
    frame(&mut runtime, &mut renderer);
    let pixel = renderer
        .pixel_at((bounds.x + 3.0) as usize, (bounds.y + 6.0) as usize)
        .expect("pixel");
    let page = renderer
        .pixel_at((bounds.x - 4.0) as usize, (bounds.y + 6.0) as usize)
        .expect("page pixel");
    let expected = over(page, tokens::with_alpha(theme.color_primary, 166));
    assert!(close(pixel, expected), "{pixel:08x} vs {expected:08x}");
    // the spinner widens the button by its slot and gap (16 + 8)
    runtime.app_state_mut::<Panel>().loading = false;
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer);
    let idle = runtime.tree.bounds_of_key("load").expect("bounds");
    assert!((bounds.width - idle.width - 24.0).abs() <= 0.5);
    // done loading: clicks work again
    let center = point(idle.x + idle.width * 0.5, idle.y + idle.height * 0.5);
    runtime.dispatch(Event::PointerMove(center));
    runtime.dispatch(Event::PointerDown(center));
    runtime.dispatch(Event::PointerUp(center));
    assert_eq!(runtime.app_state_mut::<Panel>().clicks, 1);
    motion::set_enabled(true);
}

static NOW: AtomicI64 = AtomicI64::new(1_000);

fn clock() -> i64 {
    NOW.load(Ordering::Relaxed)
}

fn spin_t(runtime: &Runtime) -> f32 {
    let id = runtime.find_key("load").expect("load");
    match &runtime.tree.get(id).data {
        NodeData::Button(data) => data.spin_t,
        _ => panic!("expected a button"),
    }
}

#[test]
fn the_loading_spinner_turns_without_rebuilding() {
    let _guard = MOTION.lock().unwrap_or_else(|e| e.into_inner());
    motion::set_enabled(true);
    NOW.store(1_000, Ordering::Relaxed);
    let (mut runtime, mut renderer) = runtime(ant_light(), true);
    runtime.clock = clock;
    frame(&mut runtime, &mut renderer);
    assert!(runtime.any_animation, "a mounted loading button animates");
    assert_eq!(runtime.next_delay_ms, 16);
    NOW.store(1_250, Ordering::Relaxed);
    frame(&mut runtime, &mut renderer);
    assert!(!runtime.needs_build, "spinner frames never rebuild");
    assert!((spin_t(&runtime) - 0.25).abs() < 0.01);
    NOW.store(1_500, Ordering::Relaxed);
    frame(&mut runtime, &mut renderer);
    assert!((spin_t(&runtime) - 0.5).abs() < 0.01);
    assert!(runtime.any_animation, "it keeps turning while loading");
    // loading ends: the frames stop
    runtime.app_state_mut::<Panel>().loading = false;
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer);
    NOW.store(2_000, Ordering::Relaxed);
    frame(&mut runtime, &mut renderer);
    assert!(!runtime.any_animation);
    assert_eq!(runtime.next_delay_ms, -1);

    // reduced motion: a still indicator and no frames
    motion::set_enabled(false);
    runtime.app_state_mut::<Panel>().loading = true;
    runtime.needs_build = true;
    frame(&mut runtime, &mut renderer);
    NOW.store(2_300, Ordering::Relaxed);
    frame(&mut runtime, &mut renderer);
    assert_eq!(spin_t(&runtime), 0.0);
    assert!(!runtime.any_animation);
    motion::set_enabled(true);
}
