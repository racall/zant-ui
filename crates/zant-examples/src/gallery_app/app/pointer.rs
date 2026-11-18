//! Pointer input of the gallery's hand-written host loop: hover, cursors, press / release, focus.

use super::*;

impl GalleryApp {
    /// Handles a translated native event; 1 = consumed.
    pub fn handle(&mut self, event: Event) -> i32 {
        match event {
            Event::PointerMove(p) | Event::PointerEnter(p) => {
                self.handle_pointer_move(p);
                1
            }
            Event::PointerDown(p) => {
                self.handle_pointer_down(p);
                1
            }
            Event::PointerUp(p) => {
                self.handle_pointer_up(p);
                1
            }
            Event::PointerLeave(_) => {
                self.set_hover(None);
                self.drag_input = None;
                if let Some(id) = self.pressed_button {
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.pressed = false;
                    }
                    self.pressed_button = None;
                    self.tree.mark_paint_dirty();
                }
                self.set_cursor(Cursor::Arrow);
                1
            }
            Event::KeyDown(k) => self.handle_key_down(k),
            Event::KeyUp(_) => 0,
            Event::TextInput(value) => self.handle_text_input(value),
            Event::Composition(composition) => self.handle_composition(composition),
            Event::WindowBlur => {
                self.active = false;
                self.cancel_focused_composition();
                self.tree.set_focus(None);
                self.set_hover(None);
                if let Some(id) = self.pressed_button {
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.pressed = false;
                    }
                    self.pressed_button = None;
                }
                self.tree.mark_paint_dirty();
                0
            }
            Event::WindowFocus => {
                self.active = true;
                self.tree.mark_paint_dirty();
                0
            }
            Event::WindowResize(_) => {
                self.tree.mark_layout_dirty();
                0
            }
            Event::WindowClose
            | Event::WindowCloseRequest
            | Event::AppQuitRequest
            | Event::AppearanceChange => 0,
            Event::Scroll(_) => 0,
        }
    }

    pub(super) fn handle_pointer_move(&mut self, p: PointerEvent) {
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        if let Some(id) = self.pressed_button {
            let (inside, pressed) = {
                let node = self.tree.get(id);
                let pressed = match &node.data {
                    NodeData::Button(data) => data.pressed,
                    _ => false,
                };
                (node.bounds.contains(p.x, p.y), pressed)
            };
            if pressed != inside
                && let NodeData::Button(data) = &mut self.tree.get_mut(id).data
            {
                data.pressed = inside;
                self.tree.mark_dirty(id, false, true);
            }
        }
        if let Some(id) = self.drag_input {
            self.set_input_cursor(id, p.x, p.y, true);
        }
        self.update_cursor(target);
    }

    pub(super) fn set_hover(&mut self, target: Option<NodeId>) {
        if self.hovered == target {
            return;
        }
        if let Some(previous) = self.hovered {
            if let NodeData::Button(data) = &mut self.tree.get_mut(previous).data {
                data.hovered = false;
            } else if let NodeData::Input(data) = &mut self.tree.get_mut(previous).data {
                data.hovered = false;
            }
            self.tree.mark_dirty(previous, false, true);
        }
        self.hovered = target;
        if let Some(id) = target {
            let enabled = self.tree.get(id).enabled;
            if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                data.hovered = enabled;
            } else if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
                data.hovered = enabled;
            }
            self.tree.mark_dirty(id, false, true);
        }
        self.tree.mark_paint_dirty();
    }

    /// The pointer cursor for the node under the pointer.
    pub(super) fn update_cursor(&mut self, target: Option<NodeId>) {
        let cursor = match target {
            Some(id) => match &self.tree.get(id).data {
                NodeData::Input(_) => Cursor::Ibeam,
                NodeData::Button(_) => Cursor::PointingHand,
                _ => Cursor::Arrow,
            },
            None => Cursor::Arrow,
        };
        self.set_cursor(cursor);
    }

    pub(super) fn set_cursor(&self, cursor: Cursor) {
        if let Some(window) = self.window.as_ref().and_then(Weak::upgrade) {
            window.set_cursor(cursor);
        }
    }

    pub(super) fn handle_pointer_down(&mut self, p: PointerEvent) {
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        if let Some(id) = target {
            let enabled = self.tree.get(id).enabled;
            match &self.tree.get(id).data {
                NodeData::Button(_) => {
                    self.focus_node(Some(id));
                    if !enabled {
                        return;
                    }
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.pressed = true;
                        data.hovered = true;
                    }
                    self.pressed_button = Some(id);
                    self.tree.mark_dirty(id, false, true);
                }
                NodeData::Input(_) => {
                    if !enabled {
                        return;
                    }
                    self.focus_node(Some(id));
                    self.drag_input = Some(id);
                    self.set_input_cursor(id, p.x, p.y, p.modifiers.shift);
                }
                _ => {}
            }
            self.update_cursor(target);
        } else {
            self.focus_node(None);
            self.tree.mark_paint_dirty();
        }
    }

    pub(super) fn handle_pointer_up(&mut self, p: PointerEvent) {
        self.drag_input = None;
        if let Some(id) = self.pressed_button {
            let (inside, enabled) = {
                let node = self.tree.get(id);
                (node.bounds.contains(p.x, p.y), node.enabled)
            };
            let action = if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                data.pressed = false;
                data.action.clone()
            } else {
                None
            };
            if inside
                && enabled
                && let Some(action) = action
            {
                let mut cx = ui::ActionCx::new(self as &mut dyn Any);
                action.call(&mut cx);
            }
            self.pressed_button = None;
            self.tree.mark_dirty(id, false, true);
        }
        self.update_cursor(self.tree.hit_test(p.x, p.y));
    }

    pub(super) fn focus_node(&mut self, id: Option<NodeId>) {
        if let Some(old) = self.tree.focused
            && Some(old) != id
        {
            self.cancel_composition_for(old);
        }
        self.tree.set_focus(id);
        self.tree.mark_paint_dirty();
    }
}
