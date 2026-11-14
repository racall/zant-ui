//! Port of `src/core/tree.zig` — WP-1.3: node allocation, builders, tombstones,
//! the key index and dirty-state bookkeeping.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::components::button;
use crate::core::geometry::Rect;
use crate::core::node::{
    ButtonData, ButtonSize, ButtonType, ContainerData, FlexStyle, ImageData, ImageFit, InputData,
    NodeData, NodeId, RichSpan, RichTextData, SelectData, SelectOption, TextData, ToggleData,
    ToggleKind, VectorData,
};
use crate::core::tree::Tree;
use crate::core::vector::Drawing;
use crate::renderer::text;
use crate::ui::actions::{Action, TextAction};

impl Tree {
    // ── allocation ─────────────────────────────────────────────────────────

    /// Allocates a slot, reusing a tombstone when one is waiting.
    pub fn create_node(&mut self, data: NodeData) -> NodeId {
        if let Some(id) = self.free_ids.pop() {
            self.nodes[id as usize] = crate::core::node::RetainedNode::new(id, data);
            return id;
        }
        let id = self.nodes.len() as NodeId;
        self.nodes
            .push(crate::core::node::RetainedNode::new(id, data));
        id
    }

    pub fn add_container(&mut self, parent: Option<NodeId>, style: FlexStyle) -> NodeId {
        let id = self.create_node(NodeData::Container(ContainerData::default()));
        self.get_mut(id).style = style;
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_text(
        &mut self,
        parent: Option<NodeId>,
        content: &str,
        style: text::Style,
    ) -> NodeId {
        let id = self.create_node(NodeData::Text(TextData {
            content: Str::copy(content),
            style,
            ..Default::default()
        }));
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_rich_text(&mut self, parent: Option<NodeId>, spans: &[RichSpan]) -> NodeId {
        let id = self.create_node(NodeData::RichText(RichTextData {
            spans: spans.to_vec(),
            ..Default::default()
        }));
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_vector(&mut self, parent: Option<NodeId>, drawing: &Drawing) -> NodeId {
        let copy = drawing.deep_clone();
        let hash = copy.hash;
        let id = self.create_node(NodeData::Vector(VectorData {
            drawing: Rc::new(copy),
            hash,
            copy: Str::default(),
        }));
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    /// Replaces a vector node's drawing when its content hash changed.
    pub fn vector_set(&mut self, id: NodeId, drawing: &Drawing) {
        let hash = drawing.content_hash();
        let changed = {
            let Some(data) = self.vector_mut(id) else {
                return;
            };
            if hash == data.hash {
                false
            } else {
                let keep = data.copy.clone();
                data.drawing = Rc::new(drawing.deep_clone());
                data.hash = hash;
                data.copy = keep;
                true
            }
        };
        if changed {
            self.mark_dirty(id, true, true);
        }
    }

    fn vector_mut(&mut self, id: NodeId) -> Option<&mut VectorData> {
        match &mut self.nodes[id as usize].data {
            NodeData::Vector(data) => Some(data),
            _ => None,
        }
    }

    pub fn rich_text_set(&mut self, id: NodeId, spans: &[RichSpan]) {
        {
            let Some(data) = self.rich_text_mut(id) else {
                return;
            };
            data.invalidate_layout();
            data.spans = spans.to_vec();
            data.layout_dirty = true;
        }
        self.mark_dirty(id, true, true);
    }

    fn rich_text_mut(&mut self, id: NodeId) -> Option<&mut RichTextData> {
        match &mut self.nodes[id as usize].data {
            NodeData::RichText(data) => Some(data),
            _ => None,
        }
    }

    pub fn add_button(
        &mut self,
        parent: Option<NodeId>,
        label: &str,
        button_type: ButtonType,
        size: ButtonSize,
        action: Option<Action>,
    ) -> NodeId {
        let id = self.create_node(NodeData::Button(ButtonData {
            label: Str::copy(label),
            button_type,
            size,
            font_size: button::font_size(size),
            action,
            ..Default::default()
        }));
        let radius = self.theme.border_radius;
        let control_height = button::control_height(size);
        {
            let node = self.get_mut(id);
            node.focusable = true;
            node.visual.radius = radius;
            node.style.height = Some(control_height);
        }
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_input(
        &mut self,
        parent: Option<NodeId>,
        value: &str,
        placeholder: &str,
        text_action: Option<TextAction>,
    ) -> NodeId {
        let text_color = self.theme.color_text;
        let radius = self.theme.border_radius;
        let mut data = InputData {
            placeholder: Str::copy(placeholder),
            text_action,
            text_color,
            ..Default::default()
        };
        data.text.push_str(value);
        data.cursor = data.text.len();
        data.anchor = data.cursor;
        data.rebuild_display();
        let id = self.create_node(NodeData::Input(data));
        {
            let node = self.get_mut(id);
            node.focusable = true;
            node.visual.radius = radius;
        }
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_toggle(
        &mut self,
        parent: Option<NodeId>,
        label: &str,
        kind: ToggleKind,
        checked: bool,
    ) -> NodeId {
        let radius_sm = self.theme.border_radius_sm;
        let one = crate::motion::motion::Animated::at(1.0);
        let zero = crate::motion::motion::Animated::at(0.0);
        let id = self.create_node(NodeData::Toggle(ToggleData {
            label: Str::copy(label),
            kind,
            checked,
            check: if checked { one } else { zero },
            mark: if checked { one } else { zero },
            check_t: if checked { 1.0 } else { 0.0 },
            mark_t: if checked { 1.0 } else { 0.0 },
            ..Default::default()
        }));
        {
            let node = self.get_mut(id);
            node.focusable = true;
            node.visual.radius = radius_sm;
        }
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    /// Adds an image node. Loading the file happens in `components::image`
    /// (WP-2.2): Zig synchronously loaded it here; the Rust tree stores the
    /// path and lets the component load / decode.
    pub fn add_image(
        &mut self,
        parent: Option<NodeId>,
        path: &str,
        fit: ImageFit,
        radius: f32,
    ) -> NodeId {
        let id = self.create_node(NodeData::Image(ImageData {
            path: Str::copy(path),
            fit,
            radius,
            ..Default::default()
        }));
        self.get_mut(id).visual.radius = radius;
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_select(
        &mut self,
        parent: Option<NodeId>,
        options: &[&str],
        selected: usize,
    ) -> NodeId {
        let options: Vec<SelectOption> = options
            .iter()
            .map(|label| SelectOption {
                label: Str::copy(label),
            })
            .collect();
        let selected = if options.is_empty() {
            0
        } else {
            selected.min(options.len() - 1)
        };
        let radius = self.theme.border_radius;
        let id = self.create_node(NodeData::Select(SelectData {
            options,
            selected,
            ..Default::default()
        }));
        {
            let node = self.get_mut(id);
            node.focusable = true;
            node.visual.radius = radius;
        }
        if let Some(parent) = parent {
            self.add_child(parent, id);
        }
        id
    }

    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        self.get_mut(parent).children.push(child);
        self.get_mut(child).parent = Some(parent);
        self.mark_dirty(parent, true, true);
    }

    pub fn set_tooltip(&mut self, id: NodeId, value: Option<&str>) {
        self.get_mut(id).tooltip = value.map(Str::copy);
    }

    /// Releases `id` and its whole subtree: the slots are recycled. The caller
    /// must already have removed `id` from its parent's `children` and cleared
    /// any runtime references to the subtree.
    pub fn free_subtree(&mut self, id: NodeId) {
        if !self.nodes[id as usize].alive {
            return;
        }
        let children = self.nodes[id as usize].children.clone();
        for child in children {
            self.free_subtree(child);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
        if self.nodes[id as usize].anim_queued
            && let Some(index) = self.animating.iter().position(|&queued| queued == id)
        {
            self.animating.swap_remove(index);
        }
        self.unindex_key(id);
        self.nodes[id as usize] = crate::core::node::RetainedNode::tombstone(id);
        self.free_ids.push(id);
    }

    // ── keys ───────────────────────────────────────────────────────────────

    /// Window-space box of the laid-out node with `key` (`None` when absent).
    pub fn bounds_of_key(&self, key: &str) -> Option<Rect> {
        if let Some(&id) = self.key_index.get(key) {
            let node = &self.nodes[id as usize];
            if node.alive && node.in_flow() && node.key.as_deref() == Some(key) {
                return Some(node.bounds);
            }
        }
        for node in &self.nodes {
            if !node.alive || !node.in_flow() {
                continue;
            }
            let Some(node_key) = node.key.as_deref() else {
                continue;
            };
            if node_key == key {
                return Some(node.bounds);
            }
        }
        None
    }

    pub fn set_key(&mut self, id: NodeId, key: Option<&str>) {
        self.unindex_key(id);
        self.get_mut(id).key = key.map(Str::copy);
        let Some(node_key) = self.get(id).key.as_deref() else {
            return;
        };
        self.key_index.insert(node_key.to_string(), id);
    }

    /// Live, non-exiting node with `key` (index first, scan as fallback).
    pub fn id_of_key(&self, key: &str) -> Option<NodeId> {
        if let Some(&id) = self.key_index.get(key) {
            let node = &self.nodes[id as usize];
            if node.alive && !node.exiting && node.key.as_deref() == Some(key) {
                return Some(id);
            }
        }
        for node in &self.nodes {
            if !node.alive || node.exiting {
                continue;
            }
            if node.key.as_deref() == Some(key) {
                return Some(node.id);
            }
        }
        None
    }

    fn unindex_key(&mut self, id: NodeId) {
        let Some(key) = self.nodes[id as usize].key.as_deref() else {
            return;
        };
        if self.key_index.get(key) == Some(&id) {
            self.key_index.remove(key);
        }
    }

    // ── dirty state ────────────────────────────────────────────────────────

    /// Repaints `id` and schedules it for animation (its state changed, so a
    /// transition may have to start). Cheap when nothing ends up animating.
    pub fn poke(&mut self, id: NodeId) {
        if !self.nodes[id as usize].alive {
            return;
        }
        self.mark_dirty(id, false, true);
        if self.nodes[id as usize].anim_queued {
            return;
        }
        self.animating.push(id);
        self.nodes[id as usize].anim_queued = true;
    }

    /// Number of live (non-recycled) nodes.
    pub fn live_count(&self) -> usize {
        self.nodes.len() - self.free_ids.len()
    }

    pub fn mark_dirty(&mut self, id: NodeId, needs_layout: bool, needs_paint: bool) {
        let mut current = Some(id);
        while let Some(current_id) = current {
            let node = &mut self.nodes[current_id as usize];
            node.mark(needs_layout, needs_paint);
            current = node.parent;
        }
    }

    pub fn needs_layout(&self) -> bool {
        self.get(self.root).dirty.layout
    }

    pub fn needs_paint(&self) -> bool {
        self.get(self.root).dirty.paint
    }

    pub fn mark_layout_dirty(&mut self) {
        self.mark_dirty(self.root, true, true);
    }

    pub fn mark_paint_dirty(&mut self) {
        self.mark_dirty(self.root, false, true);
    }

    pub fn clear_paint(&mut self) {
        for node in &mut self.nodes {
            node.dirty.paint = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::geometry::Size;
    use crate::core::node::{FlexStyle, NodeData};
    use crate::core::tree::Tree;
    use crate::theme::tokens::ant_light;

    #[test]
    fn freed_slots_are_tombstoned_and_reused() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let first = tree.add_text(Some(root), "a", Default::default());
        let second = tree.add_text(Some(root), "b", Default::default());
        let live = tree.live_count();
        assert_eq!(live, 3);
        tree.free_subtree(first);
        assert!(!tree.get(first).alive, "freed slot is a tombstone");
        assert_eq!(tree.live_count(), live - 1);
        let reused = tree.add_text(Some(root), "c", Default::default());
        assert_eq!(reused, first, "the freed id is recycled");
        assert!(tree.get(reused).alive);
        assert_eq!(tree.get(reused).id, reused);
        assert!(matches!(tree.get(reused).data, NodeData::Text(_)));
        assert!(tree.get(second).alive);
    }

    #[test]
    fn key_index_resolves_ids_and_bounds() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let id = tree.add_container(
            Some(root),
            FlexStyle {
                width: Some(80.0),
                height: Some(20.0),
                ..Default::default()
            },
        );
        tree.set_key(id, Some("panel"));
        assert_eq!(tree.id_of_key("panel"), Some(id));
        assert_eq!(tree.id_of_key("missing"), None);
        tree.layout(Size {
            width: 200.0,
            height: 100.0,
        });
        let bounds = tree.bounds_of_key("panel").expect("keyed bounds");
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (0.0, 0.0, 80.0, 20.0)
        );
        tree.set_key(id, Some("renamed"));
        assert_eq!(tree.id_of_key("panel"), None);
        assert_eq!(tree.id_of_key("renamed"), Some(id));
        tree.set_key(id, None);
        assert_eq!(tree.id_of_key("renamed"), None);
        // freeing removes the index entry
        tree.set_key(id, Some("gone"));
        tree.free_subtree(id);
        assert_eq!(tree.id_of_key("gone"), None);
    }

    #[test]
    fn dirty_flags_propagate_to_the_root() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let container = tree.add_container(Some(root), FlexStyle::default());
        let text = tree.add_text(Some(container), "x", Default::default());
        tree.clear_paint();
        assert!(!tree.needs_paint() || tree.needs_layout());
        tree.mark_dirty(text, false, true);
        assert!(tree.needs_paint());
        tree.mark_dirty(text, true, true);
        assert!(tree.needs_layout());
        tree.clear_paint();
        assert!(!tree.get(root).dirty.paint);
        let extra = tree.add_container(None, FlexStyle::default());
        tree.clear_paint();
        tree.get_mut(root).dirty.layout = false;
        tree.add_child(root, extra);
        assert!(tree.needs_layout(), "adding a child marks layout dirty");
        // detach the extra node again so the tree stays consistent
        tree.free_subtree(extra);
    }

    #[test]
    fn poke_queues_animation_once() {
        let mut tree = Tree::init(ant_light());
        let id = tree.add_text(Some(tree.root), "x", Default::default());
        assert!(tree.animating.is_empty());
        tree.poke(id);
        assert_eq!(tree.animating, vec![id]);
        assert!(tree.get(id).anim_queued);
        tree.poke(id);
        assert_eq!(tree.animating.len(), 1, "already queued");
        tree.free_subtree(id);
        assert!(tree.animating.is_empty(), "freed nodes leave the queue");
    }
}
