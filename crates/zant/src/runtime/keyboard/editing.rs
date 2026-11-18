//! Editing keys, shortcuts, and clipboard operations for focused inputs.

use crate::core::events::{self, Key, KeyEvent};
use crate::core::node::{NodeData, NodeId};
use crate::core::tree::Move;
use crate::platform::platform;

use super::Runtime;

impl Runtime {
    /// Key handling of a focused input.
    pub(super) fn input_key(&mut self, id: NodeId, k: KeyEvent) -> i32 {
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
                    let mut app = self.app.borrow_mut();
                    let mut cx = super::action_cx(&mut app);
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
        let mut app = self.app.borrow_mut();
        let mut cx = super::action_cx(&mut app);
        action.call(&mut cx);
        self.needs_build = true;
        true
    }

    /// Puts the input's selection on the clipboard.
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

    /// Inserts the clipboard text into the input.
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
