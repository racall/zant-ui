//! Row / column layout, padding, margin, gap, grow and shrink.

use super::*;

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
