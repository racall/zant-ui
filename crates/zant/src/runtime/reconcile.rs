//! Reconciling the declarative tree into the retained tree.
//!
//! Matching rules: keyed specs only match a node with the
//! same key and kind; unkeyed specs match the next unused unkeyed node in
//! order (nodes playing an exit transition are never reused); unmatched live
//! nodes are freed, or play their presence exit transition first and are freed
//! by the frame tick when it ends.
//!
//! New nodes are created here as well (`create_node`), including the
//! `ui::memo` expansion and the local-state recording that keeps the state of
//! skipped memo subtrees alive.
#![forbid(unsafe_code)]

use std::collections::HashMap;

use crate::Str;
use crate::core::node::{NodeId, NodeKind};
use crate::motion::motion;
use crate::ui::element::Node;
use crate::ui::memo::resolve_memo;

use super::Runtime;
use super::patch::apply_selection;

/// Finds the old child a keyed spec matches without scanning every old child
/// per spec (that was quadratic in long keyed lists).
///
/// Keyed children usually keep their order, so the old child right after the
/// previous keyed match is tried first; the first miss builds a key → index
/// map of the old children (the "map of remaining children" of React / Vue /
/// Inferno). Duplicate keys or a kind mismatch fall back to the full scan.
#[derive(Default)]
struct KeyedLookup {
    next: usize,
    by_key: Option<HashMap<Str, usize>>,
}

/// The retained node must have the same kind as the spec.
fn kind_compatible(node_kind: NodeKind, spec_kind: NodeKind) -> bool {
    node_kind == spec_kind
}

impl Runtime {
    /// Reconciles `specs` into `parent_id`'s children.
    /// Returns true when the layout has to run again.
    pub(crate) fn reconcile_children(&mut self, parent_id: NodeId, specs: &[Node]) -> bool {
        let old: Vec<NodeId> = self.tree.get(parent_id).children.clone();
        let old_len = old.len();
        let mut used = vec![false; old_len];
        let mut new_children: Vec<NodeId> = Vec::with_capacity(specs.len());
        let mut changed_layout = false;
        let mut changed_children = false;
        // walks `old` for unkeyed specs
        let mut cursor = 0usize;
        let mut keyed = KeyedLookup::default();

        for spec in specs {
            let found = self.find_match(&old, &used, &mut cursor, &mut keyed, spec);
            let Some(index) = found else {
                if let Some(id) = self.create_memo(spec.clone()) {
                    self.tree.get_mut(id).parent = Some(parent_id);
                    self.begin_enter(id);
                    new_children.push(id);
                    changed_layout = true;
                    changed_children = true;
                }
                continue;
            };

            used[index] = true;
            let id = old[index];
            let exiting = self.tree.get(id).exiting;
            if exiting {
                // re-added while leaving: play the enter transition again
                self.revive_node(id);
                changed_layout = true;
                changed_children = true;
            }
            if let Some(hash) = spec.element().memo_hash {
                let node_hash = self.tree.get(id).memo_hash;
                if node_hash.is_some() && node_hash == Some(hash) {
                    let states = self.tree.get(id).memo_states.clone();
                    self.local_states.borrow_mut().touch(&states);
                    new_children.push(id);
                    continue;
                }
            }
            if spec.element().memo_build.is_none() {
                if self.patch_node(id, spec.clone()) {
                    changed_layout = true;
                }
                self.set_memo(id, None, Vec::new());
            } else {
                self.local_states.borrow_mut().record();
                let real = resolve_memo(spec.clone());
                if self.patch_node(id, real) {
                    changed_layout = true;
                }
                let states = self.local_states.borrow_mut().end_record();
                self.set_memo(id, spec.element().memo_hash, states);
            }
            new_children.push(id);
        }

        // unmatched nodes leave: with a presence transition they stay (out of
        // flow) until it ends, anchored after their previous live sibling
        let mut leaving: Vec<(NodeId, Option<NodeId>)> = Vec::new(); // (id, anchor)
        let mut last_live: Option<NodeId> = None;
        for (index, &id) in old.iter().enumerate() {
            if used[index] {
                last_live = Some(id);
                continue;
            }
            let (exiting, presence, motion_on) = {
                let node = self.tree.get(id);
                (node.exiting, node.motion.presence, motion::enabled())
            };
            if !exiting {
                if !motion_on || presence == motion::Presence::None {
                    self.remove_subtree(id);
                    changed_layout = true;
                    changed_children = true;
                    continue;
                }
                self.begin_exit(id);
                changed_layout = true;
                changed_children = true;
            }
            leaving.push((id, last_live));
        }
        if !leaving.is_empty() {
            let mut merged: Vec<NodeId> = Vec::new();
            for (id, anchor) in &leaving {
                if anchor.is_none() {
                    merged.push(*id);
                }
            }
            for &id in &new_children {
                merged.push(id);
                for (leaving_id, anchor) in &leaving {
                    if *anchor == Some(id) {
                        merged.push(*leaving_id);
                    }
                }
            }
            // anchors that were removed themselves: keep those nodes at the end
            for (id, _) in &leaving {
                if !merged.contains(id) {
                    merged.push(*id);
                }
            }
            new_children = merged;
        }

        if old_len != new_children.len() {
            changed_children = true;
        } else if old != new_children {
            changed_children = true;
            changed_layout = true;
        }

        if changed_children {
            self.tree.get_mut(parent_id).children = new_children.clone();
            for &id in &new_children {
                self.tree.get_mut(id).parent = Some(parent_id);
            }
            self.tree.mark_dirty(parent_id, true, true);
        }
        if changed_layout {
            self.tree.mark_dirty(parent_id, true, true);
        }
        changed_layout
    }

    /// The match search: the index of the old child `spec` reuses.
    fn find_match(
        &self,
        old: &[NodeId],
        used: &[bool],
        cursor: &mut usize,
        keyed: &mut KeyedLookup,
        spec: &Node,
    ) -> Option<usize> {
        let element = spec.element();
        if let Some(key) = element.key.as_deref() {
            // keyed specs only ever match a node with the same key
            let found = self.find_keyed(old, used, keyed, key, element.kind);
            if let Some(index) = found {
                keyed.next = index + 1;
            }
            return found;
        }
        // unkeyed specs match the next unused unkeyed node in order
        // (nodes playing an exit transition are never reused)
        while *cursor < old.len() {
            let index = *cursor;
            let candidate = self.tree.get(old[index]);
            if used[index] || candidate.key.is_some() || candidate.exiting {
                *cursor += 1;
                continue;
            }
            *cursor += 1;
            if kind_compatible(candidate.kind(), element.kind) {
                return Some(index);
            }
            return None;
        }
        None
    }

    fn find_keyed(
        &self,
        old: &[NodeId],
        used: &[bool],
        keyed: &mut KeyedLookup,
        key: &str,
        kind: NodeKind,
    ) -> Option<usize> {
        let matches = |index: usize| {
            let node = self.tree.get(old[index]);
            !used[index] && node.key.as_deref() == Some(key) && kind_compatible(node.kind(), kind)
        };
        // in order: the old child after the previous keyed match
        if keyed.next < old.len() && matches(keyed.next) {
            return Some(keyed.next);
        }
        let by_key = keyed.by_key.get_or_insert_with(|| {
            let mut map = HashMap::with_capacity(old.len());
            for (index, &id) in old.iter().enumerate() {
                if let Some(old_key) = &self.tree.get(id).key {
                    map.entry(old_key.clone()).or_insert(index);
                }
            }
            map
        });
        match by_key.get(key) {
            Some(&index) if matches(index) => Some(index),
            // a duplicate key already used, or another kind: scan
            Some(_) => (0..old.len()).find(|&index| matches(index)),
            None => None,
        }
    }

    /// Creates the node of a child spec, expanding (and
    /// recording the local-state ids of) a memo.
    fn create_memo(&mut self, spec: Node) -> Option<NodeId> {
        if spec.element().memo_build.is_none() {
            return self.create_node(spec);
        }
        self.local_states.borrow_mut().record();
        let id = self.create_node(resolve_memo(spec.clone()));
        let states = self.local_states.borrow_mut().end_record();
        let id = id?;
        self.set_memo(id, spec.element().memo_hash, states);
        Some(id)
    }

    /// Stores the memo hash and the local-state ids its build
    /// touched.
    fn set_memo(&mut self, id: NodeId, hash: Option<u64>, states: Vec<u64>) {
        let node = self.tree.get_mut(id);
        node.memo_hash = hash;
        node.memo_states = if hash.is_some() { states } else { Vec::new() };
    }

    /// Allocates the retained node and fills in its
    /// kind-specific data.
    fn create_node(&mut self, spec: Node) -> Option<NodeId> {
        let element = spec.element();
        let id = match element.kind {
            NodeKind::Container => self.tree.add_container(None, element.layout),
            NodeKind::Text => self
                .tree
                .add_text(None, &element.content, element.text_style),
            NodeKind::Button => self.tree.add_button(
                None,
                &element.content,
                element.button_type,
                element.button_size,
                None,
            ),
            NodeKind::Input => {
                self.tree
                    .add_input(None, &element.content, &element.placeholder, None)
            }
            NodeKind::Toggle => {
                self.tree
                    .add_toggle(None, &element.content, element.toggle_kind, element.checked)
            }
            NodeKind::Image => self.tree.add_image(
                None,
                &element.content,
                element.image_fit,
                element.visual.radius,
            ),
            NodeKind::Select => {
                let options: Vec<&str> =
                    element.options.iter().map(|label| label.as_str()).collect();
                self.tree.add_select(None, &options, element.selected)
            }
            NodeKind::RichText => self.tree.add_rich_text(None, &element.rich_spans),
            NodeKind::Vector => {
                let Some(drawing) = element.drawing.as_deref() else {
                    eprintln!("Zant UI: a vector node needs a drawing");
                    return None;
                };
                self.tree.add_vector(None, drawing)
            }
            NodeKind::Root => {
                eprintln!("Zant UI: a Root node is not a valid child");
                return None;
            }
        };
        self.apply_common(id, element);
        if element.auto_focus {
            self.focus_memory.auto_focus = Some(id);
        }
        {
            let node = self.tree.get_mut(id);
            match &mut node.data {
                crate::core::node::NodeData::Container(data) => {
                    data.pointer = element.pointer.clone();
                    data.key_action = element.key_action.clone();
                    data.cursor = element.cursor;
                    data.resize_action = element.on_resize.clone();
                    data.hover_visual = element.hover_visual;
                }
                crate::core::node::NodeData::Button(data) => {
                    data.action = element.on_click.clone();
                    data.index_action = element.index_action.clone();
                    data.index = element.index;
                    data.font_size = element.text_style.font_size;
                    data.text_align = element.text_style.text_align;
                    data.label_padding = element.label_padding;
                    data.text_color = element.text_color;
                    data.hover_text_color = element.hover_text_color;
                    data.icon = element.icon;
                    data.icon_size = element.icon_size;
                    data.danger = element.danger;
                    data.loading = element.loading;
                    data.invalidate_layout();
                }
                crate::core::node::NodeData::Input(data) => {
                    data.text_action = element.on_text.clone();
                    data.submit_action = element.on_click.clone();
                    data.undo_action = element.on_undo.clone();
                    data.redo_action = element.on_redo.clone();
                    data.read_only = element.read_only;
                    data.password = element.password;
                    data.multiline = element.multiline;
                    data.focus_ring = element.focus_ring;
                    data.font_size = element.text_style.font_size;
                    data.line_height = element.text_style.line_height;
                    data.selection_action = element.on_selection.clone();
                    data.key_action = element.on_key.clone();
                    let _ = apply_selection(data, element.selection);
                    data.content_max_width = element.content_max_width;
                    data.rebuild_display();
                }
                crate::core::node::NodeData::RichText(data) => {
                    data.line_height = element.rich_line_height;
                    data.link_action = element.on_link.clone();
                }
                crate::core::node::NodeData::Toggle(data) => {
                    data.action = element.on_bool.clone();
                    data.index_action = element.index_action.clone();
                    data.index = element.index;
                    data.font_size = element.text_style.font_size;
                    data.checked = element.checked;
                    let value: f32 = if element.checked { 1.0 } else { 0.0 };
                    data.check = crate::motion::motion::Animated::at(value);
                    data.mark = crate::motion::motion::Animated::at(value);
                    data.check_t = value;
                    data.mark_t = value;
                }
                crate::core::node::NodeData::Vector(data) => {
                    data.copy = crate::Str::copy(&element.content);
                }
                crate::core::node::NodeData::Image(data) => {
                    data.fit = element.image_fit;
                    if element.image_id != 0 {
                        crate::components::image::set_source(
                            data,
                            element.image_handle.as_ref(),
                            element.image_id,
                        );
                    } else {
                        // the tree does no I/O, so the runtime loads the
                        // file here (outside `render`).
                        crate::components::image::load(data);
                    }
                }
                crate::core::node::NodeData::Select(data) => {
                    data.action = element.index_action.clone();
                    data.selected = element.selected;
                }
                crate::core::node::NodeData::Text(_) => {}
                crate::core::node::NodeData::Root => {}
            }
        }
        // a button mounted loading starts its spinner frames
        if element.kind == NodeKind::Button && element.loading {
            self.tree.poke(id);
        }
        if let Some(key) = element.key.as_deref() {
            self.tree.set_key(id, Some(key));
        }
        if element.kind == NodeKind::Container {
            self.reconcile_children(id, &element.children);
        }
        Some(id)
    }

    /// Starts a freshly mounted node's enter transition.
    fn begin_enter(&mut self, id: NodeId) {
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
    fn begin_exit(&mut self, id: NodeId) {
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
    fn revive_node(&mut self, id: NodeId) {
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

#[cfg(test)]
mod tests;
