//! Text and link hit testing, input caret placement, and selection helpers.

use super::Runtime;
use crate::core::node::{NodeData, NodeId, RetainedNode};

impl Runtime {
    /// The link of the rich-text span under a window point.
    pub(crate) fn link_at(&mut self, id: NodeId, x: f32, y: f32) -> Option<String> {
        let bounds = self.tree.get(id).bounds;
        let NodeData::RichText(data) = &mut self.tree.get_mut(id).data else {
            return None;
        };
        data.link_action.as_ref()?;
        let _ = data.ensure_layout(bounds.width)?;
        let byte = {
            let layout = data.layout.as_ref()?;
            layout.index_at(x - bounds.x, y - bounds.y)
        };
        // indexAt returns the nearest caret; probe the glyph on both sides.
        if let Some(link) = data.link_at_byte(byte) {
            return Some(link.to_string());
        }
        if byte > 0 {
            return data.link_at_byte(byte - 1).map(str::to_string);
        }
        None
    }

    /// Places the input caret at a window point (a click
    /// or a drag inside the input).
    pub(super) fn set_input_cursor(&mut self, id: NodeId, x: f32, y: f32, extend: bool) {
        let bounds = self.tree.get(id).bounds;
        let NodeData::Input(data) = &mut self.tree.get_mut(id).data else {
            return;
        };
        if data.has_composition {
            return;
        }
        let multiline = data.multiline;
        let scroll_x = data.scroll_x;
        let scroll_y = data.scroll_y;
        let text_len = data.text.len();
        let text_left = crate::components::input::text_left(data, bounds);
        if !crate::components::input::ensure_layout(data) {
            return;
        }
        let Some(layout) = data.layout.as_ref() else {
            return;
        };
        let origin_x = text_left - scroll_x;
        let origin_y = if multiline {
            bounds.y + 8.0 - scroll_y
        } else {
            bounds.y + (bounds.height - layout.metrics().height) * 0.5
        };
        let byte = layout.index_at(x - origin_x, y - origin_y);
        data.cursor = byte.min(text_len);
        if !extend {
            data.anchor = data.cursor;
        }
        self.tree.mark_dirty(id, false, true);
    }

    /// A text node of a selectable container under (x, y) (I-beam cursor).
    pub(super) fn selectable_text_at(&self, x: f32, y: f32) -> bool {
        let Some(root) = crate::core::text_select::root_at(&self.tree, x, y) else {
            return false;
        };
        self.text_node_under(self.tree.get(root), x, y)
    }

    fn text_node_under(&self, node: &RetainedNode, x: f32, y: f32) -> bool {
        if !node.in_flow() || !node.bounds.contains(x, y) {
            return false;
        }
        if crate::core::text_select::is_text(node) {
            return true;
        }
        node.children
            .iter()
            .any(|child| self.text_node_under(self.tree.get(*child), x, y))
    }

    /// Whether `ancestor` is `id` or one of its ancestors.
    pub(crate) fn is_ancestor(&self, ancestor: NodeId, id: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(node) = current {
            if node == ancestor {
                return true;
            }
            current = self.tree.get(node).parent;
        }
        false
    }
}
