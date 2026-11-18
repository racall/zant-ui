//! Size constraints, nesting, justification, alignment, wrapping, percentages and order.

use super::*;

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
