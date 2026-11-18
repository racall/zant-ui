//! Pointer movement, hover state, tooltips, and cursor updates.

use super::Runtime;
use crate::core::events::PointerEvent;
use crate::core::node::{ButtonType, NodeData, NodeId, NodeKind};
use crate::platform::platform::Cursor;
use crate::ui::actions::PointerPhase;

impl Runtime {
    /// Pointer moved: text-selection drag, pointer-area capture, hover and
    /// cursor updates.
    pub(crate) fn handle_pointer_move(&mut self, p: PointerEvent) {
        if self.sel_dragging {
            self.drag_text_selection(p);
            self.set_cursor(Cursor::Ibeam);
            return;
        }
        if let Some(id) = self.pointer_capture {
            // dragging: the capturing area gets every move, even outside it
            self.send_pointer(id, PointerPhase::Drag, p, 0.0, 0.0);
            self.update_cursor(Some(id));
            return;
        }
        if let Some(select_id) = self.open_select {
            let open = {
                let node = self.tree.get(select_id);
                match &node.data {
                    NodeData::Select(data) => Some(data.open),
                    _ => None,
                }
            };
            if let Some(open) = open {
                let hovered = crate::components::select::option_at(
                    self.tree.get(select_id),
                    p.x,
                    p.y,
                    self.viewport,
                );
                let changed = {
                    let node = self.tree.get_mut(select_id);
                    match &mut node.data {
                        NodeData::Select(data) => {
                            if data.hovered_option != hovered {
                                data.hovered_option = hovered;
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    }
                };
                if changed {
                    self.tree.mark_dirty(select_id, false, true);
                }
                self.set_cursor(if open {
                    Cursor::PointingHand
                } else {
                    Cursor::Arrow
                });
                return;
            }
        }
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        self.update_pointer_hover(target, p);
        let hover_box = self.hover_box_for(target, p.x, p.y);
        self.set_hover_box(hover_box);
        if let Some(id) = self.pressed_button {
            let inside = self.tree.get(id).bounds.contains(p.x, p.y);
            let changed = {
                let node = self.tree.get_mut(id);
                match &mut node.data {
                    NodeData::Button(data) => {
                        if data.pressed != inside {
                            data.pressed = inside;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            };
            if changed {
                self.tree.poke(id);
            }
        }
        if let Some(id) = self.pressed_toggle {
            let inside = self.tree.get(id).bounds.contains(p.x, p.y);
            let changed = {
                let node = self.tree.get_mut(id);
                match &mut node.data {
                    NodeData::Toggle(data) => {
                        if data.pressed != inside {
                            data.pressed = inside;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            };
            if changed {
                self.tree.poke(id);
            }
        }
        if let Some(id) = self.drag_input {
            self.set_input_cursor(id, p.x, p.y, true);
        }
        self.update_cursor_at(target, Some([p.x, p.y]));
    }

    /// Moves the hover state of the widget under the pointer and
    /// arms its tooltip timer.
    pub(crate) fn set_hover(&mut self, target: Option<NodeId>) {
        if self.hovered == target {
            return;
        }
        self.clear_tooltip();
        if let Some(previous) = self.hovered {
            let node = self.tree.get_mut(previous);
            match &mut node.data {
                NodeData::Button(data) => data.hovered = false,
                NodeData::Input(data) => data.hovered = false,
                NodeData::Toggle(data) => data.hovered = false,
                NodeData::Select(data) => data.hovered = false,
                _ => {}
            }
            self.tree.poke(previous);
        }
        self.hovered = target;
        if let Some(id) = target {
            let has_tooltip = self.tree.get(id).tooltip.is_some();
            if has_tooltip {
                self.tooltip_target = Some(id);
                self.tooltip_show_at = self.now_ms + 450;
            }
            let node = self.tree.get_mut(id);
            match &mut node.data {
                NodeData::Button(data) => data.hovered = node.enabled,
                NodeData::Input(data) => data.hovered = node.enabled,
                NodeData::Toggle(data) => data.hovered = node.enabled,
                NodeData::Select(data) => data.hovered = node.enabled,
                _ => {}
            }
            self.tree.poke(id);
        }
    }

    /// Sets the platform cursor of this runtime's window.
    fn set_cursor(&self, cursor: Cursor) {
        if let Some(window) = &self.window {
            window.set_cursor(cursor);
        }
    }

    pub(crate) fn update_cursor(&mut self, target: Option<NodeId>) {
        self.update_cursor_at(target, None);
    }

    /// The cursor of the node under the pointer
    /// (`point` lets containers / rich text probe text and links).
    pub(super) fn update_cursor_at(&mut self, target: Option<NodeId>, point: Option<[f32; 2]>) {
        let cursor = match target {
            Some(id) => match self.tree.get(id).kind() {
                NodeKind::Input => Cursor::Ibeam,
                NodeKind::Button => {
                    let (button_type, loading) = match &self.tree.get(id).data {
                        NodeData::Button(data) => (data.button_type, data.loading),
                        _ => (ButtonType::Default, false),
                    };
                    match button_type {
                        _ if loading => Cursor::Arrow,
                        ButtonType::Plain | ButtonType::Caption | ButtonType::CaptionClose => {
                            Cursor::Arrow
                        }
                        _ => Cursor::PointingHand,
                    }
                }
                NodeKind::Toggle => Cursor::PointingHand,
                NodeKind::Container => {
                    let custom = match &self.tree.get(id).data {
                        NodeData::Container(data) => data.cursor,
                        _ => None,
                    };
                    custom.unwrap_or_else(|| self.default_cursor(point))
                }
                NodeKind::RichText => match point {
                    Some(point) => {
                        if self.link_at(id, point[0], point[1]).is_some() {
                            Cursor::PointingHand
                        } else {
                            Cursor::Arrow
                        }
                    }
                    None => Cursor::Arrow,
                },
                _ => Cursor::Arrow,
            },
            None => self.default_cursor(point),
        };
        self.set_cursor(cursor);
    }

    fn default_cursor(&self, point: Option<[f32; 2]>) -> Cursor {
        match point {
            Some(point) if self.selectable_text_at(point[0], point[1]) => Cursor::Ibeam,
            _ => Cursor::Arrow,
        }
    }

    /// The container whose hover look applies: the nearest
    /// one around the widget under the pointer, or the innermost one containing
    /// the point.
    fn hover_box_for(&self, target: Option<NodeId>, x: f32, y: f32) -> Option<NodeId> {
        let Some(mut current) = target else {
            return self.tree.hover_box_at(x, y);
        };
        loop {
            let node = self.tree.get(current);
            if let NodeData::Container(data) = &node.data
                && data.hover_visual.is_some()
            {
                return Some(current);
            }
            current = node.parent?;
        }
    }

    /// Moves the container hover look (paint-only, animated).
    pub(crate) fn set_hover_box(&mut self, id: Option<NodeId>) {
        if self.hover_box == id {
            return;
        }
        if let Some(old) = self.hover_box
            && old < self.tree.nodes.len() as NodeId
        {
            if let NodeData::Container(data) = &mut self.tree.get_mut(old).data {
                data.hovered = false;
            }
            self.tree.poke(old);
        }
        self.hover_box = id;
        if let Some(new) = id {
            if let NodeData::Container(data) = &mut self.tree.get_mut(new).data {
                data.hovered = true;
            }
            self.tree.poke(new);
        }
    }
}
