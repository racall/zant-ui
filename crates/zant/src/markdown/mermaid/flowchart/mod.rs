//! Flowchart graph types and rendering entry point.

use crate::Str;

mod draw;
mod parse;

pub(super) use self::draw::flowchart;

// ── flowchart ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    Rect,
    Round,
    Stadium,
    Circle,
    Diamond,
    Hexagon,
    Subroutine,
    Cylinder,
    Asymmetric,
}

pub(super) struct FNode {
    id: Str,
    label: Str,
    pub(super) shape: Shape,
    layer: usize,
    order: f32,
    w: f32,
    h: f32,
    x: f32,
    y: f32,
}

impl FNode {
    fn new(id: &str) -> FNode {
        FNode {
            id: Str::copy(id),
            label: Str::copy(id),
            shape: Shape::Rect,
            layer: 0,
            order: 0.0,
            w: 0.0,
            h: 0.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeStyle {
    Solid,
    Dotted,
    Thick,
}

pub(super) struct FEdge {
    from: usize,
    to: usize,
    pub(super) label: Str,
    arrow: bool,
    style: EdgeStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Td,
    Bt,
    Lr,
    Rl,
}

#[derive(Default)]
pub(super) struct Flow {
    pub(super) nodes: Vec<FNode>,
    pub(super) edges: Vec<FEdge>,
}
