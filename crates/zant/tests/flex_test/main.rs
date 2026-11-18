//! 24 retained-tree layout cases.

use zant::core::geometry::{Edges, Size};
use zant::core::node::{ButtonSize, ButtonType, FlexStyle, NodeData, NodeId, RichSpan};
use zant::core::tree::Tree;
use zant::flex;
use zant::renderer::text;
use zant::theme::tokens::ant_light;

mod absolute;
mod basics;
mod constraints;

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
