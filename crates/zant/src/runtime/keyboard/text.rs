//! Text input filtering, insertion, and edit notifications.

use crate::core::node::{NodeData, NodeId};

use super::{Runtime, has_control_chars};

impl Runtime {
    /// Types text into the focused input, dropping
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

    /// Calls the input's `text_action`. The tree only changes state, so the
    /// runtime notifies the action here after every successful edit.
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
        let mut app = self.app.borrow_mut();
        let mut cx = super::action_cx(&mut app);
        action.call(&mut cx, &data.text);
    }
}
