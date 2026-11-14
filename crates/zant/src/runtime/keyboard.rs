//! Port of `src/ui_runtime.zig:2229-2574`: keyboard and text input.
//!
//! Keys go to: the app's `on_key` hook (in `dispatch`), a focused pointer
//! area's `on_key`, the selected text (copy / clear / select all), a focused
//! input's `on_key` hook, Tab / Escape handling, the focused button / toggle /
//! select, and finally the focused input's editing keys (including the macOS
//! and Windows navigation differences).
#![forbid(unsafe_code)]

use crate::core::events::{self, Key, KeyEvent};
use crate::core::node::{NodeData, NodeId};
use crate::core::tree::Move;
use crate::platform::platform;

use super::Runtime;

impl Runtime {
    /// Zig `handleKeyDown`: returns 1 when the key was consumed.
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
                    let mut cx = super::action_cx(&mut self.app);
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
                    let mut cx = super::action_cx(&mut self.app);
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
                    // copy first: the node must not be read after an action
                    let (index_action, index, action) = {
                        let node = self.tree.get(id);
                        match &node.data {
                            NodeData::Button(data) => {
                                (data.index_action.clone(), data.index, data.action.clone())
                            }
                            _ => (None, 0, None),
                        }
                    };
                    let mut cx = super::action_cx(&mut self.app);
                    if let Some(index_action) = index_action {
                        index_action.call(&mut cx, index);
                    } else if let Some(action) = action {
                        action.call(&mut cx);
                    }
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
                            self.tree.toggle_set(id, true);
                        }
                    } else {
                        self.tree.toggle_flip(id);
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

    /// Zig's `.select` key arm of `handleKeyDown`.
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
                let mut cx = super::action_cx(&mut self.app);
                action.call(&mut cx, next);
            }
            self.tree.mark_dirty(id, false, true);
            return 1;
        }
        0
    }

    /// Zig's `.input` key arm of `handleKeyDown`.
    fn input_key(&mut self, id: NodeId, k: KeyEvent) -> i32 {
        let shift = k.modifiers.shift;
        let (has_composition, multiline) = {
            let node = self.tree.get(id);
            match &node.data {
                NodeData::Input(data) => (data.has_composition, data.multiline),
                _ => return 0,
            }
        };
        if has_composition {
            return 0;
        }
        if k.modifiers.meta || k.modifiers.control {
            let action_char = |c: u8| events::character_equals(k.key, c);
            if action_char(b'a') {
                self.tree.input_select_all(id);
                return 1;
            }
            if action_char(b'c') {
                let _ = self.copy_selection(id);
                return 1;
            }
            if action_char(b'x') {
                if self.copy_selection(id) && self.tree.input_delete_backward(id) {
                    self.notify_text_action(id);
                }
                return 1;
            }
            if action_char(b'v') {
                self.paste_into(id);
                return 1;
            }
            if action_char(b'z') {
                if shift {
                    if self.call_undo_redo(id, true) {
                        return 1;
                    }
                } else if self.call_undo_redo(id, false) {
                    return 1;
                }
            }
            if action_char(b'y') && self.call_undo_redo(id, true) {
                return 1;
            }
            // macOS text navigation: ⌘← / ⌘→ line edges, ⌘↑ / ⌘↓ document edges
            if k.modifiers.meta {
                match k.key {
                    Key::Left => {
                        self.tree.input_move(id, Move::LineStart, shift);
                        return 1;
                    }
                    Key::Right => {
                        self.tree.input_move(id, Move::LineEnd, shift);
                        return 1;
                    }
                    Key::Up if multiline => {
                        self.tree.input_move(id, Move::DocStart, shift);
                        return 1;
                    }
                    Key::Down if multiline => {
                        self.tree.input_move(id, Move::DocEnd, shift);
                        return 1;
                    }
                    _ => {}
                }
            }
            // Windows text navigation: Ctrl+← / Ctrl+→ words, Ctrl+Home /
            // Ctrl+End document edges, Ctrl+Backspace / Ctrl+Delete a word
            if platform::IS_WINDOWS && k.modifiers.control && !k.modifiers.alt {
                match k.key {
                    Key::Left => {
                        self.tree.input_move(id, Move::WordLeft, shift);
                        return 1;
                    }
                    Key::Right => {
                        self.tree.input_move(id, Move::WordRight, shift);
                        return 1;
                    }
                    Key::Home => {
                        self.tree.input_move(
                            id,
                            if multiline {
                                Move::DocStart
                            } else {
                                Move::Home
                            },
                            shift,
                        );
                        return 1;
                    }
                    Key::End => {
                        self.tree.input_move(
                            id,
                            if multiline { Move::DocEnd } else { Move::End },
                            shift,
                        );
                        return 1;
                    }
                    Key::Backspace => {
                        self.tree.input_move(id, Move::WordLeft, true);
                        if self.tree.input_delete_backward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    Key::Delete => {
                        self.tree.input_move(id, Move::WordRight, true);
                        if self.tree.input_delete_forward(id) {
                            self.notify_text_action(id);
                        }
                        return 1;
                    }
                    _ => {}
                }
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
                1
            }
            Key::Right => {
                if k.modifiers.alt {
                    self.tree.input_move(id, Move::WordRight, shift);
                } else {
                    self.tree.input_move(id, Move::Right, shift);
                }
                1
            }
            Key::Home => {
                self.tree.input_move(
                    id,
                    if multiline {
                        Move::LineStart
                    } else {
                        Move::Home
                    },
                    shift,
                );
                1
            }
            Key::End => {
                self.tree
                    .input_move(id, if multiline { Move::LineEnd } else { Move::End }, shift);
                1
            }
            Key::Up => {
                if !multiline {
                    return 0;
                }
                self.tree.input_move(id, Move::Up, shift);
                1
            }
            Key::Down => {
                if !multiline {
                    return 0;
                }
                self.tree.input_move(id, Move::Down, shift);
                1
            }
            Key::Backspace => {
                if self.tree.input_delete_backward(id) {
                    self.notify_text_action(id);
                }
                1
            }
            Key::Delete => {
                if self.tree.input_delete_forward(id) {
                    self.notify_text_action(id);
                }
                1
            }
            Key::Enter => {
                if multiline {
                    if self.tree.input_insert(id, "\n") {
                        self.notify_text_action(id);
                    }
                } else if let Some(action) = {
                    let node = self.tree.get(id);
                    match &node.data {
                        NodeData::Input(data) => data.submit_action.clone(),
                        _ => None,
                    }
                } {
                    let mut cx = super::action_cx(&mut self.app);
                    action.call(&mut cx);
                }
                1
            }
            Key::Character(_) | Key::PageUp | Key::PageDown | Key::Function(_) => 0,
            Key::Tab | Key::Escape => 0,
        }
    }

    /// Calls the focused input's undo (`redo == false`) / redo action; true when
    /// it existed.
    fn call_undo_redo(&mut self, id: NodeId, redo: bool) -> bool {
        let action = {
            let node = self.tree.get(id);
            match &node.data {
                NodeData::Input(data) => {
                    if redo {
                        data.redo_action.clone()
                    } else {
                        data.undo_action.clone()
                    }
                }
                _ => None,
            }
        };
        let Some(action) = action else {
            return false;
        };
        let mut cx = super::action_cx(&mut self.app);
        action.call(&mut cx);
        self.needs_build = true;
        true
    }

    /// Zig `handleTextInput`: types text into the focused input, dropping
    /// control characters (an input method or a stray key event may deliver
    /// Esc / Backspace as text).
    pub(crate) fn handle_text_input(&mut self, raw: &str) -> i32 {
        let cleaned;
        let value = if has_control_chars(raw) {
            cleaned = raw
                .chars()
                .filter(|&c| {
                    !((c < '\u{20}' && c != '\t' && c != '\n' && c != '\r') || c == '\u{7f}')
                })
                .collect::<String>();
            cleaned.as_str()
        } else {
            raw
        };
        if value.is_empty() {
            return 0;
        }
        if let Some(id) = self.tree.focused
            && matches!(self.tree.get(id).data, NodeData::Input(_))
            && self.tree.input_insert(id, value)
        {
            self.notify_text_action(id);
            return 1;
        }
        0
    }

    /// Zig `Tree.inputChanged` → `components/input.zig` calls the input's
    /// `on_change` (legacy `Callback`, not ported) and then its `text_action`.
    /// The Rust tree only changes state, so the runtime notifies the action
    /// here after every successful edit.
    pub(crate) fn notify_text_action(&mut self, id: NodeId) {
        let action = {
            let node = self.tree.get(id);
            match &node.data {
                NodeData::Input(data) => data.text_action.clone(),
                _ => None,
            }
        };
        let Some(action) = action else {
            return;
        };
        let node = self.tree.get(id);
        let NodeData::Input(data) = &node.data else {
            return;
        };
        let mut cx = super::action_cx(&mut self.app);
        action.call(&mut cx, &data.text);
    }

    /// Zig `copySelection`: puts the input's selection on the clipboard.
    fn copy_selection(&mut self, id: NodeId) -> bool {
        let text = {
            let node = self.tree.get(id);
            let NodeData::Input(data) = &node.data else {
                return false;
            };
            if !super::ime::has_selection(data) {
                return false;
            }
            let (start, end) = super::ime::selection_range(data);
            if start == end {
                return false;
            }
            data.text.get(start..end).map(str::to_owned)
        };
        let Some(text) = text else {
            return false;
        };
        platform::set_clipboard(&text)
    }

    /// Zig `pasteInto`: inserts the clipboard text into the input.
    fn paste_into(&mut self, id: NodeId) {
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
}

/// Zig `hasControlChars`: a control character that must not be typed.
pub(crate) fn has_control_chars(text: &str) -> bool {
    text.bytes().any(|byte| {
        (byte < 0x20 && byte != b'\t' && byte != b'\n' && byte != b'\r') || byte == 0x7f
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_are_detected_like_zig() {
        assert!(!has_control_chars("hello"));
        assert!(!has_control_chars("a\tb\nc\rd"));
        assert!(!has_control_chars("héllo 中文"));
        assert!(has_control_chars("\u{1}"));
        assert!(has_control_chars("a\u{7f}b"));
        assert!(has_control_chars("\u{0}"));
    }
}
