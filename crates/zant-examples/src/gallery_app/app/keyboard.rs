//! Keyboard, text and IME input of the gallery's host loop, with copy / paste.

use super::*;

impl GalleryApp {
    pub(super) fn cancel_composition_for(&mut self, id: NodeId) {
        let has_composition =
            matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition);
        if has_composition {
            self.tree.input_cancel_composition(id);
        }
    }

    pub(super) fn cancel_focused_composition(&mut self) {
        if let Some(id) = self.tree.focused {
            self.cancel_composition_for(id);
        }
    }

    /// Places the caret under the pointer.
    pub(super) fn set_input_cursor(&mut self, id: NodeId, x: f32, y: f32, extend: bool) {
        let (bounds, scroll_x) = {
            let node = self.tree.get(id);
            let scroll_x = match &node.data {
                NodeData::Input(data) => data.scroll_x,
                _ => return,
            };
            (node.bounds, scroll_x)
        };
        let byte = {
            let node = self.tree.get_mut(id);
            let NodeData::Input(data) = &mut node.data else {
                return;
            };
            if data.has_composition {
                return;
            }
            if !zant::components::input::ensure_layout(data) {
                return;
            }
            let Some(layout) = data.layout.as_ref() else {
                return;
            };
            let origin_x = bounds.x + zant::components::input::PADDING_X - scroll_x;
            let origin_y = bounds.y + (bounds.height - layout.metrics().height) * 0.5;
            let byte = layout.index_at(x - origin_x, y - origin_y);
            byte.min(data.text.len())
        };
        if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
            data.cursor = byte;
            if !extend {
                data.anchor = byte;
            }
        }
        self.tree.mark_dirty(id, false, true);
    }

    pub(super) fn handle_key_down(&mut self, k: KeyEvent) -> i32 {
        let shift = k.modifiers.shift;
        match k.key {
            Key::Tab => {
                self.cancel_focused_composition();
                self.tree.tab_focus(shift);
                return 1;
            }
            Key::Escape => {
                if let Some(id) = self.tree.focused
                    && matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition)
                {
                    self.tree.input_cancel_composition(id);
                    return 1;
                }
                self.focus_node(None);
                return 1;
            }
            _ => {}
        }

        if let Some(id) = self.tree.focused {
            let is_button = matches!(self.tree.get(id).data, NodeData::Button(_));
            let is_input = matches!(self.tree.get(id).data, NodeData::Input(_));
            if is_button {
                let data = match &self.tree.get(id).data {
                    NodeData::Button(data) => data,
                    _ => unreachable!(),
                };
                if k.key == Key::Enter || events::character_equals(k.key, b' ') {
                    if let Some(action) = data.action.clone() {
                        let mut cx = ui::ActionCx::new(self as &mut dyn Any);
                        action.call(&mut cx);
                    }
                    return 1;
                }
            } else if is_input {
                let meta_or_control = k.modifiers.meta || k.modifiers.control;
                let has_composition = matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition);
                if has_composition {
                    return 0;
                }
                if meta_or_control {
                    if events::character_equals(k.key, b'a') {
                        self.tree.input_select_all(id);
                        return 1;
                    }
                    if events::character_equals(k.key, b'c') {
                        let _ = self.copy_selection(id);
                        return 1;
                    }
                    if events::character_equals(k.key, b'x') {
                        if self.copy_selection(id) && self.tree.input_delete_backward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    if events::character_equals(k.key, b'v') {
                        self.paste_into(id);
                        return 1;
                    }
                    if events::character_equals(k.key, b'a')
                        || events::character_equals(k.key, b'A')
                    {
                        self.tree.input_select_all(id);
                        return 1;
                    }
                    return 0;
                }
                match k.key {
                    Key::Left => {
                        if k.modifiers.alt {
                            self.tree.input_move(id, Move::WordLeft, shift);
                        } else {
                            self.tree.input_move(id, Move::Left, shift);
                        }
                        return 1;
                    }
                    Key::Right => {
                        if k.modifiers.alt {
                            self.tree.input_move(id, Move::WordRight, shift);
                        } else {
                            self.tree.input_move(id, Move::Right, shift);
                        }
                        return 1;
                    }
                    Key::Home => {
                        self.tree.input_move(id, Move::Home, shift);
                        return 1;
                    }
                    Key::End => {
                        self.tree.input_move(id, Move::End, shift);
                        return 1;
                    }
                    Key::Backspace => {
                        if self.tree.input_delete_backward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    Key::Delete => {
                        if self.tree.input_delete_forward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    Key::Enter => {
                        let submit = match &self.tree.get(id).data {
                            NodeData::Input(data) => data.submit_action.clone(),
                            _ => None,
                        };
                        if let Some(action) = submit {
                            let mut cx = ui::ActionCx::new(self as &mut dyn Any);
                            action.call(&mut cx);
                        }
                        return 1;
                    }
                    _ => return 0,
                }
            }
        }
        0
    }

    pub(super) fn handle_text_input(&mut self, value: String) -> i32 {
        if let Some(id) = self.tree.focused
            && matches!(self.tree.get(id).data, NodeData::Input(_))
            && self.tree.input_insert(id, &value)
        {
            self.notify_text_action(id);
            return 1;
        }
        0
    }

    pub(super) fn handle_composition(&mut self, composition: Composition) -> i32 {
        if let Some(id) = self.tree.focused
            && matches!(self.tree.get(id).data, NodeData::Input(_))
        {
            if composition.text.is_empty() {
                self.tree.input_cancel_composition(id);
            } else {
                self.tree.input_composition(
                    id,
                    &composition.text,
                    composition.selection_start,
                    composition.selection_end,
                );
            }
            return 1;
        }
        0
    }

    /// Runs the input's `text_action` after a successful edit. The tree only
    /// changes state; normally the runtime notifies the action (see
    /// `runtime/keyboard.rs:notify_text_action`), but the gallery drives the
    /// tree itself, so it notifies here.
    pub(super) fn notify_text_action(&mut self, id: NodeId) {
        let action = match &self.tree.get(id).data {
            NodeData::Input(data) => data.text_action.clone(),
            _ => None,
        };
        let Some(action) = action else {
            return;
        };
        let text = match &self.tree.get(id).data {
            NodeData::Input(data) => data.text.clone(),
            _ => return,
        };
        let mut cx = ui::ActionCx::new(self as &mut dyn Any);
        action.call(&mut cx, &text);
    }

    /// Copies the input's selection to the clipboard.
    pub(super) fn copy_selection(&mut self, id: NodeId) -> bool {
        let text = {
            let node = self.tree.get(id);
            let NodeData::Input(data) = &node.data else {
                return false;
            };
            if !zant::components::input::has_selection(data) {
                return false;
            }
            let (start, end) = zant::components::input::selection_range(data);
            if start == end {
                return false;
            }
            data.text[start..end].to_string()
        };
        platform::set_clipboard(&text)
    }

    /// Pastes the clipboard text into the input.
    pub(super) fn paste_into(&mut self, id: NodeId) {
        let Some(clipboard) = platform::clipboard_text() else {
            return;
        };
        if clipboard.is_empty() {
            return;
        }
        if self.tree.input_insert(id, &clipboard) {
            self.notify_text_action(id);
        }
    }

    /// The caret rectangle of the focused input, for the input method.
    pub fn ime_rect(&mut self) -> Option<zant::core::geometry::Rect> {
        let id = self.tree.focused?;
        if !matches!(self.tree.get(id).data, NodeData::Input(_)) {
            return None;
        }
        self.tree.input_caret_rect(id)
    }
}
