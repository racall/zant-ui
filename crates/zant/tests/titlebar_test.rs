//! Title bar hit test of app-drawn window chrome.

use zant::platform::platform::hit;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui;
use zant::ui::app::{App, RunOptions};

struct Chrome;

impl App for Chrome {
    fn render(&mut self) -> ui::Node {
        ui::column(ui::BoxOptions {
            width: Some(400.0),
            height: Some(200.0),
            children: vec![
                ui::window_controls(ui::WindowControlsOptions {
                    height: 32.0,
                    maximized: false,
                    active: true,
                    on_minimize: None,
                    on_maximize: None,
                    on_close: None,
                }),
                ui::button(ui::ButtonOptions {
                    key: Some("ok".into()),
                    text: "OK".into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn titlebar_hit_distinguishes_caption_widgets_and_the_maximize_button() {
    let mut runtime = Runtime::init(Chrome, RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 200).expect("raster renderer");
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, 400.0, 200.0, 1.0);
    renderer.end();

    let center = |runtime: &Runtime, key: &str| {
        let id = runtime
            .find_key(key)
            .unwrap_or_else(|| panic!("node {key}"));
        let bounds = runtime.tree.get(id).bounds;
        (
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        )
    };
    // the maximize button reports itself, so Windows 11 offers snap layouts
    let (x, y) = center(&runtime, ui::CAPTION_MAXIMIZE_KEY);
    assert_eq!(runtime.titlebar_hit(x, y), hit::MAXIMIZE);
    // any other widget keeps its clicks
    let (x, y) = center(&runtime, "ok");
    assert_eq!(runtime.titlebar_hit(x, y), hit::CLIENT);
    // empty space drags the window
    assert_eq!(runtime.titlebar_hit(200.0, 190.0), hit::CAPTION);
}
