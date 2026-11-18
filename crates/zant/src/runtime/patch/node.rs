//! `patch_node`: patching a node's kind-specific data from its element.

use super::*;

impl Runtime {
    /// Updates the retained node `id` from `spec` and
    /// reconciles its children for containers. Returns true when the layout
    /// must run.
    pub(crate) fn patch_node(&mut self, id: NodeId, spec: Node) -> bool {
        let element = spec.element();
        let mut layout_changed = self.apply_common(id, element);
        let mut paint_changed = layout_changed;
        let kind = self.tree.get(id).kind();

        match kind {
            crate::core::node::NodeKind::Container => {
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Container(data) = &mut node.data else {
                        return layout_changed;
                    };
                    // a pointer area appearing / disappearing changes hit testing
                    if data.pointer.is_none() != element.pointer.is_none() {
                        paint_changed = true;
                    }
                    data.pointer = element.pointer.clone();
                    data.key_action = element.key_action.clone();
                    data.cursor = element.cursor;
                    data.resize_action = element.on_resize.clone();
                    if data.hover_visual != element.hover_visual {
                        paint_changed = true;
                    }
                    data.hover_visual = element.hover_visual;
                }
                if element.pointer.is_none() && self.pointer_capture == Some(id) {
                    self.pointer_capture = None;
                }
            }
            crate::core::node::NodeKind::Text => {
                let (content_changed, style_changed) = {
                    let NodeData::Text(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    (
                        data.content.as_str() != element.content.as_str(),
                        !text_style_equal(&data.style, &element.text_style),
                    )
                };
                if content_changed {
                    self.tree.set_text(id, &element.content);
                    layout_changed = true;
                    paint_changed = true;
                }
                if style_changed {
                    if let NodeData::Text(data) = &mut self.tree.get_mut(id).data {
                        data.style = element.text_style;
                        data.invalidate();
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
            }
            crate::core::node::NodeKind::Button => {
                let label_changed = {
                    let NodeData::Button(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    data.label != element.content
                };
                if label_changed {
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.label = Str::copy(&element.content);
                        data.invalidate_layout();
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Button(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.button_type != element.button_type
                        || data.size != element.button_size
                        || data.font_size != element.text_style.font_size
                    {
                        data.button_type = element.button_type;
                        data.size = element.button_size;
                        data.font_size = element.text_style.font_size;
                        data.invalidate_layout();
                        layout_changed = true;
                        paint_changed = true;
                    }
                    data.action = element.on_click.clone();
                    data.index_action = element.index_action.clone();
                    data.index = element.index;
                    node.focusable = element.focusable;
                    data.hover_text_color = element.hover_text_color;
                    if data.icon != element.icon
                        || data.icon_size != element.icon_size
                        || data.text_color != element.text_color
                        || data.text_align != element.text_style.text_align
                        || data.label_padding != element.label_padding
                    {
                        data.icon = element.icon;
                        data.icon_size = element.icon_size;
                        data.text_color = element.text_color;
                        data.text_align = element.text_style.text_align;
                        data.label_padding = element.label_padding;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    if data.danger != element.danger {
                        data.danger = element.danger;
                        paint_changed = true;
                    }
                    // the spinner widens the button and starts its frames
                    if data.loading != element.loading {
                        data.loading = element.loading;
                        layout_changed = true;
                        paint_changed = true;
                        self.tree.poke(id);
                    }
                }
            }
            crate::core::node::NodeKind::Input => {
                if element.controlled {
                    let value_changed = {
                        let NodeData::Input(data) = &self.tree.get(id).data else {
                            return layout_changed;
                        };
                        data.text.as_str() != element.content.as_str()
                    };
                    if value_changed {
                        self.tree.input_set_value(id, &element.content);
                        paint_changed = true;
                    }
                }
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Input(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.placeholder != element.placeholder {
                        data.placeholder = Str::copy(&element.placeholder);
                        data.placeholder_layout = None;
                        paint_changed = true;
                    }
                    if data.read_only != element.read_only {
                        data.read_only = element.read_only;
                        paint_changed = true;
                    }
                    if data.password != element.password {
                        data.password = element.password;
                        data.rebuild_display();
                        paint_changed = true;
                    }
                    if data.font_size != element.text_style.font_size
                        || data.line_height != element.text_style.line_height
                    {
                        data.font_size = element.text_style.font_size;
                        data.line_height = element.text_style.line_height;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    data.selection_action = element.on_selection.clone();
                    data.key_action = element.on_key.clone();
                    if apply_selection(data, element.selection) {
                        paint_changed = true;
                    }
                    if data.content_max_width != element.content_max_width {
                        data.content_max_width = element.content_max_width;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    data.text_action = element.on_text.clone();
                    data.submit_action = element.on_click.clone();
                    data.undo_action = element.on_undo.clone();
                    data.redo_action = element.on_redo.clone();
                    data.focus_ring = element.focus_ring;
                    if data.multiline != element.multiline {
                        data.multiline = element.multiline;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    node.focusable = true;
                }
            }
            crate::core::node::NodeKind::Vector => {
                {
                    let NodeData::Vector(data) = &mut self.tree.get_mut(id).data else {
                        return layout_changed;
                    };
                    if data.copy != element.content {
                        data.copy = Str::copy(&element.content);
                    }
                }
                if let Some(drawing) = element.drawing.as_deref() {
                    let before = match &self.tree.get(id).data {
                        NodeData::Vector(data) => data.hash,
                        _ => 0,
                    };
                    self.tree.vector_set(id, drawing);
                    let after = match &self.tree.get(id).data {
                        NodeData::Vector(data) => data.hash,
                        _ => before,
                    };
                    if before != after {
                        layout_changed = true;
                        paint_changed = true;
                    }
                }
            }
            crate::core::node::NodeKind::RichText => {
                let spans_changed = {
                    let NodeData::RichText(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    !rich_spans_equal(&data.spans, &element.rich_spans)
                };
                if spans_changed {
                    self.tree.rich_text_set(id, &element.rich_spans);
                    layout_changed = true;
                    paint_changed = true;
                }
                {
                    let NodeData::RichText(data) = &mut self.tree.get_mut(id).data else {
                        return layout_changed;
                    };
                    if data.line_height != element.rich_line_height {
                        data.line_height = element.rich_line_height;
                        data.invalidate_layout();
                        layout_changed = true;
                        paint_changed = true;
                    }
                    data.link_action = element.on_link.clone();
                }
            }
            crate::core::node::NodeKind::Select => {
                let options_changed = {
                    let NodeData::Select(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    !select_options_equal(data, &element.options)
                };
                if options_changed {
                    {
                        let NodeData::Select(data) = &mut self.tree.get_mut(id).data else {
                            return layout_changed;
                        };
                        data.clear_options();
                        for label in &element.options {
                            data.options.push(crate::core::node::SelectOption {
                                label: label.clone(),
                            });
                        }
                        if data.selected >= data.options.len() {
                            data.selected = 0;
                        }
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Select(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.selected != element.selected {
                        let last = data.options.len().saturating_sub(1);
                        data.selected = element.selected.min(last);
                        data.layout_dirty = true;
                        paint_changed = true;
                    }
                    data.action = element.index_action.clone();
                    node.focusable = true;
                }
            }
            crate::core::node::NodeKind::Image => {
                let node = self.tree.get_mut(id);
                let NodeData::Image(data) = &mut node.data else {
                    return layout_changed;
                };
                if element.image_id != 0 {
                    if data.source_id != element.image_id {
                        // compare the intrinsic size before / after
                        // `set_source`: a differently sized source changes
                        // the measured box.
                        let old_width = data.intrinsic_width;
                        let old_height = data.intrinsic_height;
                        crate::components::image::set_source(
                            data,
                            element.image_handle.as_ref(),
                            element.image_id,
                        );
                        if old_width != data.intrinsic_width || old_height != data.intrinsic_height
                        {
                            layout_changed = true;
                        }
                        paint_changed = true;
                    }
                } else if data.source_id != 0 || data.path != element.content {
                    // the file is loaded synchronously here; this runs
                    // during reconcile, never from `render`.
                    data.path = Str::copy(&element.content);
                    data.source_id = 0;
                    crate::components::image::load(data);
                    layout_changed = true;
                    paint_changed = true;
                }
                if data.fit != element.image_fit {
                    data.fit = element.image_fit;
                    paint_changed = true;
                }
            }
            crate::core::node::NodeKind::Toggle => {
                let label_changed = {
                    let NodeData::Toggle(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    data.label != element.content
                };
                if label_changed {
                    if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                        data.label = Str::copy(&element.content);
                        data.invalidate_layout();
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
                let checked_changed;
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Toggle(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.kind != element.toggle_kind {
                        data.kind = element.toggle_kind;
                        layout_changed = true;
                        paint_changed = true;
                    }
                    if data.font_size != element.text_style.font_size {
                        data.font_size = element.text_style.font_size;
                        data.invalidate_layout();
                        layout_changed = true;
                        paint_changed = true;
                    }
                    checked_changed = data.checked != element.checked;
                    if checked_changed {
                        data.checked = element.checked;
                        paint_changed = true;
                    }
                    data.action = element.on_bool.clone();
                    data.index_action = element.index_action.clone();
                    data.index = element.index;
                    node.focusable = true;
                }
                if checked_changed {
                    self.tree.poke(id);
                }
            }
            crate::core::node::NodeKind::Root => {}
        }

        if kind == crate::core::node::NodeKind::Container
            && self.reconcile_children(id, &element.children)
        {
            layout_changed = true;
        }
        if layout_changed {
            self.tree.mark_dirty(id, true, true);
        } else if paint_changed {
            self.tree.mark_dirty(id, false, true);
        }
        layout_changed
    }
}
