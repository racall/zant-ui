//! Port of `src/core/tree.zig` — WP-1.3: hit testing (innermost interactive
//! node, hover boxes and wheel targets).
#![forbid(unsafe_code)]

use crate::core::node::{NodeData, NodeId};
use crate::core::tree::Tree;

impl Tree {
    pub fn hit_test(&self, x: f32, y: f32) -> Option<NodeId> {
        self.hit_test_node(self.root, x, y)
    }

    fn hit_test_node(&self, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
        let node = &self.nodes[id as usize];
        if !node.in_flow() {
            return None;
        }
        if !node.bounds.contains(x, y) {
            return None;
        }
        let mut i = node.children.len();
        while i > 0 {
            i -= 1;
            if let Some(hit) = self.hit_test_node(node.children[i], x, y) {
                return Some(hit);
            }
        }
        if node.is_interactive() || node.visual.block_pointer {
            return Some(id);
        }
        None
    }

    /// Innermost container with a hover look under `(x, y)`.
    pub fn hover_box_at(&self, x: f32, y: f32) -> Option<NodeId> {
        self.hover_box_node(self.root, x, y)
    }

    fn hover_box_node(&self, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
        let node = &self.nodes[id as usize];
        if !node.in_flow() || !node.bounds.contains(x, y) {
            return None;
        }
        let mut i = node.children.len();
        while i > 0 {
            i -= 1;
            if let Some(hit) = self.hover_box_node(node.children[i], x, y) {
                return Some(hit);
            }
        }
        match &node.data {
            NodeData::Container(data) if data.hover_visual.is_some() => Some(id),
            _ => None,
        }
    }

    /// Innermost node under `(x, y)` that takes wheel input: a scroll
    /// container with overflow or a pointer area with `capture_scroll`.
    pub fn wheel_target_at(&self, x: f32, y: f32) -> Option<NodeId> {
        self.wheel_target_node(self.root, x, y)
    }

    fn wheel_target_node(&self, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
        let node = &self.nodes[id as usize];
        if !node.in_flow() || !node.bounds.contains(x, y) {
            return None;
        }
        let mut i = node.children.len();
        while i > 0 {
            i -= 1;
            if let Some(hit) = self.wheel_target_node(node.children[i], x, y) {
                return Some(hit);
            }
        }
        if let NodeData::Container(data) = &node.data {
            if data
                .pointer
                .as_ref()
                .is_some_and(|area| area.capture_scroll)
            {
                return Some(id);
            }
            if node.style.scroll && data.content_height > node.bounds.height + 0.5 {
                return Some(id);
            }
        }
        None
    }
}
