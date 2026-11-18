//! Key dispatch, focus navigation, and keyboard activation of controls.

use crate::core::events::{self, Key, KeyEvent};
use crate::core::node::{NodeData, NodeId};

use super::Runtime;

impl Runtime {
    /// Returns 1 when the key was consumed.
    pub(crate) fn handle_key_down(&mut self, k: KeyEvent) -> i32 {
        let shift = k.modifiers.shift;
        // focused pointer areas (canvases, viewers) see keys first
        if let Some(id) = self.tree.focused {
            let hook = match &self.tree.get(id).data {
                NodeData::Container(data) => data.key_action.clone(),
                _ => None,
            };
            if let Some(hook) = hook {
                let consumed = {
                    let mut app = self.app.borrow_mut();
                    let mut cx = super::action_cx(&mut app);
                    hook.call(&mut cx, k)
                };
                if consumed {
                    self.needs_build = true;
                    return 1;
                }
            }
        }
        let input_focused = self
            .tree
            .focused
            .is_some_and(|id| matches!(self.tree.get(id).data, NodeData::Input(_)));
        // selected text: copy / clear / select all (unless an input is focused)
        if !input_focused {
            let primary = k.modifiers.primary();
            if self.sel_anchor.is_some() {
                if primary && events::character_equals(k.key, b'c') {
                    let _ = self.copy_text_selection();
                    return 1;
                }
                if k.key == Key::Escape {
                    self.clear_text_selection();
                    return 1;
                }
            }
            if primary
                && events::character_equals(k.key, b'a')
                && let Some(root) = self.sel_root
            {
                self.select_all_text(root);
                return 1;
            }
        }
        // input key hooks see every key first (Tab, Enter, shortcuts)
        if let Some(id) = self.tree.focused {
            let hook = match &self.tree.get(id).data {
                NodeData::Input(data) if !data.has_composition => data.key_action.clone(),
                _ => None,
            };
            if let Some(hook) = hook {
                let node = self.tree.get(id);
                let NodeData::Input(data) = &node.data else {
                    unreachable!()
                };
                let consumed = {
                    let mut app = self.app.borrow_mut();
                    let mut cx = super::action_cx(&mut app);
                    hook.call(&mut cx, k, &data.text, data.anchor, data.cursor)
                };
                if consumed {
                    self.needs_build = true;
                    return 1;
                }
            }
        }
        match k.key {
            Key::Tab => {
                self.cancel_focused_composition();
                self.tree.tab_focus(shift);
                self.close_select_unless_focused();
                return 1;
            }
            Key::Escape => {
                if self.open_select.is_some() {
                    self.close_select();
                    return 1;
                }
                if let Some(id) = self.tree.focused
                    && matches!(&self.tree.get(id).data, NodeData::Input(data) if data.has_composition)
                {
                    self.tree.input_cancel_composition(id);
                    return 1;
                }
                // a modal or popover closes (antd `keyboard`)
                if let Some(layer) = self.tree.escape_target()
                    && self.run_button_action(layer)
                {
                    return 1;
                }
                self.focus_node(None);
                return 1;
            }
            _ => {}
        }

        let Some(id) = self.tree.focused else {
            return 0;
        };
        match self.tree.get(id).kind() {
            crate::core::node::NodeKind::Button => {
                if k.key == Key::Enter || events::character_equals(k.key, b' ') {
                    let now = self.now_ms;
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        crate::components::button::start_wave(data, now);
                    }
                    self.tree.poke(id);
                    self.run_button_action(id);
                    return 1;
                }
                0
            }
            crate::core::node::NodeKind::Toggle => {
                if k.key == Key::Enter || events::character_equals(k.key, b' ') {
                    let (kind, checked) = {
                        let node = self.tree.get(id);
                        match &node.data {
                            NodeData::Toggle(data) => (data.kind, data.checked),
                            _ => (crate::core::node::ToggleKind::Checkbox, false),
                        }
                    };
                    if kind == crate::core::node::ToggleKind::Radio {
                        if !checked {
                            self.toggle_set(id, true);
                        }
                    } else {
                        self.toggle_flip(id);
                    }
                    return 1;
                }
                0
            }
            crate::core::node::NodeKind::Select => self.select_key(id, k),
            crate::core::node::NodeKind::Input => self.input_key(id, k),
            _ => 0,
        }
    }

    /// Runs button `id`'s action as a click would; false when it has none.
    fn run_button_action(&mut self, id: NodeId) -> bool {
        // copy first: the node must not be read after an action
        let (index_action, index, action) = {
            let node = self.tree.get(id);
            match &node.data {
                // a loading button ignores activation (antd)
                NodeData::Button(data) if data.loading => (None, 0, None),
                NodeData::Button(data) => {
                    (data.index_action.clone(), data.index, data.action.clone())
                }
                _ => (None, 0, None),
            }
        };
        if index_action.is_none() && action.is_none() {
            return false;
        }
        let mut app = self.app.borrow_mut();
        let mut cx = super::action_cx(&mut app);
        if let Some(index_action) = index_action {
            index_action.call(&mut cx, index);
        } else if let Some(action) = action {
            action.call(&mut cx);
        }
        drop(app);
        self.needs_build = true;
        true
    }

    /// Key handling of a focused select.
    fn select_key(&mut self, id: NodeId, k: KeyEvent) -> i32 {
        let (open, len, selected) = {
            let node = self.tree.get(id);
            match &node.data {
                NodeData::Select(data) => (data.open, data.options.len(), data.selected),
                _ => return 0,
            }
        };
        if k.key == Key::Escape {
            self.close_select();
            return 1;
        }
        if k.key == Key::Enter || events::character_equals(k.key, b' ') {
            if open {
                self.close_select();
            } else {
                if let NodeData::Select(data) = &mut self.tree.get_mut(id).data {
                    data.open = true;
                }
                self.open_select = Some(id);
                self.tree.poke(id);
            }
            return 1;
        }
        if k.key == Key::Up || k.key == Key::Down {
            if len == 0 {
                return 1;
            }
            let next = if k.key == Key::Up {
                if selected == 0 { len - 1 } else { selected - 1 }
            } else {
                (selected + 1) % len
            };
            let action = {
                let NodeData::Select(data) = &mut self.tree.get_mut(id).data else {
                    return 1;
                };
                data.selected = next;
                data.layout_dirty = true;
                data.action.clone()
            };
            if let Some(action) = action {
                let mut app = self.app.borrow_mut();
                let mut cx = super::action_cx(&mut app);
                action.call(&mut cx, next);
            }
            self.tree.mark_dirty(id, false, true);
            return 1;
        }
        0
    }
}
