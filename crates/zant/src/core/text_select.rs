//! Text selection across the text
//! nodes of a `selectable` container: document-order collection, hit testing
//! (nearest text when the pointer is between nodes), word expansion and the
//! copied text. Collected ids and copied text are returned as owned `Vec` /
//! `String` values.
#![forbid(unsafe_code)]

use crate::core::node::{NodeData, NodeId, RetainedNode};
use crate::core::tree::Tree;
use crate::core::unicode;

/// A position in a text node: node plus UTF-8 byte offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub node: NodeId,
    pub byte: usize,
}

/// Result of [`ordered`]: the selection sorted in document order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ordered {
    pub start: Point,
    pub end: Point,
    pub si: usize,
    pub ei: usize,
}

pub fn is_text(node: &RetainedNode) -> bool {
    match &node.data {
        NodeData::Text(_) => true,
        NodeData::RichText(_) => true,
        NodeData::Vector(data) => !data.copy.is_empty(),
        _ => false,
    }
}

/// Text and rich text nodes of `root`'s subtree, in document order.
pub fn collect(tree: &Tree, root: NodeId, out: &mut Vec<NodeId>) {
    let node = tree.get(root);
    if !node.alive || !node.in_flow() || node.visual.unselectable {
        return;
    }
    if is_text(node) {
        out.push(root);
    }
    for &child in &node.children {
        collect(tree, child, out);
    }
}

/// Innermost `selectable` container under (x, y).
pub fn root_at(tree: &Tree, x: f32, y: f32) -> Option<NodeId> {
    root_at_node(tree, tree.root, x, y)
}

fn root_at_node(tree: &Tree, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
    let node = tree.get(id);
    if !node.in_flow() || !node.bounds.contains(x, y) {
        return None;
    }
    let mut i = node.children.len();
    while i > 0 {
        i -= 1;
        if let Some(hit) = root_at_node(tree, node.children[i], x, y) {
            return Some(hit);
        }
    }
    if node.visual.selectable {
        Some(node.id)
    } else {
        None
    }
}

/// The node's text as laid out (rich text: spans joined).
pub fn node_text(node: &RetainedNode) -> String {
    match &node.data {
        NodeData::Text(data) => data.content.to_string(),
        NodeData::RichText(data) => data.joined_text(),
        NodeData::Vector(data) => data.copy.to_string(),
        _ => String::new(),
    }
}

fn byte_at(node: &mut RetainedNode, x: f32, y: f32) -> usize {
    let bounds = node.bounds;
    let lx = x - bounds.x;
    let ly = y - bounds.y;
    match &mut node.data {
        NodeData::Text(data) => {
            let content_len = data.content.len();
            let width = data.layout_width(bounds.width);
            data.ensure_layout(width)
                .map_or(0, |layout| layout.index_at(lx, ly).min(content_len))
        }
        NodeData::RichText(data) => data
            .ensure_layout(bounds.width)
            .map_or(0, |layout| layout.index_at(lx, ly)),
        // a drawing is one unit: before or after it
        NodeData::Vector(data) => {
            if lx < bounds.width / 2.0 {
                0
            } else {
                data.copy.len()
            }
        }
        _ => 0,
    }
}

/// Text position under (x, y): the node containing the point, else the nearest
/// node (lines first, then columns), clamped into it.
pub fn point_at(tree: &mut Tree, nodes: &[NodeId], x: f32, y: f32) -> Option<Point> {
    let mut best: Option<NodeId> = None;
    let mut best_score = f32::INFINITY;
    for &id in nodes {
        let b = tree.get(id).bounds;
        let dy = if y < b.y {
            b.y - y
        } else if y > b.bottom() {
            y - b.bottom()
        } else {
            0.0
        };
        let dx = if x < b.x {
            b.x - x
        } else if x > b.right() {
            x - b.right()
        } else {
            0.0
        };
        let score = dy * 4.0 + dx;
        if score < best_score {
            best_score = score;
            best = Some(id);
        }
    }
    let id = best?;
    let b = tree.get(id).bounds;
    // above / below the node: its start / end
    if y < b.y {
        return Some(Point { node: id, byte: 0 });
    }
    if y > b.bottom() {
        return Some(Point {
            node: id,
            byte: text_len(tree.get(id)),
        });
    }
    let clamped_x = x.clamp(b.x, b.right());
    let byte = byte_at(tree.get_mut(id), clamped_x, y);
    Some(Point { node: id, byte })
}

pub fn text_len(node: &RetainedNode) -> usize {
    match &node.data {
        NodeData::Text(data) => data.content.len(),
        NodeData::RichText(data) => data.text_len(),
        NodeData::Vector(data) => data.copy.len(),
        _ => 0,
    }
}

pub fn index_of(nodes: &[NodeId], id: NodeId) -> Option<usize> {
    nodes.iter().position(|&node| node == id)
}

/// Word around `byte` in a node (double click).
pub fn word_at(node: &RetainedNode, byte: usize) -> [usize; 2] {
    let value = node_text(node);
    let at = byte.min(value.len());
    let start = unicode::word_start(&value, at);
    let mut end = unicode::word_end(&value, at);
    if start == end && end < value.len() {
        end = unicode::next_codepoint(&value, end);
    }
    [start, end]
}

/// Ordered selection (start, end) in node-list order.
pub fn ordered(nodes: &[NodeId], a: Point, b: Point) -> Option<Ordered> {
    let ai = index_of(nodes, a.node)?;
    let bi = index_of(nodes, b.node)?;
    if ai < bi || (ai == bi && a.byte <= b.byte) {
        Some(Ordered {
            start: a,
            end: b,
            si: ai,
            ei: bi,
        })
    } else {
        Some(Ordered {
            start: b,
            end: a,
            si: bi,
            ei: ai,
        })
    }
}

/// Copied text of a node range: spans' `copy` text replaces their text
/// (an embed is copied whole when any of it is selected).
fn append_range(node: &RetainedNode, from: usize, to: usize, out: &mut String) {
    match &node.data {
        NodeData::Text(data) => {
            let s = from.min(data.content.len());
            let e = to.min(data.content.len());
            if s < e && data.content.is_char_boundary(s) && data.content.is_char_boundary(e) {
                out.push_str(&data.content[s..e]);
            }
        }
        NodeData::RichText(data) => {
            let mut offset = 0usize;
            for span in &data.spans {
                let s0 = offset;
                let e0 = offset + span.text.len();
                offset = e0;
                let s = s0.max(from);
                let e = e0.min(to);
                if s >= e {
                    continue;
                }
                if let Some(copy) = span.copy.as_ref() {
                    out.push_str(copy);
                } else if span.text.is_char_boundary(s - s0) && span.text.is_char_boundary(e - s0) {
                    out.push_str(&span.text[s - s0..e - s0]);
                }
            }
        }
        NodeData::Vector(data) => {
            if from < to {
                out.push_str(&data.copy);
            }
        }
        _ => {}
    }
}

/// The selected text: nodes joined by a line break when the next starts
/// below the previous one, by a space when they sit side by side.
pub fn copy_text(tree: &Tree, nodes: &[NodeId], a: Point, b: Point) -> String {
    let mut out = String::new();
    let Some(range) = ordered(nodes, a, b) else {
        return out;
    };
    let mut prev: Option<&RetainedNode> = None;
    let mut i = range.si;
    while i <= range.ei {
        let node = tree.get(nodes[i]);
        let from = if i == range.si { range.start.byte } else { 0 };
        let to = if i == range.ei {
            range.end.byte
        } else {
            text_len(node)
        };
        if let Some(prev) = prev {
            if node.bounds.y >= prev.bounds.bottom() - 2.0 {
                out.push('\n');
            } else if out.chars().last().is_some_and(|last| last != ' ') {
                out.push(' ');
            }
        }
        append_range(node, from, to, &mut out);
        prev = Some(node);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;
    use crate::core::tree::Tree;
    use crate::flex;
    use crate::renderer::text;
    use crate::theme::tokens::ant_light;

    fn selectable_tree() -> (Tree, Vec<NodeId>) {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        tree.get_mut(root).style.direction = flex::Direction::Column;
        tree.get_mut(root).visual.selectable = true;
        let _first = tree.add_text(
            Some(root),
            "Hello world",
            text::Style {
                wrap: false,
                ..Default::default()
            },
        );
        let _second = tree.add_text(
            Some(root),
            "Second line",
            text::Style {
                wrap: false,
                ..Default::default()
            },
        );
        let third = tree.add_text(
            Some(root),
            "unselectable",
            text::Style {
                wrap: false,
                ..Default::default()
            },
        );
        tree.get_mut(third).visual.unselectable = true;
        tree.layout(Size {
            width: 300.0,
            height: 200.0,
        });
        let mut nodes = Vec::new();
        collect(&tree, root, &mut nodes);
        (tree, nodes)
    }

    #[test]
    fn collect_returns_text_nodes_in_document_order() {
        let (tree, nodes) = selectable_tree();
        assert_eq!(nodes.len(), 2, "unselectable text is skipped");
        assert_eq!(tree.get(nodes[0]).bounds.y, 0.0);
        assert!(tree.get(nodes[1]).bounds.y > 0.0);
        assert!(is_text(tree.get(nodes[0])));
        assert!(!is_text(tree.get(tree.root)));
    }

    #[test]
    fn root_at_finds_the_selectable_container() {
        let (tree, _) = selectable_tree();
        assert_eq!(root_at(&tree, 5.0, 5.0), Some(tree.root));
        assert_eq!(root_at(&tree, 500.0, 500.0), None);
    }

    #[test]
    fn point_at_clamps_into_the_nearest_text() {
        let (mut tree, nodes) = selectable_tree();
        // above the first node: its start
        let point = point_at(&mut tree, &nodes, 5.0, -10.0).expect("point");
        assert_eq!(point.node, nodes[0]);
        assert_eq!(point.byte, 0);
        // below the last node: its end
        let point = point_at(&mut tree, &nodes, 5.0, 500.0).expect("point");
        assert_eq!(point.node, nodes[1]);
        assert_eq!(point.byte, text_len(tree.get(nodes[1])));
        // inside the first node: a byte offset near the start
        let point = point_at(&mut tree, &nodes, 1.0, 5.0).expect("point");
        assert_eq!(point.node, nodes[0]);
        assert!(point.byte <= 2);
        assert!(point_at(&mut tree, &[], 0.0, 0.0).is_none());
    }

    #[test]
    fn word_at_expands_to_word_boundaries() {
        let (tree, nodes) = selectable_tree();
        assert_eq!(word_at(tree.get(nodes[0]), 1), [0, 5], "Hello");
        assert_eq!(word_at(tree.get(nodes[0]), 7), [6, 11], "world");
        let len = text_len(tree.get(nodes[0]));
        // past the end: the word ending there (`word_at` clamps the offset)
        assert_eq!(word_at(tree.get(nodes[0]), len), [6, 11]);
    }

    #[test]
    fn copy_text_joins_lines_with_a_newline_and_columns_with_a_space() {
        let (tree, nodes) = selectable_tree();
        let start = Point {
            node: nodes[0],
            byte: 0,
        };
        let end = Point {
            node: nodes[1],
            byte: text_len(tree.get(nodes[1])),
        };
        let copied = copy_text(&tree, &nodes, start, end);
        assert!(copied.contains("Hello world"), "copied: {copied:?}");
        assert!(copied.contains('\n'), "stacked nodes join with a newline");
        // a same-node range copies exactly
        let copied = copy_text(
            &tree,
            &nodes,
            Point {
                node: nodes[0],
                byte: 0,
            },
            Point {
                node: nodes[0],
                byte: 5,
            },
        );
        assert_eq!(copied, "Hello");
        // reversed points are ordered first
        let reversed = copy_text(
            &tree,
            &nodes,
            Point {
                node: nodes[0],
                byte: 5,
            },
            Point {
                node: nodes[0],
                byte: 0,
            },
        );
        assert_eq!(reversed, "Hello");
    }

    #[test]
    fn rich_text_and_vectors_expose_their_text() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let spans = [crate::core::node::RichSpan {
            text: "abc".into(),
            ..Default::default()
        }];
        let rich = tree.add_rich_text(Some(root), &spans);
        assert_eq!(node_text(tree.get(rich)), "abc");
        assert_eq!(text_len(tree.get(rich)), 3);
        let drawing = crate::core::vector::Drawing {
            width: 10.0,
            ascent: 4.0,
            descent: 1.0,
            ..Default::default()
        };
        let mut vector = tree.add_vector(Some(root), &drawing);
        if let crate::core::node::NodeData::Vector(data) = &mut tree.get_mut(vector).data {
            data.copy = "x^2".into();
        }
        assert!(is_text(tree.get(vector)));
        assert_eq!(node_text(tree.get(vector)), "x^2");
        let _ = vector;
        vector = rich;
        assert!(is_text(tree.get(vector)));
    }
}
