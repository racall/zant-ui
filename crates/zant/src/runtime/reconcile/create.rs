//! Creating nodes and memo nodes from elements.

use super::*;

impl Runtime {
    /// Creates the node of a child spec, expanding (and
    /// recording the local-state ids of) a memo.
    pub(super) fn create_memo(&mut self, spec: Node) -> Option<NodeId> {
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
    pub(super) fn set_memo(&mut self, id: NodeId, hash: Option<u64>, states: Vec<u64>) {
        let node = self.tree.get_mut(id);
        node.memo_hash = hash;
        node.memo_states = if hash.is_some() { states } else { Vec::new() };
    }

    /// Allocates the retained node and fills in its
    /// kind-specific data.
    pub(super) fn create_node(&mut self, spec: Node) -> Option<NodeId> {
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
                    data.resize_step = element.resize_step;
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
}
