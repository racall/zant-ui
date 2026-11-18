//! Absolutely positioned children, rich text and anchored children.

use super::*;

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
