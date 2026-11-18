//! Pointer press and release, click counting, and control activation.

use super::Runtime;
use crate::components::button;
use crate::core::events::PointerEvent;
use crate::core::node::{NodeData, NodeId, NodeKind};
use crate::ui::actions::PointerPhase;

impl Runtime {
    /// Releases the pressed button / toggle without
    /// activating it.
    pub(crate) fn clear_pressed(&mut self) {
        if let Some(id) = self.pressed_button {
            if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                data.pressed = false;
            }
            self.tree.poke(id);
        }
        if let Some(id) = self.pressed_toggle {
            if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                data.pressed = false;
            }
            self.tree.poke(id);
        }
        self.pressed_button = None;
        self.pressed_toggle = None;
    }

    /// Pointer pressed: the open select popup first, then focus, press state,
    /// input caret placement and the start of a text selection.
    pub(crate) fn handle_pointer_down(&mut self, p: PointerEvent) {
        if let Some(select_id) = self.open_select {
            let is_select = matches!(self.tree.get(select_id).data, NodeData::Select(_));
            if is_select {
                let option = crate::components::select::option_at(
                    self.tree.get(select_id),
                    p.x,
                    p.y,
                    self.viewport,
                );
                if let Some(index) = option {
                    // copy the callback out before the state changes (an action
                    // may change the UI)
                    let action = {
                        let node = self.tree.get(select_id);
                        match &node.data {
                            NodeData::Select(data) => data.action.clone(),
                            _ => None,
                        }
                    };
                    if let NodeData::Select(data) = &mut self.tree.get_mut(select_id).data {
                        data.selected = index;
                        data.layout_dirty = true;
                    }
                    if let Some(action) = action {
                        let mut app = self.app.borrow_mut();
                        let mut cx = super::action_cx(&mut app);
                        action.call(&mut cx, index);
                    }
                    self.close_select();
                    self.needs_build = true;
                    return;
                }
                if self.tree.get(select_id).bounds.contains(p.x, p.y) {
                    self.close_select();
                    return;
                }
            }
            self.close_select();
            return;
        }
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        if self.begin_text_selection(target, p) {
            return;
        }
        let Some(id) = target else {
            self.focus_node(None);
            return;
        };
        let (kind, enabled, focusable) = {
            let node = self.tree.get(id);
            (node.kind(), node.enabled, node.focusable)
        };
        match kind {
            NodeKind::Button => {
                if focusable {
                    self.focus_node(Some(id));
                }
                if !enabled {
                    return;
                }
                if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = true;
                    data.hovered = true;
                }
                self.pressed_button = Some(id);
                self.tree.poke(id);
            }
            NodeKind::Toggle => {
                self.focus_node(Some(id));
                if !enabled {
                    return;
                }
                if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = true;
                    data.hovered = true;
                }
                self.pressed_toggle = Some(id);
                self.tree.poke(id);
            }
            NodeKind::Input => {
                if !enabled {
                    return;
                }
                self.focus_node(Some(id));
                self.drag_input = Some(id);
                self.set_input_cursor(id, p.x, p.y, p.modifiers.shift);
            }
            NodeKind::RichText => {
                if self.link_at(id, p.x, p.y).is_some() {
                    self.pressed_link = Some(id);
                }
            }
            NodeKind::Select => {
                if !enabled {
                    return;
                }
                self.focus_node(Some(id));
                if let NodeData::Select(data) = &mut self.tree.get_mut(id).data {
                    data.open = true;
                    data.hovered_option = None;
                }
                self.open_select = Some(id);
                self.tree.poke(id);
            }
            NodeKind::Container => {
                let (has_pointer, clicks) = match &self.tree.get(id).data {
                    NodeData::Container(data) => (
                        data.pointer.is_some(),
                        data.pointer.as_ref().is_some_and(|area| area.clicks),
                    ),
                    _ => (false, false),
                };
                if has_pointer && clicks && enabled {
                    self.focus_node(if focusable { Some(id) } else { None });
                    self.count_click(p);
                    self.pointer_capture = Some(id);
                    self.send_pointer(id, PointerPhase::Down, p, 0.0, 0.0);
                } else {
                    // blocking panels (block_pointer) still blur inputs; a
                    // scroll-only area (a list) takes the focus for its keys
                    self.focus_node(if has_pointer && focusable {
                        Some(id)
                    } else {
                        None
                    });
                }
            }
            _ => {}
        }
        self.update_cursor(Some(id));
    }

    /// Double / triple click detection.
    pub(crate) fn count_click(&mut self, p: PointerEvent) {
        let near = (p.x - self.last_click_x).abs() <= 4.0 && (p.y - self.last_click_y).abs() <= 4.0;
        if near && self.now_ms - self.last_click_ms <= 500 && self.click_count > 0 {
            self.click_count = if self.click_count >= 3 {
                1
            } else {
                self.click_count + 1
            };
        } else {
            self.click_count = 1;
        }
        self.last_click_ms = self.now_ms;
        self.last_click_x = p.x;
        self.last_click_y = p.y;
    }

    /// Activates buttons / toggles / links, finishes a
    /// pointer-area drag and a text selection.
    pub(crate) fn handle_pointer_up(&mut self, p: PointerEvent) {
        self.drag_input = None;
        if self.sel_dragging {
            self.sel_dragging = false;
            // a plain click selects nothing
            if self.sel_mode == super::SelMode::Char
                && let Some(anchor) = self.sel_anchor
                && let Some(focus) = self.sel_focus
                && anchor.node == focus.node
                && anchor.byte == focus.byte
            {
                self.clear_text_selection();
            }
        }
        // a drag is not a click: the next press starts a new click count
        if (p.x - self.last_click_x).abs() > 4.0 || (p.y - self.last_click_y).abs() > 4.0 {
            self.click_count = 0;
        }
        if let Some(id) = self.pointer_capture {
            self.pointer_capture = None;
            self.send_pointer(id, PointerPhase::Up, p, 0.0, 0.0);
        }
        if let Some(id) = self.pressed_button {
            self.pressed_button = None;
            let (alive, inside, enabled) = {
                let node = self.tree.get(id);
                // a loading button ignores clicks (antd)
                let loading = matches!(&node.data, NodeData::Button(data) if data.loading);
                (
                    node.alive,
                    node.bounds.contains(p.x, p.y),
                    node.enabled && !loading,
                )
            };
            if alive {
                if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = false;
                }
                self.tree.poke(id);
                if inside && enabled {
                    let now = self.now_ms;
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        button::start_wave(data, now);
                    }
                    // copy the callbacks first: an action may change the UI
                    // (or run a modal dialog), after which the node must not be
                    // read again
                    let (index_action, index, action) = {
                        let node = self.tree.get(id);
                        match &node.data {
                            NodeData::Button(data) => {
                                (data.index_action.clone(), data.index, data.action.clone())
                            }
                            _ => (None, 0, None),
                        }
                    };
                    let mut app = self.app.borrow_mut();
                    let mut cx = super::action_cx(&mut app);
                    if let Some(index_action) = index_action {
                        index_action.call(&mut cx, index);
                    } else if let Some(action) = action {
                        action.call(&mut cx);
                    }
                }
            }
        }
        if let Some(id) = self.pressed_toggle {
            let (alive, inside, enabled) = {
                let node = self.tree.get(id);
                (node.alive, node.bounds.contains(p.x, p.y), node.enabled)
            };
            if alive {
                let (kind, checked) = {
                    let node = self.tree.get(id);
                    match &node.data {
                        NodeData::Toggle(data) => (data.kind, data.checked),
                        _ => (crate::core::node::ToggleKind::Checkbox, false),
                    }
                };
                if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = false;
                }
                if inside && enabled {
                    if kind == crate::core::node::ToggleKind::Radio {
                        if !checked {
                            self.toggle_set(id, true);
                        }
                    } else {
                        self.toggle_flip(id);
                    }
                }
                self.tree.poke(id);
            }
            self.pressed_toggle = None;
        }
        if let Some(id) = self.pressed_link {
            self.pressed_link = None;
            let inside = self.tree.get(id).bounds.contains(p.x, p.y);
            if inside && let Some(link) = self.link_at(id, p.x, p.y) {
                let action = match &self.tree.get(id).data {
                    NodeData::RichText(data) => data.link_action.clone(),
                    _ => None,
                };
                if let Some(action) = action {
                    // The span memory may be replaced by the callback's
                    // rebuild; hand the callback a stable copy (owned String).
                    let mut app = self.app.borrow_mut();
                    let mut cx = super::action_cx(&mut app);
                    action.call(&mut cx, &link);
                }
            }
        }
        self.update_cursor_at(self.tree.hit_test(p.x, p.y), Some([p.x, p.y]));
    }

    /// Sets the checked state and calls the toggle's
    /// callback — the index action when the toggle belongs to a radio group,
    /// otherwise the boolean action. The callbacks are copied out of the node
    /// before they run.
    pub(crate) fn toggle_set(&mut self, id: NodeId, checked: bool) {
        {
            let node = self.tree.get_mut(id);
            let NodeData::Toggle(data) = &mut node.data else {
                return;
            };
            data.checked = checked;
        }
        self.tree.poke(id);
        let (index_action, index, action) = {
            let node = self.tree.get(id);
            match &node.data {
                NodeData::Toggle(data) => {
                    (data.index_action.clone(), data.index, data.action.clone())
                }
                _ => return,
            }
        };
        let mut app = self.app.borrow_mut();
        let mut cx = super::action_cx(&mut app);
        if let Some(index_action) = index_action {
            index_action.call(&mut cx, index);
        } else if let Some(action) = action {
            action.call(&mut cx, checked);
        }
    }

    /// Flips a checkbox / switch (radios only ever turn
    /// on, see the pointer / keyboard arms).
    pub(crate) fn toggle_flip(&mut self, id: NodeId) {
        let NodeData::Toggle(data) = &self.tree.get(id).data else {
            return;
        };
        let checked = !data.checked;
        self.toggle_set(id, checked);
    }
}
