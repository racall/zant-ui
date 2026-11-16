//! 24 retained-tree layout cases.

use zant::core::geometry::{Edges, Size};
use zant::core::node::{ButtonSize, ButtonType, FlexStyle, NodeData, NodeId, RichSpan};
use zant::core::tree::Tree;
use zant::layout::flex;
use zant::renderer::text;
use zant::theme::tokens::ant_light;

fn boxed(tree: &mut Tree, parent: NodeId, style: FlexStyle) -> NodeId {
    tree.add_container(Some(parent), style)
}

fn add_box(tree: &mut Tree, style: FlexStyle) -> NodeId {
    let root = tree.root;
    boxed(tree, root, style)
}

fn root_row(tree: &mut Tree, size: Size) {
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    tree.get_mut(root).style.align_items = flex::Align::Start;
    tree.layout(size);
}

fn expect_rect(node: &zant::core::node::RetainedNode, x: f32, y: f32, w: f32, h: f32) {
    assert!(
        (node.bounds.x - x).abs() <= 0.01,
        "x {} != {x}",
        node.bounds.x
    );
    assert!(
        (node.bounds.y - y).abs() <= 0.01,
        "y {} != {y}",
        node.bounds.y
    );
    assert!(
        (node.bounds.width - w).abs() <= 0.01,
        "width {} != {w}",
        node.bounds.width
    );
    assert!(
        (node.bounds.height - h).abs() <= 0.01,
        "height {} != {h}",
        node.bounds.height
    );
}

fn approx(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected} (±{tolerance})"
    );
}

#[test]
fn horizontal_layout_with_fixed_widths_and_gap() {
    let mut tree = Tree::init(ant_light());
    add_box(
        &mut tree,
        FlexStyle {
            width: Some(100.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    add_box(
        &mut tree,
        FlexStyle {
            width: Some(80.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    add_box(
        &mut tree,
        FlexStyle {
            width: Some(60.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 500.0,
            height: 200.0,
        },
    );
    expect_rect(tree.get(1), 0.0, 0.0, 100.0, 50.0);
    expect_rect(tree.get(2), 100.0, 0.0, 80.0, 50.0);
    expect_rect(tree.get(3), 180.0, 0.0, 60.0, 50.0);
}

#[test]
fn an_empty_placeholder_child_takes_no_space_or_gap() {
    // a TSX component that returned `null` builds `ui::Node::default()`, an
    // empty Root element: like React's `null` it must not add a gap
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.gap = 10.0;
    let first = add_box(
        &mut tree,
        FlexStyle {
            width: Some(100.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    let placeholder = tree.create_node(NodeData::Root);
    tree.add_child(root, placeholder);
    let last = add_box(
        &mut tree,
        FlexStyle {
            width: Some(60.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 500.0,
            height: 200.0,
        },
    );
    expect_rect(tree.get(first), 0.0, 0.0, 100.0, 50.0);
    expect_rect(tree.get(last), 110.0, 0.0, 60.0, 50.0);
}

#[test]
fn column_layout_and_auto_height() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Column;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(100.0),
            height: Some(40.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(100.0),
            height: Some(60.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 300.0,
        height: 200.0,
    });
    expect_rect(tree.get(1), 0.0, 0.0, 100.0, 40.0);
    expect_rect(tree.get(2), 0.0, 40.0, 100.0, 60.0);
}

#[test]
fn padding_and_margin() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Column;
    tree.get_mut(root).style.padding = Edges {
        left: 20.0,
        top: 10.0,
        right: 20.0,
        bottom: 10.0,
    };
    let row = tree.add_container(
        Some(root),
        FlexStyle {
            direction: flex::Direction::Row,
            padding: Edges::all(10.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        row,
        FlexStyle {
            width: Some(50.0),
            height: Some(20.0),
            margin: Edges {
                left: 5.0,
                right: 7.0,
                top: 3.0,
                bottom: 4.0,
            },
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        row,
        FlexStyle {
            width: Some(40.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 300.0,
        height: 200.0,
    });
    expect_rect(tree.get(row), 20.0, 10.0, 122.0, 47.0);
    expect_rect(tree.get(2), 35.0, 23.0, 50.0, 20.0);
    expect_rect(tree.get(3), 87.0, 20.0, 40.0, 20.0);
}

#[test]
fn gap_between_row_items() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    tree.get_mut(root).style.gap = 12.0;
    for _ in 0..3 {
        boxed(
            &mut tree,
            root,
            FlexStyle {
                width: Some(30.0),
                height: Some(20.0),
                ..Default::default()
            },
        );
    }
    tree.layout(Size {
        width: 300.0,
        height: 100.0,
    });
    expect_rect(tree.get(2), 42.0, 0.0, 30.0, 20.0);
    expect_rect(tree.get(3), 84.0, 0.0, 30.0, 20.0);
}

#[test]
fn grow_distributes_free_space() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    tree.get_mut(root).style.padding = Edges::all(10.0);
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(100.0),
            height: Some(30.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            basis: Some(100.0),
            grow: 1.0,
            height: Some(30.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 400.0,
        height: 100.0,
    });
    expect_rect(tree.get(1), 10.0, 10.0, 100.0, 30.0);
    expect_rect(tree.get(2), 110.0, 10.0, 280.0, 30.0);
}

#[test]
fn shrink_reduces_overflowing_items() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(200.0),
            height: Some(30.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(100.0),
            height: Some(30.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    assert!(tree.get(1).bounds.width < 200.0);
    assert!(tree.get(2).bounds.width < 100.0);
    approx(
        tree.get(1).bounds.width + tree.get(2).bounds.width,
        200.0,
        0.1,
    );
}

#[test]
fn min_and_max_size_constraints() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            basis: Some(50.0),
            grow: 1.0,
            max_width: Some(80.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            basis: Some(50.0),
            grow: 1.0,
            min_width: Some(90.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 400.0,
        height: 100.0,
    });
    approx(tree.get(1).bounds.width, 80.0, 0.1);
    assert!(tree.get(2).bounds.width >= 90.0);
}

#[test]
fn nested_flex_and_window_resize() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Column;
    tree.get_mut(root).style.padding = Edges::all(10.0);
    let row = tree.add_container(
        Some(root),
        FlexStyle {
            direction: flex::Direction::Row,
            gap: 10.0,
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        row,
        FlexStyle {
            width: Some(100.0),
            height: Some(40.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        row,
        FlexStyle {
            width: Some(100.0),
            height: Some(40.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 400.0,
        height: 200.0,
    });
    expect_rect(tree.get(row), 10.0, 10.0, 210.0, 40.0);

    tree.layout(Size {
        width: 300.0,
        height: 200.0,
    });
    expect_rect(tree.get(row), 10.0, 10.0, 210.0, 40.0);
    expect_rect(tree.get(2), 10.0, 10.0, 100.0, 40.0);
    expect_rect(tree.get(3), 120.0, 10.0, 100.0, 40.0);
}

#[test]
fn justify_space_between() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    tree.get_mut(root).style.justify = flex::Justify::SpaceBetween;
    for _ in 0..3 {
        boxed(
            &mut tree,
            root,
            FlexStyle {
                width: Some(50.0),
                height: Some(20.0),
                ..Default::default()
            },
        );
    }
    tree.layout(Size {
        width: 250.0,
        height: 100.0,
    });
    expect_rect(tree.get(1), 0.0, 0.0, 50.0, 20.0);
    expect_rect(tree.get(2), 100.0, 0.0, 50.0, 20.0);
    expect_rect(tree.get(3), 200.0, 0.0, 50.0, 20.0);
}

#[test]
fn align_center_and_end() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    tree.get_mut(root).style.align_items = flex::Align::Center;
    tree.get_mut(root).style.height = Some(100.0);
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(50.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    approx(tree.get(1).bounds.y, 40.0, 0.1);

    tree.get_mut(root).style.align_items = flex::Align::End;
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    approx(tree.get(1).bounds.y, 80.0, 0.1);
}

#[test]
fn row_wrap_creates_multiple_lines() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    tree.get_mut(root).style.wrap = flex::Wrap::Wrap;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(60.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(60.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(30.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 100.0,
        height: 200.0,
    });
    expect_rect(tree.get(1), 0.0, 0.0, 60.0, 20.0);
    expect_rect(tree.get(2), 0.0, 20.0, 60.0, 20.0);
    expect_rect(tree.get(3), 60.0, 20.0, 30.0, 20.0);
}

#[test]
fn percentage_width_resolves_against_parent() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width_percent: Some(50.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    expect_rect(tree.get(1), 0.0, 0.0, 100.0, 20.0);
}

#[test]
fn order_reorders_flex_items_without_changing_tree_ids() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(50.0),
            height: Some(20.0),
            order: 2,
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(50.0),
            height: Some(20.0),
            order: 1,
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    expect_rect(tree.get(2), 0.0, 0.0, 50.0, 20.0);
    expect_rect(tree.get(1), 50.0, 0.0, 50.0, 20.0);
}

#[test]
fn align_self_centers_one_child() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Row;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(20.0),
            height: Some(20.0),
            align_self: flex::AlignSelf::Center,
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    expect_rect(tree.get(1), 0.0, 40.0, 20.0, 20.0);
}

#[test]
fn absolute_child_is_positioned_without_taking_flow_space() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    tree.get_mut(root).style.direction = flex::Direction::Column;
    boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(50.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    let absolute = tree.add_container(
        Some(root),
        FlexStyle {
            absolute: true,
            right: Some(20.0),
            top: Some(10.0),
            width: Some(100.0),
            height: Some(40.0),
            ..Default::default()
        },
    );
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    expect_rect(tree.get(1), 0.0, 0.0, 50.0, 20.0);
    expect_rect(tree.get(absolute), 80.0, 10.0, 100.0, 40.0);
}

#[test]
fn rich_text_lays_out_and_wraps() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let spans = [
        RichSpan {
            text: "Hello ".into(),
            font_size: 15.0,
            color: 0xff29_2d2a,
            ..Default::default()
        },
        RichSpan {
            text: "world this is a long richer text run".into(),
            font_size: 15.0,
            weight: text::Weight::Semibold,
            color: 0xff60_7d6b,
            ..Default::default()
        },
    ];
    let id = tree.add_rich_text(Some(root), &spans);
    tree.get_mut(id).style.width_percent = Some(100.0);
    tree.layout(Size {
        width: 120.0,
        height: 200.0,
    });
    assert!(tree.get(id).bounds.height >= 20.0);
    assert!(tree.get(id).bounds.width <= 120.1);
}

#[test]
fn absolute_button_participates_in_hit_testing() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let id = tree.add_button(
        Some(root),
        "menu",
        ButtonType::Text,
        ButtonSize::Middle,
        None,
    );
    {
        let node = tree.get_mut(id);
        node.style.absolute = true;
        node.style.top = Some(10.0);
        node.style.right = Some(10.0);
        node.style.width = Some(80.0);
        node.style.height = Some(30.0);
    }
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    expect_rect(tree.get(id), 110.0, 10.0, 80.0, 30.0);
    assert_eq!(tree.hit_test(140.0, 20.0), Some(id));
}

#[test]
fn absolute_child_resolves_percentage_height_full_size_overlays() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let overlay = tree.add_container(
        Some(root),
        FlexStyle {
            absolute: true,
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 400.0,
            height: 300.0,
        },
    );
    expect_rect(tree.get(overlay), 0.0, 0.0, 400.0, 300.0);
}

#[test]
fn stretch_fills_an_auto_height_row_and_absolute_children_do_not_size_parents() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let row = boxed(
        &mut tree,
        root,
        FlexStyle {
            direction: flex::Direction::Row,
            align_items: flex::Align::Stretch,
            ..Default::default()
        },
    );
    let bar = boxed(
        &mut tree,
        row,
        FlexStyle {
            width: Some(3.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        row,
        FlexStyle {
            width: Some(50.0),
            height: Some(40.0),
            ..Default::default()
        },
    );
    let pill = boxed(
        &mut tree,
        root,
        FlexStyle {
            direction: flex::Direction::Row,
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        pill,
        FlexStyle {
            absolute: true,
            width_percent: Some(100.0),
            height: Some(10.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        pill,
        FlexStyle {
            width: Some(30.0),
            height: Some(10.0),
            ..Default::default()
        },
    );
    tree.get_mut(root).style.direction = flex::Direction::Column;
    tree.get_mut(root).style.align_items = flex::Align::Start;
    tree.layout(Size {
        width: 500.0,
        height: 300.0,
    });
    approx(tree.get(bar).bounds.height, 40.0, 0.01);
    approx(tree.get(pill).bounds.width, 30.0, 0.01);
}

#[test]
fn absolute_percentage_height_wins_over_a_builders_fixed_height() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let item = boxed(
        &mut tree,
        root,
        FlexStyle {
            direction: flex::Direction::Row,
            height: Some(24.0),
            width: Some(80.0),
            ..Default::default()
        },
    );
    // like ui::styled(ui::button(..) /* height 32 */, .{ .height_percent = 100 })
    let hit = boxed(
        &mut tree,
        item,
        FlexStyle {
            absolute: true,
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height: Some(32.0),
            height_percent: Some(100.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 400.0,
            height: 300.0,
        },
    );
    expect_rect(tree.get(hit), 0.0, 0.0, 80.0, 24.0);
}

#[test]
fn absolute_children_use_the_padding_box_css_containing_block() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let item = boxed(
        &mut tree,
        root,
        FlexStyle {
            direction: flex::Direction::Row,
            height: Some(24.0),
            padding: Edges {
                left: 11.0,
                right: 11.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let hit = boxed(
        &mut tree,
        item,
        FlexStyle {
            absolute: true,
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        item,
        FlexStyle {
            width: Some(40.0),
            height: Some(24.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 400.0,
            height: 300.0,
        },
    );
    expect_rect(tree.get(item), 0.0, 0.0, 62.0, 24.0);
    expect_rect(tree.get(hit), 0.0, 0.0, 62.0, 24.0);
}

#[test]
fn anchored_absolute_child_takes_the_target_siblings_box() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let track = boxed(
        &mut tree,
        root,
        FlexStyle {
            direction: flex::Direction::Row,
            padding: Edges::all(2.0),
            gap: 2.0,
            align_items: flex::Align::Center,
            ..Default::default()
        },
    );
    let thumb = boxed(
        &mut tree,
        track,
        FlexStyle {
            absolute: true,
            anchor: Some(1),
            ..Default::default()
        },
    );
    let ink = boxed(
        &mut tree,
        track,
        FlexStyle {
            absolute: true,
            anchor: Some(2),
            bottom: Some(0.0),
            height: Some(2.0),
            ..Default::default()
        },
    );
    boxed(
        &mut tree,
        track,
        FlexStyle {
            width: Some(40.0),
            height: Some(24.0),
            ..Default::default()
        },
    );
    let second = boxed(
        &mut tree,
        track,
        FlexStyle {
            width: Some(60.0),
            height: Some(24.0),
            ..Default::default()
        },
    );
    let third = boxed(
        &mut tree,
        track,
        FlexStyle {
            width: Some(30.0),
            height: Some(24.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 500.0,
            height: 200.0,
        },
    );
    let target = tree.get(second).bounds;
    expect_rect(
        tree.get(thumb),
        target.x,
        target.y,
        target.width,
        target.height,
    );
    // own vertical placement keeps bottom / height
    let t3 = tree.get(third).bounds;
    let tb = tree.get(track).bounds;
    expect_rect(tree.get(ink), t3.x, tb.y + tb.height - 2.0, t3.width, 2.0);
}

#[test]
fn single_line_text_with_max_lines_shrinks_and_ends_in_an_ellipsis() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let row = boxed(
        &mut tree,
        root,
        FlexStyle {
            direction: flex::Direction::Row,
            width: Some(120.0),
            ..Default::default()
        },
    );
    let label = tree.add_text(
        Some(row),
        "A very long document name that cannot fit.md",
        text::Style {
            font_size: 13.0,
            wrap: false,
            max_lines: 1,
            ..Default::default()
        },
    );
    tree.get_mut(label).style.min_width = Some(0.0);
    root_row(
        &mut tree,
        Size {
            width: 500.0,
            height: 200.0,
        },
    );
    let bounds = tree.get(label).bounds;
    assert!(bounds.width <= 120.5);
    let max_width = match &tree.get(label).data {
        NodeData::Text(data) => data.layout_width(bounds.width),
        _ => panic!("expected text"),
    };
    let longest_line = match &mut tree.get_mut(label).data {
        NodeData::Text(data) => data.ensure_layout(max_width).map(|l| {
            let metrics = l.metrics();
            metrics.longest_line
        }),
        _ => None,
    }
    .expect("layout");
    assert!(longest_line <= 121.0);
    approx(bounds.height, 13.0 * 1.2, 6.0);
}

#[test]
fn absolute_child_with_left_and_right_stretches_between_the_insets() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let bar = boxed(
        &mut tree,
        root,
        FlexStyle {
            width: Some(400.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    let title = boxed(
        &mut tree,
        bar,
        FlexStyle {
            absolute: true,
            left: Some(120.0),
            right: Some(120.0),
            top: Some(0.0),
            height: Some(50.0),
            ..Default::default()
        },
    );
    root_row(
        &mut tree,
        Size {
            width: 500.0,
            height: 200.0,
        },
    );
    expect_rect(tree.get(title), 120.0, 0.0, 160.0, 50.0);
}
