//! Select field, dropdown, and tooltip unit tests.

use super::*;
use crate::Str;
use crate::core::geometry::{Rect, Size};
use crate::core::node::{FlexStyle, NodeData, RetainedNode, SelectData, SelectOption};
use crate::renderer::renderer::Renderer;
use crate::renderer::text::TextLayout;
use crate::theme::tokens::ant_light;

const CLEAR: u32 = 0xff00_ff00;
const WIDTH: usize = 400;
const HEIGHT: usize = 300;

fn node(options: &[&str], selected: usize) -> RetainedNode {
    let data = SelectData {
        options: options
            .iter()
            .map(|label| SelectOption {
                label: Str::copy(label),
            })
            .collect(),
        selected,
        ..Default::default()
    };
    let mut node = RetainedNode::new(0, NodeData::Select(data));
    node.bounds = Rect::init(8.0, 8.0, 220.0, FIELD_HEIGHT);
    node
}

fn paint_into(node: &mut RetainedNode, width: usize, height: usize, theme: Theme) -> Renderer {
    let mut renderer =
        Renderer::create_raster(width as u32, height as u32).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
        paint(node, canvas, theme);
    }
    renderer.end();
    renderer
}

fn changed_pixels(a: &Renderer, b: &Renderer, width: usize, height: usize) -> usize {
    let mut changed = 0;
    for y in 0..height {
        for x in 0..width {
            if a.pixel_at(x, y) != b.pixel_at(x, y) {
                changed += 1;
            }
        }
    }
    changed
}

#[test]
fn selected_label_falls_back_to_the_placeholder() {
    let data = SelectData {
        options: vec![
            SelectOption {
                label: Str::from("A"),
            },
            SelectOption {
                label: Str::from("B"),
            },
        ],
        selected: 1,
        ..Default::default()
    };
    assert_eq!(selected_label(&data), "B");
    assert_eq!(
        selected_label(&SelectData {
            selected: 5,
            ..Default::default()
        }),
        "Select…"
    );
    assert_eq!(selected_label(&SelectData::default()), "Select…");
}

#[test]
fn ensure_layout_caches_the_label_and_every_option() {
    let theme = ant_light();
    let mut data = SelectData {
        options: vec![
            SelectOption {
                label: Str::from("Alpha"),
            },
            SelectOption {
                label: Str::from("Beta"),
            },
        ],
        ..Default::default()
    };
    let layout =
        ensure_layout(&mut data, theme.color_text, theme.color_text).expect("label layout");
    assert!(layout.metrics().longest_line > 0.0);
    assert_eq!(data.option_layouts.len(), 2);
    assert_eq!(data.layout_color, theme.color_text);
    assert!(!data.layout_dirty);
    // a different colour re-shapes the label, an unchanged colour reuses it
    let first = data
        .layout
        .as_ref()
        .map(|layout| layout.metrics().longest_line);
    ensure_layout(&mut data, theme.color_text, theme.color_text).expect("cached layout");
    assert_eq!(
        data.layout
            .as_ref()
            .map(|layout| layout.metrics().longest_line),
        first
    );
    ensure_layout(&mut data, theme.color_text_disabled, theme.color_text)
        .expect("relaid-out label");
    assert_eq!(data.layout_color, theme.color_text_disabled);
}

#[test]
fn closed_control_paints_its_fill_and_chevron() {
    let theme = ant_light();
    let mut closed = node(&["A", "B"], 0);
    let down = paint_into(&mut closed, WIDTH, HEIGHT, theme);
    // rounded field at (8, 8): (14, 24) is left padding, the container fill
    assert_eq!(down.pixel_at(14, 24), Some(theme.color_bg_container));

    let mut open = node(&["A", "B"], 0);
    open_select(&mut open, true);
    let up = paint_into(&mut open, WIDTH, HEIGHT, theme);
    assert!(
        changed_pixels(&down, &up, WIDTH, HEIGHT) > 0,
        "the chevron flips when the popup is open"
    );
}

fn open_select(node: &mut RetainedNode, open: bool) {
    if let NodeData::Select(data) = &mut node.data {
        data.open = open;
    }
}

#[test]
fn hover_focus_and_disabled_change_the_field() {
    let theme = ant_light();
    let mut plain = node(&["A", "B"], 0);
    let base = paint_into(&mut plain, WIDTH, HEIGHT, theme);

    let mut hovered = node(&["A", "B"], 0);
    if let NodeData::Select(data) = &mut hovered.data {
        data.hover_t = 1.0;
    }
    let hover = paint_into(&mut hovered, WIDTH, HEIGHT, theme);
    assert!(changed_pixels(&base, &hover, WIDTH, HEIGHT) > 0);

    let mut focused = node(&["A", "B"], 0);
    if let NodeData::Select(data) = &mut focused.data {
        data.focus_t = 1.0;
    }
    let focus = paint_into(&mut focused, WIDTH, HEIGHT, theme);
    assert!(
        changed_pixels(&base, &focus, WIDTH, HEIGHT) > 0,
        "the focus ring and active border show"
    );

    let mut disabled = node(&["A", "B"], 0);
    disabled.enabled = false;
    let dimmed = paint_into(&mut disabled, WIDTH, HEIGHT, theme);
    assert_ne!(dimmed.pixel_at(14, 24), base.pixel_at(14, 24));
}

#[test]
fn popup_rect_sits_below_flips_above_and_clamps() {
    let node = node(&["A", "B", "C"], 0);
    let rect = popup_rect(
        &node,
        Size {
            width: 400.0,
            height: 300.0,
        },
    );
    assert_eq!(
        rect,
        Rect::init(8.0, 44.0, 220.0, 3.0 * ROW_HEIGHT + POPUP_PADDING * 2.0)
    );

    // near the bottom edge the dropdown flips above and clamps to 8px
    let rect = popup_rect(
        &node,
        Size {
            width: 400.0,
            height: 100.0,
        },
    );
    assert_eq!(rect.y, 8.0);
}

#[test]
fn option_at_hits_the_rows_only() {
    let node = node(&["A", "B", "C"], 0);
    let viewport = Size {
        width: 400.0,
        height: 300.0,
    };
    assert_eq!(
        option_at(
            &node,
            100.0,
            44.0 + POPUP_PADDING + ROW_HEIGHT * 2.5,
            viewport
        ),
        Some(2)
    );
    assert_eq!(option_at(&node, 100.0, 45.0, viewport), None, "padding");
    assert_eq!(option_at(&node, 300.0, 100.0, viewport), None, "sideways");
    assert_eq!(
        option_at(
            &node,
            100.0,
            44.0 + 2.0 * POPUP_PADDING + 3.0 * ROW_HEIGHT - 1.0,
            viewport
        ),
        None,
        "below the rows"
    );
}

#[test]
fn popup_paints_rows_hover_and_selection() {
    let theme = ant_light();
    let mut node = node(&["A", "B", "C"], 1);
    if let NodeData::Select(data) = &mut node.data {
        data.open = true;
        data.open_t = 1.0;
        data.hovered_option = Some(2);
    }
    ensure_layout(
        match &mut node.data {
            NodeData::Select(data) => data,
            _ => unreachable!(),
        },
        theme.color_text,
        theme.color_text,
    )
    .expect("layouts");
    let viewport = Size {
        width: WIDTH as f32,
        height: HEIGHT as f32,
    };
    let rect = popup_rect(&node, viewport);
    let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
        paint_popup(&node, canvas, theme, viewport);
    }
    renderer.end();
    // rows are 30px tall from y = rect.y + 4, 4px inside the popup x
    let row = |index: usize| rect.y + POPUP_PADDING + index as f32 * ROW_HEIGHT;
    let plain_pixel = renderer.pixel_at(100, row(1) as usize + 15).expect("pixel");
    assert_eq!(plain_pixel, theme.color_bg_elevated);
    let hovered_pixel = renderer.pixel_at(100, row(2) as usize + 15).expect("pixel");
    assert_ne!(hovered_pixel, plain_pixel);
    // selected row 1 keeps the 2px primary marker at its left
    assert_eq!(
        renderer.pixel_at(
            rect.x as usize + POPUP_PADDING as usize,
            row(1) as usize + 15
        ),
        Some(theme.color_primary)
    );
}

#[test]
fn popup_paints_nothing_until_it_opens() {
    let theme = ant_light();
    let mut node = node(&["A", "B"], 0);
    let viewport = Size {
        width: WIDTH as f32,
        height: HEIGHT as f32,
    };
    let closed = {
        let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            paint_popup(&node, canvas, theme, viewport);
        }
        renderer.end();
        renderer
    };
    let mut blank = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
    {
        let canvas = blank.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
    }
    blank.end();
    assert_eq!(changed_pixels(&closed, &blank, WIDTH, HEIGHT), 0);

    if let NodeData::Select(data) = &mut node.data {
        data.open_t = 0.5;
    }
    let opening = {
        let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            paint_popup(&node, canvas, theme, viewport);
        }
        renderer.end();
        renderer
    };
    assert!(changed_pixels(&opening, &blank, WIDTH, HEIGHT) > 0);
}

#[test]
fn animate_tracks_hover_focus_and_open() {
    let theme = ant_light();
    let mut data = SelectData {
        open: true,
        hovered: true,
        ..Default::default()
    };
    assert!(animate(&mut data, 1_000, theme));
    assert_eq!(data.open_t, 0.0);
    animate(&mut data, 1_100, theme);
    assert!(data.hover_t > 0.0 && data.hover_t < 1.0);
    assert!(data.open_t > 0.0 && data.open_t < 1.0);
    assert!(!animate(&mut data, 10_000, theme));
    assert_eq!(data.hover_t, 1.0);
    assert_eq!(data.focus_t, 1.0, "open counts as focused");
    assert_eq!(data.open_t, 1.0);
}

#[test]
fn tree_paint_dispatches_to_the_select_module() {
    use crate::core::tree::Tree;
    let theme = ant_light();
    let mut tree = Tree::init(theme);
    let root = tree.root;
    tree.add_select(Some(root), &["A", "B"], 1);
    tree.layout(Size {
        width: 400.0,
        height: 100.0,
    });
    let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
        tree.paint(canvas);
    }
    renderer.end();
    // the field at (0, 0, 220, 32): (4, 16) is the container fill, left of
    // the label
    assert_eq!(renderer.pixel_at(4, 16), Some(theme.color_bg_container));
}

#[test]
fn measure_and_padding_follow_the_style() {
    let style = FlexStyle::default();
    assert_eq!(measure_width(&style), 220.0);
    assert_eq!(measure_height(&style), FIELD_HEIGHT);
    let sized = FlexStyle {
        width: Some(160.0),
        height: Some(40.0),
        ..Default::default()
    };
    assert_eq!(measure_width(&sized), 160.0);
    assert_eq!(measure_height(&sized), 40.0);
    assert_eq!(control_height(), FIELD_HEIGHT);
    assert_eq!(horizontal_padding(), PADDING_X);
    assert_eq!(
        font_size(&SelectData {
            font_size: 16.0,
            ..Default::default()
        }),
        16.0
    );
}

#[test]
fn tooltip_rect_centres_below_and_flips_above() {
    let layout = TextLayout::init("Tip", tooltip::text_style(), 1.0e6).expect("tooltip layout");
    let (width, height) = tooltip::measure(&layout);
    let bounds = Rect::init(100.0, 100.0, 80.0, 24.0);
    let (rect, above) = tooltip::rect(
        bounds,
        Size {
            width: 400.0,
            height: 300.0,
        },
        &layout,
    );
    assert!(!above);
    assert_eq!(rect.width, width);
    assert_eq!(rect.height, height);
    assert_eq!(rect.y, 124.0 + tooltip::GAP);
    assert_eq!(rect.x, 100.0 + (80.0 - width) * 0.5);

    // near the bottom it flips above the target, still 8px from the top
    let (rect, above) = tooltip::rect(
        Rect::init(100.0, 250.0, 80.0, 24.0),
        Size {
            width: 400.0,
            height: 300.0,
        },
        &layout,
    );
    assert!(above);
    assert_eq!(rect.y, 250.0 - height - tooltip::GAP);
    assert!(rect.y + rect.height <= 250.0);

    // clamped to the left margin when the target sticks out on the left
    let (rect, _) = tooltip::rect(
        Rect::init(-100.0, 100.0, 10.0, 24.0),
        Size {
            width: 400.0,
            height: 300.0,
        },
        &layout,
    );
    assert_eq!(rect.x, tooltip::MARGIN);
}

#[test]
fn tooltip_paint_fades_in() {
    let layout = TextLayout::init("Tip", tooltip::text_style(), 1.0e6).expect("tooltip layout");
    let (rect, above) = tooltip::rect(
        Rect::init(100.0, 100.0, 80.0, 24.0),
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        },
        &layout,
    );
    let render = |value: f32| {
        let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            tooltip::paint(canvas, &layout, rect, above, value);
        }
        renderer.end();
        renderer
    };
    let hidden = render(0.0);
    let shown = render(1.0);
    assert!(changed_pixels(&hidden, &shown, WIDTH, HEIGHT) > 0);
    // the spotlight box covers the middle of the rect
    let pixel = shown
        .pixel_at(
            (rect.x + 20.0) as usize,
            (rect.y + rect.height - 3.0) as usize,
        )
        .expect("pixel");
    assert_ne!(pixel, CLEAR);
    let mut blank = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
    {
        let canvas = blank.begin(1.0).expect("frame canvas");
        canvas.clear(CLEAR);
    }
    blank.end();
    assert_eq!(changed_pixels(&hidden, &blank, WIDTH, HEIGHT), 0);
}
