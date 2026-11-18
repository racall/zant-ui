//! `ui::pointer_area` (capture, local
//! coordinates, multi-click, wheel capture, keys; `BoxOptions::on_resize`),
//! in-memory images shown by `ui::image` and the container hover look.

use std::cell::Cell;

use zant::core::events::{Event, Key, KeyEvent, Modifiers, PointerEvent, Scroll};
use zant::core::node::{NodeData, NodeId};
use zant::image::Image;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::actions::{KeyAction, PointerAction, PointerInfo, PointerPhase};
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{BoxOptions, ImageOptions, PointerAreaOptions};
use zant::ui::{Cursor, button, column, image, inset, pointer_area, row};

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, w: f32, h: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, w, h, 1.0);
    renderer.end();
}

thread_local! {
    static NOW: Cell<i64> = const { Cell::new(1000) };
}

fn fake_clock() -> i64 {
    NOW.with(|now| now.get())
}

struct Log {
    phases: [PointerPhase; 32],
    infos: [PointerInfo; 32],
    n: usize,
    keys: usize,
    resizes: usize,
    wide: bool,
}

impl Log {
    fn new() -> Log {
        Log {
            phases: [PointerPhase::default(); 32],
            infos: [PointerInfo::default(); 32],
            n: 0,
            keys: 0,
            resizes: 0,
            wide: false,
        }
    }

    fn record(&mut self, info: PointerInfo) -> bool {
        if self.n < self.phases.len() {
            self.phases[self.n] = info.phase;
            self.infos[self.n] = info;
            self.n += 1;
        }
        info.phase != PointerPhase::Hover
    }

    fn last(&self) -> PointerInfo {
        self.infos[self.n - 1]
    }
}

impl App for Log {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            grow: 1.0,
            padding: inset(20.0),
            children: zant::nodes![pointer_area(PointerAreaOptions {
                r#box: BoxOptions {
                    key: Some("area".into()),
                    width_percent: Some(100.0),
                    height_percent: Some(100.0),
                    on_resize: Some(zant::ui::action(|state: &mut Log| {
                        state.resizes += 1;
                        if let Some(bounds) = zant::ui::key_bounds("area") {
                            state.wide = bounds.width > 300.0;
                        }
                    })),
                    ..Default::default()
                },
                on_pointer: PointerAction::new(|cx, info| cx.state::<Log>().record(info)),
                capture_scroll: true,
                cursor: Some(Cursor::Crosshair),
                focusable: true,
                on_key: Some(KeyAction::new(|cx, _key| {
                    cx.state::<Log>().keys += 1;
                    true
                })),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn pointer_area_capture_local_coordinates_clicks_wheel_and_keys() {
    let log = Log::new();
    let mut runtime = Runtime::init(
        log,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 400.0,
                height: 300.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");
    NOW.with(|now| now.set(1000));
    runtime.clock = fake_clock;
    frame(&mut runtime, &mut renderer, 400.0, 300.0);
    assert_eq!(runtime.app_state_mut::<Log>().resizes, 1);
    assert!(runtime.app_state_mut::<Log>().wide);

    // hover, then press inside and drag outside: the area keeps the pointer
    runtime.dispatch(Event::PointerMove(PointerEvent {
        x: 30.0,
        y: 40.0,
        ..Default::default()
    }));
    {
        let log = &*runtime.app_state_mut::<Log>();
        assert_eq!(log.last().phase, PointerPhase::Hover);
        assert!((log.last().x - 10.0).abs() <= 0.01);
        assert!((log.last().y - 20.0).abs() <= 0.01);
    }
    runtime.dispatch(Event::PointerDown(PointerEvent {
        x: 30.0,
        y: 40.0,
        ..Default::default()
    }));
    {
        let log = &*runtime.app_state_mut::<Log>();
        assert_eq!(log.last().phase, PointerPhase::Down);
        assert_eq!(log.last().click_count, 1);
    }
    runtime.dispatch(Event::PointerMove(PointerEvent {
        x: 390.0,
        y: 290.0,
        ..Default::default()
    }));
    {
        let log = &*runtime.app_state_mut::<Log>();
        assert_eq!(log.last().phase, PointerPhase::Drag);
        assert!((log.last().x - 370.0).abs() <= 0.01);
        assert!((log.last().y - 270.0).abs() <= 0.01);
    }
    runtime.dispatch(Event::PointerUp(PointerEvent {
        x: 390.0,
        y: 290.0,
        ..Default::default()
    }));
    {
        let log = &*runtime.app_state_mut::<Log>();
        assert_eq!(log.last().phase, PointerPhase::Up);
    }
    // focused by the click: keys go to on_key
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    assert_eq!(runtime.app_state_mut::<Log>().keys, 1);

    // double click (same spot, quickly)
    NOW.with(|now| now.set(now.get() + 1000));
    runtime.dispatch(Event::PointerDown(PointerEvent {
        x: 50.0,
        y: 50.0,
        ..Default::default()
    }));
    runtime.dispatch(Event::PointerUp(PointerEvent {
        x: 50.0,
        y: 50.0,
        ..Default::default()
    }));
    NOW.with(|now| now.set(now.get() + 120));
    runtime.dispatch(Event::PointerDown(PointerEvent {
        x: 51.0,
        y: 50.0,
        ..Default::default()
    }));
    assert_eq!(runtime.app_state_mut::<Log>().last().click_count, 2);
    runtime.dispatch(Event::PointerUp(PointerEvent {
        x: 51.0,
        y: 50.0,
        ..Default::default()
    }));
    NOW.with(|now| now.set(now.get() + 800));
    runtime.dispatch(Event::PointerDown(PointerEvent {
        x: 51.0,
        y: 50.0,
        ..Default::default()
    }));
    assert_eq!(runtime.app_state_mut::<Log>().last().click_count, 1);
    runtime.dispatch(Event::PointerUp(PointerEvent {
        x: 51.0,
        y: 50.0,
        ..Default::default()
    }));

    // the wheel goes to the area
    runtime.dispatch(Event::Scroll(Scroll {
        x: 60.0,
        y: 60.0,
        dx: 0.0,
        dy: -12.0,
        modifiers: Modifiers::default(),
    }));
    {
        let log = &*runtime.app_state_mut::<Log>();
        assert_eq!(log.last().phase, PointerPhase::Scroll);
        assert!((log.last().dy + 12.0).abs() <= 0.01);
    }

    // window resize: on_resize runs and the UI rebuilds in the same frame
    frame(&mut runtime, &mut renderer, 200.0, 300.0);
    assert_eq!(runtime.app_state_mut::<Log>().resizes, 2);
    assert!(!runtime.app_state_mut::<Log>().wide);
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
            align_items: zant::flex::Align::Stretch,
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
                align_items: zant::flex::Align::Stretch,
                children: zant::nodes![rows],
                ..Default::default()
            });
        }
        column(BoxOptions {
            grow: 1.0,
            align_items: zant::flex::Align::Stretch,
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
