//! Port of `src/ui_runtime.zig:2188-2212` + `reportSelection`: focus moves and
//! the input selection notification.
//!
//! The Tab order itself lives in the retained tree (`Tree::tab_focus`,
//! WP-1.3); this module only coordinates it with IME composition, the select
//! popup and the `on_selection` callback.
#![forbid(unsafe_code)]

use crate::core::node::{NodeData, NodeId};

use super::Runtime;

impl Runtime {
    /// Zig `focusNode`: cancels the IME composition of the node that loses the
    /// focus, moves the focus and closes a select popup that is no longer
    /// focused.
    pub(crate) fn focus_node(&mut self, id: Option<NodeId>) {
        if let Some(old) = self.tree.focused
            && Some(old) != id
        {
            self.cancel_composition_for(old);
        }
        self.tree.set_focus(id);
        self.close_select_unless_focused();
    }

    /// Focus moved away from an open select: close its popup so it cannot
    /// swallow the next click (Zig `closeSelectUnlessFocused`).
    pub(crate) fn close_select_unless_focused(&mut self) {
        let Some(select_id) = self.open_select else {
            return;
        };
        if self.tree.focused != Some(select_id) {
            self.close_select();
        }
    }

    /// Cancels a pending IME composition of `id` when it is an input.
    pub(crate) fn cancel_composition_for(&mut self, id: NodeId) {
        let has_composition = {
            let node = self.tree.get(id);
            matches!(&node.data, NodeData::Input(data) if data.has_composition)
        };
        if has_composition {
            self.tree.input_cancel_composition(id);
        }
    }

    /// Cancels the focused input's composition (Tab, window blur).
    pub(crate) fn cancel_focused_composition(&mut self) {
        if let Some(id) = self.tree.focused {
            self.cancel_composition_for(id);
        }
    }

    /// Zig `focusedHasComposition`: the app's `on_key` hook is skipped while an
    /// input method is composing.
    pub(crate) fn focused_has_composition(&self) -> bool {
        let Some(id) = self.tree.focused else {
            return false;
        };
        matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition)
    }

    /// Zig `reportSelection`: notifies the focused input's `on_selection` when
    /// its anchor / cursor moved.
    pub(crate) fn report_selection(&mut self) {
        let Some(id) = self.tree.focused else {
            return;
        };
        let (action, anchor, cursor) = {
            let node = self.tree.get(id);
            let NodeData::Input(data) = &node.data else {
                return;
            };
            let Some(action) = data.selection_action.clone() else {
                return;
            };
            if data.reported_selection[0] == data.anchor
                && data.reported_selection[1] == data.cursor
            {
                return;
            }
            (action, data.anchor, data.cursor)
        };
        if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
            data.reported_selection = [anchor, cursor];
        }
        let mut cx = super::action_cx(&mut self.app);
        action.call(&mut cx, anchor, cursor);
        self.needs_build = true;
    }
}
