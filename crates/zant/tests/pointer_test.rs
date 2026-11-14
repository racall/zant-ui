//! Port of `tests/pointer_test.zig` — in-memory images shown by `ui::image`
//! and the container hover look.
//!
//! Two of the three Zig cases are not portable yet (listed in the WP-2.10
//! report): the `ui::pointer_area` case asserts that `ui::key_bounds` answers
//! inside `BoxOptions::on_resize`, but the Rust runtime clears the key-bounds
//! hook at the end of the build, before `notify_resizes` runs.

use zant::core::events::Event;
use zant::core::node::{NodeData, NodeId};
use zant::image::Image;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{BoxOptions, ImageOptions};
use zant::ui::{button, column, image, row};

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, w: f32, h: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, w, h, 1.0);
    renderer.end();
}

#[test]
fn image_in_memory_pixels_shown_by_ui_image() {
    let pixels = [0xffu8; 4 * 4 * 4];
    let img = Image::from_pixels(&pixels, 4, 4, 16).expect("image");
    assert_eq!(img.width, 4);
    let copy = img.retain();
    drop(copy);

    struct State {
        img: Image,
    }

    impl App for State {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: zant::nodes![image(ImageOptions {
                    key: Some("img".into()),
                    image: Some(std::rc::Rc::new(self.img.retain())),
                    ..Default::default()
                })],
                ..Default::default()
            })
        }
    }

    let mut runtime = Runtime::init(
        State { img },
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 100.0,
                height: 100.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    runtime.rebuild_for_test();
    let mut found = false;
    for node in &runtime.tree.nodes {
        if !node.alive {
            continue;
        }
        if node.key.as_deref() == Some("img") {
            found = true;
            let NodeData::Image(data) = &node.data else {
                panic!("expected an image node");
            };
            assert_eq!(data.intrinsic_width, 4);
        }
    }
    assert!(found);
}

struct HoverApp {
    builds: usize,
    modal: bool,
}

impl App for HoverApp {
    fn render(&mut self) -> Node {
        self.builds += 1;
        let rows = column(BoxOptions {
            key: Some("rows".into()),
            align_items: zant::layout::flex::Align::Stretch,
            children: zant::nodes![
                row(BoxOptions {
                    key: Some("row-a".into()),
                    height: Some(40.0),
                    hover_background: Some(0xff112233),
                    children: zant::nodes![button(zant::ui::ButtonOptions {
                        key: Some("btn-a".into()),
                        text: "A".into(),
                        ..Default::default()
                    })],
                    ..Default::default()
                }),
                row(BoxOptions {
                    key: Some("row-b".into()),
                    height: Some(40.0),
                    hover_background: Some(0xff445566),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });
        if !self.modal {
            return column(BoxOptions {
                grow: 1.0,
                align_items: zant::layout::flex::Align::Stretch,
                children: zant::nodes![rows],
                ..Default::default()
            });
        }
        column(BoxOptions {
            grow: 1.0,
            align_items: zant::layout::flex::Align::Stretch,
            children: zant::nodes![rows, zant::ui::modal(Default::default())],
            ..Default::default()
        })
    }
}

fn container_hovered(tree: &zant::core::tree::Tree, id: NodeId) -> bool {
    let NodeData::Container(data) = &tree.get(id).data else {
        panic!("expected a container node");
    };
    data.hovered
}

#[test]
fn container_hover_look_no_rebuild_follows_the_pointer_hidden_under_a_modal() {
    zant::motion::motion::set_enabled(false);
    let mut runtime = Runtime::init(
        HoverApp {
            builds: 0,
            modal: false,
        },
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 300.0,
                height: 300.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(300, 300).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 300.0, 300.0);
    let builds = runtime.app_state_mut::<HoverApp>().builds;
    let row_b_bounds = runtime.tree.bounds_of_key("row-b").expect("row-b");
    let row_b = runtime.tree.id_of_key("row-b").expect("row-b");
    runtime.dispatch(Event::PointerMove(zant::core::events::PointerEvent {
        x: 200.0,
        y: row_b_bounds.y + 20.0,
        ..Default::default()
    }));
    frame(&mut runtime, &mut renderer, 300.0, 300.0);
    assert_eq!(builds, runtime.app_state_mut::<HoverApp>().builds);
    assert!(container_hovered(&runtime.tree, row_b));
    // over the button in row A: row A shows its hover (ancestor of the widget)
    let button_bounds = runtime.tree.bounds_of_key("btn-a").expect("btn-a");
    runtime.dispatch(Event::PointerMove(zant::core::events::PointerEvent {
        x: button_bounds.x + 5.0,
        y: button_bounds.y + 5.0,
        ..Default::default()
    }));
    frame(&mut runtime, &mut renderer, 300.0, 300.0);
    assert!(!container_hovered(&runtime.tree, row_b));
    let row_a = runtime.tree.id_of_key("row-a").expect("row-a");
    assert!(container_hovered(&runtime.tree, row_a));
    // a modal over the rows hides their hover
    runtime.app_state_mut::<HoverApp>().modal = true;
    runtime.invalidate();
    frame(&mut runtime, &mut renderer, 300.0, 300.0);
    runtime.dispatch(Event::PointerMove(zant::core::events::PointerEvent {
        x: 200.0,
        y: row_b_bounds.y + 20.0,
        ..Default::default()
    }));
    frame(&mut runtime, &mut renderer, 300.0, 300.0);
    assert!(!container_hovered(&runtime.tree, row_b));
    zant::motion::motion::set_enabled(true);
}
