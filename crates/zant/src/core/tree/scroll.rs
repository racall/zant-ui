//! Scrolling (wheel offsets and the
//! scroll container under the pointer).
#![forbid(unsafe_code)]

use crate::core::layout;
use crate::core::node::{NodeData, NodeId};
use crate::core::tree::Tree;

impl Tree {
    /// Scrolls the innermost scroll container under `(x, y)` by `dy` points
    /// (positive dy moves content down, like AppKit's scrollingDeltaY).
    /// Returns true when an offset changed.
    pub fn scroll_at(&mut self, x: f32, y: f32, dy: f32) -> bool {
        let Some(id) = self.scroll_container_at(self.root, x, y) else {
            return false;
        };
        // the user's own scrolling wins over a running scrollIntoView
        if let Some(data) = self.nodes[id as usize].container_mut() {
            data.scroll_animating = false;
        }
        self.scroll_by(id, -dy)
    }

    /// Adds `delta` to a scroll container's offset (clamped). Returns true on change.
    pub fn scroll_by(&mut self, id: NodeId, delta: f32) -> bool {
        let viewport = self.nodes[id as usize].bounds.height;
        let Some(data) = self.nodes[id as usize].container_mut() else {
            return false;
        };
        let max = data.max_scroll(viewport);
        let next = (data.scroll_offset + delta).max(0.0).min(max);
        if next == data.scroll_offset {
            return false;
        }
        let shift = data.scroll_offset - next;
        data.scroll_offset = next;
        let children = self.nodes[id as usize].children.clone();
        for child in children {
            layout::translate_subtree_y(&mut self.nodes, child, shift);
        }
        self.mark_dirty(id, false, true);
        true
    }

    fn scroll_container_at(&self, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
        let node = &self.nodes[id as usize];
        if !node.in_flow() || !node.bounds.contains(x, y) {
            return None;
        }
        let mut i = node.children.len();
        while i > 0 {
            i -= 1;
            if let Some(hit) = self.scroll_container_at(node.children[i], x, y) {
                return Some(hit);
            }
        }
        if node.style.scroll
            && let NodeData::Container(data) = &node.data
            && data.content_height > node.bounds.height + 0.5
        {
            return Some(id);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::core::geometry::Size;
    use crate::core::node::FlexStyle;
    use crate::core::tree::Tree;
    use crate::theme::tokens::ant_light;

    #[test]
    fn scroll_by_clamps_and_translates_children() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let scroll = tree.add_container(
            Some(root),
            FlexStyle {
                width: Some(100.0),
                height: Some(50.0),
                scroll: true,
                direction: crate::layout::flex::Direction::Column,
                ..Default::default()
            },
        );
        let child = tree.add_container(
            Some(scroll),
            FlexStyle {
                width: Some(100.0),
                height: Some(200.0),
                ..Default::default()
            },
        );
        tree.layout(Size {
            width: 100.0,
            height: 50.0,
        });
        let content_height = tree.get(scroll).container().unwrap().content_height;
        assert_eq!(content_height, 200.0);
        assert!(tree.scroll_by(scroll, 30.0));
        assert_eq!(tree.get(scroll).container().unwrap().scroll_offset, 30.0);
        assert!((tree.get(child).bounds.y - (0.0 - 30.0)).abs() < 0.01);
        assert!(tree.scroll_by(scroll, -100.0));
        assert_eq!(tree.get(scroll).container().unwrap().scroll_offset, 0.0);
        assert!((tree.get(child).bounds.y - 0.0).abs() < 0.01);
        // clamped at zero: nothing changes
        assert!(!tree.scroll_by(scroll, -5.0));
        // huge delta clamps to the maximum
        assert!(tree.scroll_by(scroll, 1000.0));
        let offset = tree.get(scroll).container().unwrap().scroll_offset;
        assert!(offset > 0.0);
        assert!(offset <= content_height - 50.0);
    }

    #[test]
    fn scroll_at_finds_the_innermost_overflow_container() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let scroll = tree.add_container(
            Some(root),
            FlexStyle {
                width: Some(100.0),
                height: Some(50.0),
                scroll: true,
                direction: crate::layout::flex::Direction::Column,
                ..Default::default()
            },
        );
        tree.add_container(
            Some(scroll),
            FlexStyle {
                width: Some(100.0),
                height: Some(200.0),
                ..Default::default()
            },
        );
        tree.layout(Size {
            width: 100.0,
            height: 50.0,
        });
        assert!(tree.scroll_at(10.0, 10.0, -20.0));
        assert_eq!(tree.get(scroll).container().unwrap().scroll_offset, 20.0);
        assert!(
            !tree.scroll_at(500.0, 500.0, -20.0),
            "outside the container"
        );
    }
}
