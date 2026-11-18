//! Enter / exit transitions, reviving exiting nodes and forgetting removed subtrees.

use super::*;

impl Runtime {
    /// Starts a freshly mounted node's enter transition.
    pub(super) fn begin_enter(&mut self, id: NodeId) {
        let (presence, motion_on) = {
            let node = self.tree.get(id);
            (node.motion.presence, motion::enabled())
        };
        if presence == motion::Presence::None || !motion_on {
            return;
        }
        let now = self.now_ms;
        let Some(anim) = self.tree.ensure_anim(id) else {
            return;
        };
        anim.presence = motion::Animated::start(0.0, 1.0, now, motion::presence_enter(presence));
        anim.fx = motion::presence_fx(presence, 0.0);
        self.tree.poke(id);
    }

    /// Starts the exit transition: the node stops taking input right away and
    /// is freed by `tick` when the transition ends.
    pub(super) fn begin_exit(&mut self, id: NodeId) {
        self.tree.get_mut(id).exiting = true;
        self.forget_subtree(id);
        let presence = self.tree.get(id).motion.presence;
        let now = self.now_ms;
        let Some(anim) = self.tree.ensure_anim(id) else {
            self.remove_subtree(id);
            return;
        };
        anim.presence
            .retarget(0.0, now, motion::presence_exit(presence));
        self.tree.poke(id);
    }

    /// A node re-added while its exit transition ran plays the enter
    /// transition again.
    pub(super) fn revive_node(&mut self, id: NodeId) {
        self.tree.get_mut(id).exiting = false;
        let presence = self.tree.get(id).motion.presence;
        let now = self.now_ms;
        if let Some(anim) = self.tree.get_mut(id).anim.as_mut() {
            anim.presence
                .retarget(1.0, now, motion::presence_enter(presence));
        }
        self.tree.poke(id);
    }

    /// Clears every runtime reference to `id` (hover, press, drag, focus,
    /// tooltip, open select, link press, text selection). Any new field that
    /// keeps a `NodeId` across frames must be cleared here too, because ids are
    /// recycled.
    pub(crate) fn forget_node(&mut self, id: NodeId) {
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        if self.pressed_button == Some(id) {
            self.pressed_button = None;
        }
        if self.pressed_toggle == Some(id) {
            self.pressed_toggle = None;
        }
        if self.drag_input == Some(id) {
            self.drag_input = None;
        }
        if self.pressed_link == Some(id) {
            self.pressed_link = None;
        }
        if self.pointer_capture == Some(id) {
            self.pointer_capture = None;
        }
        if self.pointer_hover_area == Some(id) {
            self.pointer_hover_area = None;
        }
        if self.hover_box == Some(id) {
            self.hover_box = None;
        }
        if self.sel_root == Some(id)
            || self.sel_anchor.is_some_and(|point| point.node == id)
            || self.sel_focus.is_some_and(|point| point.node == id)
        {
            self.clear_text_selection();
        }
        if self.open_select == Some(id) {
            self.open_select = None;
        }
        if self.tooltip_target == Some(id) {
            self.clear_tooltip();
        }
        if self.popup_select == Some(id) {
            self.popup_select = None;
        }
        if self.tree.focused == Some(id) {
            self.cancel_composition_for(id);
            self.tree.set_focus(None);
        }
    }

    /// Clears the runtime references of the whole subtree.
    pub(crate) fn forget_subtree(&mut self, id: NodeId) {
        if !self.tree.get(id).alive {
            return;
        }
        let children = self.tree.get(id).children.clone();
        for child in children {
            self.forget_subtree(child);
        }
        self.forget_node(id);
    }

    /// Removes a node that left the UI: clears references, frees the subtree.
    pub(crate) fn remove_subtree(&mut self, id: NodeId) {
        self.forget_subtree(id);
        self.tree.free_subtree(id);
    }
}
