//! Icons, dropdown menus, segmented controls and dismiss layers.

use super::*;

#[test]
fn icons_build_for_every_name() {
    for name in Icon::ALL {
        let node = icon(IconOptions {
            name,
            size: 18.0,
            ..Default::default()
        });
        let drawing = node.element().drawing.as_ref().expect("icon drawing");
        assert_eq!(drawing.width, 18.0);
    }
}

pub(super) struct Clicks {
    pub(super) clicks: usize,
}

impl App for Clicks {
    fn render(&mut self) -> Node {
        column(BoxOptions::default())
    }
}

#[test]
fn dropdown_menu_segmented_and_dismiss_layer() {
    let _state = Clicks { clicks: 0 };
    let menu = dropdown_menu(MenuOptions {
        items: vec![
            MenuItem {
                label: "New".into(),
                icon: Icon::File,
                extra: "⌘N".into(),
                on_click: Some(action(|state: &mut Clicks| state.clicks += 1)),
                ..Default::default()
            },
            MenuItem::DIVIDER,
            MenuItem::group("View"),
            MenuItem {
                label: "Disabled".into(),
                disabled: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    assert_eq!(menu.element().children.len(), 4);
    let seg = segmented(SegmentedOptions {
        options: vec![
            SegmentedOption {
                label: "A".into(),
                ..Default::default()
            },
            SegmentedOption {
                label: "B".into(),
                ..Default::default()
            },
        ],
        selected: 1,
        ..Default::default()
    });
    // inline (antd default): a wrapper row around the track; the track holds
    // the sliding thumb + 2 options
    assert_eq!(seg.element().children.len(), 1);
    assert_eq!(seg.element().children[0].element().children.len(), 3);
    let block = segmented(SegmentedOptions {
        options: vec![
            SegmentedOption {
                label: "A".into(),
                ..Default::default()
            },
            SegmentedOption {
                label: "B".into(),
                ..Default::default()
            },
        ],
        block: true,
        ..Default::default()
    });
    assert_eq!(block.element().children.len(), 3);
    let layer = dismiss_layer(DismissLayerOptions {
        on_dismiss: Some(action(|state: &mut Clicks| state.clicks += 1)),
        ..Default::default()
    });
    assert!(layer.element().layout.absolute);
    assert_eq!(layer.element().button_type, ButtonType::Plain);
}

#[test]
fn block_pointer_containers_swallow_clicks_on_empty_areas() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let backdrop = tree.add_container(
        Some(root),
        FlexStyle {
            width: Some(200.0),
            height: Some(100.0),
            ..Default::default()
        },
    );
    tree.get_mut(backdrop).visual.block_pointer = true;
    let plain = tree.add_container(
        Some(root),
        FlexStyle {
            width: Some(200.0),
            height: Some(100.0),
            ..Default::default()
        },
    );
    tree.layout(zant::core::geometry::Size {
        width: 300.0,
        height: 300.0,
    });
    assert_eq!(tree.hit_test(10.0, 10.0), Some(backdrop));
    assert_eq!(tree.hit_test(10.0, tree.get(plain).bounds.y + 10.0), None);
}

pub(super) struct Dim;

impl App for Dim {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            grow: 1.0,
            background: Some(0xffffffff),
            children: zant::nodes![dismiss_layer(DismissLayerOptions {
                background: Some(0xff000000),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn dismiss_layer_paints_its_dimming_background() {
    let mut runtime = Runtime::init(
        Dim,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 100.0,
                height: 100.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(100, 100).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 100.0, 100.0, 1.0);
        renderer.end();
    }
    assert_eq!(renderer.pixel_at(50, 50), Some(0xff000000));
}
