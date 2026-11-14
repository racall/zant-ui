//! Port of `src/ui_runtime.zig:384-621`: reconciling the declarative tree into
//! the retained tree.
//!
//! Matching rules (unchanged from Zig): keyed specs only match a node with the
//! same key and kind; unkeyed specs match the next unused unkeyed node in
//! order (nodes playing an exit transition are never reused); unmatched live
//! nodes are freed, or play their presence exit transition first and are freed
//! by the frame tick when it ends.
//!
//! New nodes are created here as well (`createNode`), including the
//! `ui::memo` expansion and the local-state recording that keeps the state of
//! skipped memo subtrees alive.
#![forbid(unsafe_code)]

use crate::core::node::{NodeId, NodeKind};
use crate::motion::motion;
use crate::ui::element::Node;
use crate::ui::memo::resolve_memo;

use super::Runtime;
use super::patch::apply_selection;

/// Zig `kindCompatible`: the retained node must have the same kind as the spec.
fn kind_compatible(node_kind: NodeKind, spec_kind: NodeKind) -> bool {
    node_kind == spec_kind
}

impl Runtime {
    /// Zig `reconcileChildren`: reconciles `specs` into `parent_id`'s children.
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

        for spec in specs {
            let found = self.find_match(&old, &mut used, &mut cursor, spec);
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

    /// Zig's inline match search: the index of the old child `spec` reuses.
    fn find_match(
        &self,
        old: &[NodeId],
        used: &mut [bool],
        cursor: &mut usize,
        spec: &Node,
    ) -> Option<usize> {
        let element = spec.element();
        if let Some(key) = element.key.as_deref() {
            // keyed specs only ever match a node with the same key
            for (index, &id) in old.iter().enumerate() {
                if used[index] {
                    continue;
                }
                let node = self.tree.get(id);
                if let Some(old_key) = node.key.as_deref()
                    && old_key == key
                    && kind_compatible(node.kind(), element.kind)
                {
                    return Some(index);
                }
            }
            return None;
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

    /// Zig `createMemo`: creates the node of a child spec, expanding (and
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

    /// Zig `setMemo`: stores the memo hash and the local-state ids its build
    /// touched.
    fn set_memo(&mut self, id: NodeId, hash: Option<u64>, states: Vec<u64>) {
        let node = self.tree.get_mut(id);
        node.memo_hash = hash;
        node.memo_states = if hash.is_some() { states } else { Vec::new() };
    }

    /// Zig `createNode`: allocates the retained node and fills in its
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
                    data.icon = element.icon;
                    data.icon_size = element.icon_size;
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
                        // Zig called `components/image.zig:setSource`; the
                        // loader arrives with WP-2.2, the source id / handle
                        // are kept so the patch pass stays consistent.
                        data.source_id = element.image_id;
                        data.handle = element.image_handle.clone();
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
        if let Some(key) = element.key.as_deref() {
            self.tree.set_key(id, Some(key));
        }
        if element.kind == NodeKind::Container {
            self.reconcile_children(id, &element.children);
        }
        Some(id)
    }

    /// Starts a freshly mounted node's enter transition (`beginEnter`).
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
    /// is freed by `tick` when the transition ends (`beginExit`).
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
    /// transition again (`reviveNode`).
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
    /// recycled (Zig `forgetNode`).
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

    /// `forgetSubtree`: clears the references of the whole subtree.
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

    /// Removes a node that left the UI: clears references, frees the subtree
    /// (`removeSubtree`).
    pub(crate) fn remove_subtree(&mut self, id: NodeId) {
        self.forget_subtree(id);
        self.tree.free_subtree(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeData;
    use crate::theme::tokens::ant_light;
    use crate::ui::builders::{column, text};
    use crate::ui::options::{BoxOptions, TextOptions};
    use crate::ui::{Node, RunOptions, app};

    struct Keyed {
        names: Vec<&'static str>,
    }

    impl app::App for Keyed {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: self
                    .names
                    .iter()
                    .map(|name| {
                        text(
                            *name,
                            TextOptions {
                                key: Some((*name).into()),
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
                ..Default::default()
            })
        }
    }

    fn runtime_with(names: Vec<&'static str>) -> Runtime {
        Runtime::init(Keyed { names }, RunOptions::default())
    }

    fn texts(runtime: &Runtime) -> Vec<String> {
        runtime
            .tree
            .nodes
            .iter()
            .filter_map(|node| match &node.data {
                NodeData::Text(data) => Some(data.content.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn keyed_children_are_reused_and_unkeyed_ones_follow_order() {
        let mut runtime = runtime_with(vec!["a", "b"]);
        runtime.rebuild_for_test();
        let first_ids: Vec<NodeId> = runtime
            .tree
            .nodes
            .iter()
            .filter(|node| node.alive && node.key.is_some())
            .map(|node| node.id)
            .collect();
        assert_eq!(texts(&runtime), ["a", "b"]);
        // same keys: the retained nodes are reused (same ids), content patched
        runtime.app_state_mut::<Keyed>().names = vec!["a", "b"];
        runtime.rebuild_for_test();
        let second_ids: Vec<NodeId> = runtime
            .tree
            .nodes
            .iter()
            .filter(|node| node.alive && node.key.is_some())
            .map(|node| node.id)
            .collect();
        assert_eq!(first_ids.len(), 2);
        assert_eq!(second_ids, first_ids);
        // reordered keys are matched by key, not position
        runtime.app_state_mut::<Keyed>().names = vec!["b", "a"];
        runtime.rebuild_for_test();
        let reordered: Vec<(String, NodeId)> = runtime
            .tree
            .nodes
            .iter()
            .filter(|node| node.alive && node.key.is_some())
            .map(|node| (node.key.clone().unwrap().to_string(), node.id))
            .collect();
        let ids: std::collections::HashMap<String, NodeId> = reordered.into_iter().collect();
        assert_eq!(ids["a"], first_ids[0]);
        assert_eq!(ids["b"], first_ids[1]);
    }

    #[test]
    fn removed_children_are_freed() {
        let mut runtime = runtime_with(vec!["a", "b", "c"]);
        runtime.rebuild_for_test();
        assert_eq!(texts(&runtime), ["a", "b", "c"]);
        runtime.app_state_mut::<Keyed>().names = vec!["a"];
        runtime.rebuild_for_test();
        assert_eq!(texts(&runtime), ["a"]);
        // freed slots are recycled tombstones
        assert_eq!(runtime.tree.free_ids.len(), 2);
    }

    #[test]
    fn a_non_container_root_keeps_the_previous_tree() {
        struct TextRoot;
        impl app::App for TextRoot {
            fn render(&mut self) -> Node {
                text("not a container", TextOptions::default())
            }
        }
        let mut runtime = Runtime::init(TextRoot, RunOptions::default());
        runtime.rebuild_for_test();
        assert!(!runtime.needs_build, "the build finished");
        assert_eq!(runtime.tree.nodes.len(), 1, "only the root exists");
    }

    #[test]
    fn set_theme_replaces_the_tree_theme_and_selection_colour() {
        let mut runtime = runtime_with(vec!["a"]);
        runtime.rebuild_for_test();
        let dark = crate::theme::tokens::ant_dark();
        runtime.set_theme(dark);
        assert_eq!(runtime.theme, dark);
        assert_eq!(runtime.tree.theme, dark);
        assert_eq!(
            runtime.tree.selection_color,
            crate::theme::tokens::with_alpha(dark.color_primary, 0x4d)
        );
        assert!(runtime.tree.needs_layout());
        // setting the same theme again is a no-op
        runtime.tree.clear_paint();
        runtime.tree.get_mut(runtime.tree.root).dirty.layout = false;
        runtime.set_theme(dark);
        assert!(!runtime.tree.needs_layout());
    }

    #[test]
    fn the_default_tree_root_fills_the_window() {
        let mut runtime = runtime_with(vec![]);
        runtime.rebuild_for_test();
        let root = runtime.tree.get(runtime.tree.root);
        assert_eq!(root.style.width, None);
        assert_eq!(root.style.height, None);
        assert_eq!(root.style.grow, 0.0);
        assert_eq!(root.visual.background, Some(ant_light().color_bg_base));
        assert_eq!(
            runtime.clear_color,
            ant_light().color_bg_base,
            "the frame clears to the app background"
        );
        assert_eq!(runtime.tree.root, 0);
    }
}
