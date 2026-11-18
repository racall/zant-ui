use super::*;
use crate::core::node::NodeKind;
use crate::renderer::renderer::Renderer;
use crate::theme::tokens::ant_light;
use crate::ui::actions::action;
use crate::ui::app::App;
use crate::ui::builders::{button, column, text};
use crate::ui::options::{BoxOptions, ButtonOptions, TextOptions};
use crate::ui::{Node, RunOptions};

/// A small declarative app: white page, a heading and a primary button.
pub(crate) struct Demo {
    pub clicks: u32,
}

impl App for Demo {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            padding: crate::ui::inset(8.0),
            gap: 6.0,
            background: Some(0xffff_ffff),
            children: crate::nodes![
                text(
                    "Hello Zant",
                    TextOptions {
                        size: 20.0,
                        color: Some(0xff00_0000),
                        ..Default::default()
                    }
                ),
                button(ButtonOptions {
                    key: Some("go".into()),
                    text: "Go".into(),
                    variant: crate::core::node::ButtonType::Primary,
                    on_click: Some(action(|state: &mut Demo| state.clicks += 1)),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

/// One frame of `Demo` into a 200×120 raster, as the headless smoke test.
fn smoke_frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("a canvas");
    runtime.render_frame(canvas, 200.0, 120.0, 1.0);
    renderer.end();
}

#[test]
fn headless_frame_builds_lays_out_and_paints_pixels() {
    let mut runtime = Runtime::init(Demo { clicks: 0 }, RunOptions::default());
    let mut renderer = Renderer::create_raster(200, 120).expect("raster renderer");
    smoke_frame(&mut runtime, &mut renderer);

    // the app's column becomes the window root; one text, one button
    assert_eq!(runtime.tree.live_ids().len(), 3);
    assert!(runtime.tree.id_of_key("go").is_some());
    assert!(!runtime.needs_build, "the frame rebuilt the tree");
    let button_id = runtime.tree.id_of_key("go").expect("button");
    assert_eq!(runtime.tree.get(button_id).kind(), NodeKind::Button);
    let bounds = runtime.tree.get(button_id).bounds;
    assert!(bounds.width > 0.0 && bounds.height > 0.0);

    // the raster was blank before: the frame paints the white page and the
    // primary button (a different colour)
    let page = renderer.pixel_at(2, 2).expect("pixel");
    assert_eq!(page & 0x00ff_ffff, 0x00ff_ffff, "white background");
    let mut distinct = std::collections::HashSet::new();
    for y in 0..120 {
        for x in 0..200 {
            if let Some(pixel) = renderer.pixel_at(x, y) {
                distinct.insert(pixel);
            }
        }
    }
    assert!(
        distinct.len() >= 3,
        "background, text and button colours, got {distinct:?}"
    );
    // antd primary blue is painted somewhere inside the button box
    let primary = ant_light().color_primary;
    let mut saw_primary = false;
    for y in (bounds.y as usize)..((bounds.bottom() as usize).min(120)) {
        for x in (bounds.x as usize)..((bounds.right() as usize).min(200)) {
            if renderer
                .pixel_at(x, y)
                .is_some_and(|pixel| (pixel & 0x00ff_ffff) == (primary & 0x00ff_ffff))
            {
                saw_primary = true;
            }
        }
    }
    assert!(saw_primary, "the button painted its primary background");
}

#[test]
fn a_second_frame_without_changes_paints_again_without_rebuilding() {
    let mut runtime = Runtime::init(Demo { clicks: 0 }, RunOptions::default());
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    smoke_frame(&mut runtime, &mut renderer);
    runtime.tree.clear_paint();
    let revision = runtime.tree.live_ids();
    smoke_frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.tree.live_ids(), revision, "no rebuild happened");
    assert!(runtime.now_ms >= 0);
}

#[test]
fn next_frame_delay_is_unset_for_a_static_ui() {
    let mut runtime = Runtime::init(Demo { clicks: 0 }, RunOptions::default());
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    smoke_frame(&mut runtime, &mut renderer);
    assert_eq!(
        runtime.next_delay_ms, -1,
        "a static window does not poll for frames"
    );
}

#[test]
fn wake_after_schedules_a_rebuild_frame() {
    use std::sync::atomic::{AtomicI64, Ordering};
    static NOW: AtomicI64 = AtomicI64::new(1_000);
    fn clock() -> i64 {
        NOW.load(Ordering::Relaxed)
    }
    struct Waker;
    impl App for Waker {
        fn render(&mut self) -> Node {
            crate::ui::wake_after(50);
            column(BoxOptions::default())
        }
    }
    NOW.store(1_000, Ordering::Relaxed);
    let mut runtime = Runtime::init(Waker, RunOptions::default());
    runtime.clock = clock;
    let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
    smoke_frame(&mut runtime, &mut renderer);
    assert_eq!(runtime.wake_at_ms, Some(1_050));
    assert_eq!(runtime.next_delay_ms, 50);
    NOW.store(1_050, Ordering::Relaxed);
    smoke_frame(&mut runtime, &mut renderer);
    // the next build queues a new 50 ms wake-up
    assert_eq!(runtime.wake_at_ms, Some(1_100));
    assert!(!runtime.needs_build);
    assert_eq!(runtime.next_delay_ms, 50);
}
