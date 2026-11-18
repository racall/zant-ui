//! `ui::view`: RN `View` / HTML `div`
//! defaults, box-option passthrough, shadow, style override, layout and clicks.

use zant::components::view::{ViewOptions, view};
use zant::core::events::{Event, PointerEvent};
use zant::core::geometry::Rect;
use zant::core::node::{NodeData, NodeKind};
use zant::flex;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::actions::{action, index_action};
use zant::ui::app::{App, RunOptions};
use zant::ui::builders::{button, text};
use zant::ui::element::Node;
use zant::ui::options::{ButtonOptions, TextOptions};
use zant::ui::{Style, inset, inset_xy, with_key};

fn render_frame(runtime: &mut Runtime, renderer: &mut Renderer, width: f32, height: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, width, height, 1.0);
    renderer.end();
}

fn click_at(runtime: &mut Runtime, rect: Rect) {
    let point = PointerEvent {
        x: rect.x + rect.width * 0.5,
        y: rect.y + rect.height * 0.5,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerMove(point));
    runtime.dispatch(Event::PointerDown(point));
    runtime.dispatch(Event::PointerUp(point));
}

fn window_options(width: f32, height: f32) -> RunOptions {
    RunOptions {
        window: zant::platform::platform::WindowOptions {
            width,
            height,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn view_defaults_to_a_stretching_unshrinking_column_container() {
    let node = view(ViewOptions::default());
    let e = node.element();
    assert_eq!(e.kind, NodeKind::Container);
    assert_eq!(e.layout.direction, flex::Direction::Column);
    assert_eq!(e.layout.align_items, flex::Align::Stretch);
    assert_eq!(e.layout.shrink, 0.0);
    assert!(e.pointer.is_none());
    assert!(e.visual.shadow_color.is_none());
}

#[test]
fn view_direction_and_the_flex_direction_alias() {
    assert_eq!(
        view(ViewOptions {
            direction: flex::Direction::Row,
            ..Default::default()
        })
        .element()
        .layout
        .direction,
        flex::Direction::Row
    );
    assert_eq!(
        view(ViewOptions {
            flex_direction: Some(flex::Direction::Row),
            ..Default::default()
        })
        .element()
        .layout
        .direction,
        flex::Direction::Row
    );
    assert_eq!(
        view(ViewOptions {
            direction: flex::Direction::Row,
            flex_direction: Some(flex::Direction::Column),
            ..Default::default()
        })
        .element()
        .layout
        .direction,
        flex::Direction::Column
    );
}

#[test]
fn view_passes_every_box_option_through() {
    let child = text("x", TextOptions::default());
    let node = view(ViewOptions {
        key: Some("card".into()),
        gap: 6.0,
        padding: inset(12.0),
        margin: inset_xy(4.0, 2.0),
        justify_content: Some(flex::Justify::Center),
        align_items: flex::Align::Center,
        width: Some(120.0),
        height_percent: Some(50.0),
        grow: 1.0,
        shrink: 1.0,
        background: Some(0xff112233),
        border_color: Some(0xff445566),
        border_width: 1.0,
        radius: 8.0,
        scroll: true,
        hover_background: Some(0x0f000000),
        children: vec![child],
        ..Default::default()
    });
    let e = node.element();
    assert_eq!(e.key.as_deref(), Some("card"));
    assert_eq!(e.layout.gap, 6.0);
    assert_eq!(e.layout.padding.top, 12.0);
    assert_eq!(e.layout.margin.left, 4.0);
    assert_eq!(e.layout.margin.top, 2.0);
    assert_eq!(e.layout.justify, flex::Justify::Center);
    assert_eq!(e.layout.align_items, flex::Align::Center);
    assert_eq!(e.layout.width, Some(120.0));
    assert_eq!(e.layout.height_percent, Some(50.0));
    assert_eq!(e.layout.grow, 1.0);
    assert_eq!(e.layout.shrink, 1.0);
    assert_eq!(e.visual.background, Some(0xff112233));
    assert_eq!(e.visual.border_color, Some(0xff445566));
    assert_eq!(e.visual.border_width, 1.0);
    assert_eq!(e.visual.radius, 8.0);
    assert!(e.layout.scroll);
    assert!(e.visual.clip); // scrolling clips
    assert_eq!(
        e.hover_visual.as_ref().expect("hover visual").background,
        Some(0x0f000000)
    );
    assert_eq!(e.children.len(), 1);
}

#[test]
fn view_shadow_and_style_override() {
    let shadowed = view(ViewOptions {
        shadow_color: Some(0x33000000),
        shadow_blur: 12.0,
        shadow_offset_y: 4.0,
        ..Default::default()
    });
    assert_eq!(shadowed.element().visual.shadow_color, Some(0x33000000));
    assert_eq!(shadowed.element().visual.shadow_blur, 12.0);
    assert_eq!(shadowed.element().visual.shadow_offset_y, 4.0);

    // style wins over the plain options, including the shadow
    let node = view(ViewOptions {
        background: Some(0xff000000),
        width: Some(10.0),
        shadow_color: Some(0x33000000),
        shadow_blur: 12.0,
        style: Some(Style {
            background: Some(0xffffffff),
            width: Some(40.0),
            shadow_blur: Some(2.0),
            radius: Some(6.0),
            ..Default::default()
        }),
        ..Default::default()
    });
    assert_eq!(node.element().visual.background, Some(0xffffffff));
    assert_eq!(node.element().layout.width, Some(40.0));
    assert_eq!(node.element().visual.radius, 6.0);
    assert_eq!(node.element().visual.shadow_color, Some(0x33000000));
    assert_eq!(node.element().visual.shadow_blur, 2.0);
}

struct Layout;

impl App for Layout {
    fn render(&mut self) -> Node {
        view(ViewOptions {
            key: Some("root".into()),
            padding: inset(10.0),
            gap: 8.0,
            children: zant::nodes![
                view(ViewOptions {
                    key: Some("block".into()),
                    height: Some(30.0),
                    ..Default::default()
                }),
                view(ViewOptions {
                    key: Some("bar".into()),
                    direction: flex::Direction::Row,
                    gap: 4.0,
                    children: zant::nodes![
                        view(ViewOptions {
                            key: Some("a".into()),
                            width: Some(50.0),
                            height: Some(20.0),
                            ..Default::default()
                        }),
                        view(ViewOptions {
                            key: Some("b".into()),
                            grow: 1.0,
                            height: Some(20.0),
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn view_lays_children_out_like_a_div_and_a_row_view_side_by_side() {
    let mut runtime = Runtime::init(Layout, window_options(300.0, 200.0));
    let mut renderer = Renderer::create_raster(300, 200).expect("raster renderer");
    render_frame(&mut runtime, &mut renderer, 300.0, 200.0);

    let block = runtime.tree.bounds_of_key("block").expect("block");
    assert_eq!(block.x, 10.0);
    assert_eq!(block.y, 10.0);
    assert_eq!(block.width, 280.0); // stretched across the padding box
    assert_eq!(block.height, 30.0);

    let bar = runtime.tree.bounds_of_key("bar").expect("bar");
    assert_eq!(bar.y, 48.0);
    assert_eq!(bar.width, 280.0);
    assert_eq!(bar.height, 20.0);

    let a = runtime.tree.bounds_of_key("a").expect("a");
    let b = runtime.tree.bounds_of_key("b").expect("b");
    assert_eq!(a.x, 10.0);
    assert_eq!(a.width, 50.0);
    assert_eq!(b.x, 64.0);
    assert_eq!(b.width, 226.0); // grows into the rest of the row
}

struct Clicks {
    opened: u32,
    picked: Option<usize>,
    saved: u32,
}

impl Clicks {
    fn new() -> Clicks {
        Clicks {
            opened: 0,
            picked: None,
            saved: 0,
        }
    }
}

impl App for Clicks {
    fn render(&mut self) -> Node {
        view(ViewOptions {
            children: zant::nodes![
                view(ViewOptions {
                    key: Some("card".into()),
                    height: Some(60.0),
                    direction: flex::Direction::Row,
                    on_click: Some(action(|state: &mut Clicks| state.opened += 1)),
                    children: zant::nodes![
                        view(ViewOptions {
                            grow: 1.0,
                            ..Default::default()
                        }),
                        button(ButtonOptions {
                            key: Some("save".into()),
                            text: "Save".into(),
                            on_click: Some(action(|state: &mut Clicks| state.saved += 1)),
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                }),
                view(ViewOptions {
                    key: Some("row-3".into()),
                    height: Some(40.0),
                    index_action: Some(index_action(|state: &mut Clicks, i: usize| {
                        state.picked = Some(i)
                    })),
                    index: 3,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn clickable_view_on_click_index_action_and_inner_buttons_keep_their_clicks() {
    zant::motion::motion::set_enabled(false);
    let mut runtime = Runtime::init(Clicks::new(), window_options(300.0, 200.0));
    let mut renderer = Renderer::create_raster(300, 200).expect("raster renderer");

    render_frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let card = runtime.tree.bounds_of_key("card").expect("card");
    click_at(
        &mut runtime,
        Rect {
            x: card.x,
            y: card.y,
            width: 40.0,
            height: card.height,
        },
    );
    assert_eq!(runtime.app_state_mut::<Clicks>().opened, 1);

    render_frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let save = runtime.tree.bounds_of_key("save").expect("save");
    click_at(&mut runtime, save);
    assert_eq!(runtime.app_state_mut::<Clicks>().saved, 1);
    assert_eq!(runtime.app_state_mut::<Clicks>().opened, 1);

    render_frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let row = runtime.tree.bounds_of_key("row-3").expect("row-3");
    click_at(&mut runtime, row);
    assert_eq!(runtime.app_state_mut::<Clicks>().picked, Some(3));
    zant::motion::motion::set_enabled(true);
}

struct Wrap;

impl App for Wrap {
    fn render(&mut self) -> Node {
        view(ViewOptions {
            padding: inset(24.0),
            gap: 16.0,
            align_items: flex::Align::Start,
            children: zant::nodes![
                text(
                    "Terms",
                    TextOptions {
                        key: Some("word".into()),
                        size: 14.0,
                        weight: zant::renderer::text::Weight::Medium,
                        wrap: true,
                        ..Default::default()
                    },
                ),
                with_key(
                    zant::ui::paragraph(
                        "Every component on this page is written in TSX and compiled to native Rust.",
                        Default::default()
                    ),
                    "para"
                ),
                view(ViewOptions {
                    width: Some(420.0),
                    padding: inset(20.0),
                    align_items: flex::Align::Start,
                    children: zant::nodes![with_key(
                        zant::ui::alert(zant::ui::AlertOptions {
                            title: "Terms".into(),
                            description: "State flows from TSX into native Zant components.".into(),
                            ..Default::default()
                        }),
                        "alert"
                    )],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn wrapped_text_at_its_natural_width_stays_on_one_line_start_aligned_columns() {
    let mut runtime = Runtime::init(Wrap, window_options(960.0, 700.0));
    let mut renderer = Renderer::create_raster(960, 700).expect("raster renderer");
    render_frame(&mut runtime, &mut renderer, 960.0, 700.0);
    // one line each: a wrapped "Term / s" or a dropped last word doubles the height
    for n in &runtime.tree.nodes {
        if !n.alive {
            continue;
        }
        let NodeData::Text(data) = &n.data else {
            continue;
        };
        let one_line = data.style.font_size * 1.6;
        assert!(
            n.bounds.height <= one_line,
            "text {:?} wrapped: {}x{}",
            data.content,
            n.bounds.width,
            n.bounds.height
        );
    }
}
