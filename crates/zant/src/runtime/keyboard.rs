//! Keyboard and text input.
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

    /// Key handling of a focused input.
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

/// A control character that must not be typed.
pub(crate) fn has_control_chars(text: &str) -> bool {
    text.bytes().any(|byte| {
        (byte < 0x20 && byte != b'\t' && byte != b'\n' && byte != b'\r') || byte == 0x7f
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_are_detected() {
        assert!(!has_control_chars("hello"));
        assert!(!has_control_chars("a\tb\nc\rd"));
        assert!(!has_control_chars("héllo 中文"));
        assert!(has_control_chars("\u{1}"));
        assert!(has_control_chars("a\u{7f}b"));
        assert!(has_control_chars("\u{0}"));
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;
    use crate::core::events::{Event, Key, KeyEvent, Modifiers, PointerButton, PointerEvent};
    use crate::core::node::{FlexStyle, NodeKind};
    use crate::renderer::renderer::Renderer;
    use crate::ui::RunOptions;
    use crate::ui::actions::{action, bool_action, index_action, text_action};
    use crate::ui::app::App;
    use crate::ui::builders::{checkbox, column, select};
    use crate::ui::element::{Element, Node};
    use crate::ui::options::{BoxOptions, SelectOptions, ToggleOptions};

    struct Form {
        text: String,
        edits: u32,
        submits: u32,
        undos: u32,
        redos: u32,
    }

    impl App for Form {
        fn render(&mut self) -> Node {
            let text = self.text.clone();
            let input = Node::new(Element {
                kind: NodeKind::Input,
                content: crate::Str::copy(&text),
                placeholder: "name".into(),
                focusable: true,
                layout: FlexStyle {
                    width: Some(150.0),
                    height: Some(32.0),
                    shrink: 0.0,
                    ..Default::default()
                },
                on_text: Some(text_action(|state: &mut Form, value| {
                    state.text = value.to_string();
                    state.edits += 1;
                })),
                on_click: Some(action(|state: &mut Form| state.submits += 1)),
                on_undo: Some(action(|state: &mut Form| state.undos += 1)),
                on_redo: Some(action(|state: &mut Form| state.redos += 1)),
                ..Element::default()
            });
            column(BoxOptions {
                children: crate::nodes![input],
                ..Default::default()
            })
        }
    }

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 200.0, 120.0, 1.0);
        renderer.end();
    }

    fn key(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    fn primary(key: Key, shift: bool) -> KeyEvent {
        let mut modifiers = Modifiers::default();
        if crate::core::keys::IS_MACOS {
            modifiers.meta = true;
        } else {
            modifiers.control = true;
        }
        modifiers.shift = shift;
        KeyEvent {
            key,
            modifiers,
            repeat: false,
        }
    }

    fn focused_form() -> (Runtime, Renderer) {
        let mut runtime = Runtime::init(
            Form {
                text: String::new(),
                edits: 0,
                submits: 0,
                undos: 0,
                redos: 0,
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let id = runtime
            .tree
            .nodes
            .iter()
            .find(|node| node.alive && node.kind() == NodeKind::Input)
            .map(|node| node.id)
            .expect("input");
        let bounds = runtime.tree.get(id).bounds;
        let point = PointerEvent {
            x: bounds.x + 20.0,
            y: bounds.y + bounds.height * 0.5,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        };
        runtime.dispatch(Event::PointerMove(point));
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
        assert_eq!(runtime.tree.focused, Some(id));
        (runtime, renderer)
    }

    #[test]
    fn control_characters_are_never_typed_and_the_action_runs_once() {
        let (mut runtime, _renderer) = focused_form();
        assert_eq!(
            runtime.dispatch(Event::TextInput(String::from("\u{1}ab\u{7f}c"))),
            1
        );
        assert_eq!(runtime.app_state_mut::<Form>().text, "abc");
        assert_eq!(runtime.app_state_mut::<Form>().edits, 1);
        // a value of only control characters is dropped
        assert_eq!(
            runtime.dispatch(Event::TextInput(String::from("\u{1}\u{7f}"))),
            0
        );
        assert_eq!(runtime.app_state_mut::<Form>().text, "abc");
    }

    #[test]
    fn backspace_and_delete_edit_the_focused_input_through_the_tree() {
        let (mut runtime, _renderer) = focused_form();
        runtime.dispatch(Event::TextInput(String::from("abc")));
        runtime.dispatch(Event::KeyDown(key(Key::Backspace)));
        assert_eq!(runtime.app_state_mut::<Form>().text, "ab");
        // Delete at the end of the text changes nothing
        runtime.dispatch(Event::KeyDown(key(Key::Delete)));
        assert_eq!(runtime.app_state_mut::<Form>().text, "ab");
        // Left moves the caret, Delete then removes the following character
        runtime.dispatch(Event::KeyDown(key(Key::Left)));
        runtime.dispatch(Event::KeyDown(key(Key::Delete)));
        assert_eq!(runtime.app_state_mut::<Form>().text, "a");
        assert_eq!(runtime.app_state_mut::<Form>().edits, 3);
    }

    #[test]
    fn enter_submits_single_line_inputs() {
        let (mut runtime, _renderer) = focused_form();
        assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Enter))), 1);
        assert_eq!(runtime.app_state_mut::<Form>().submits, 1);
    }

    #[test]
    fn undo_and_redo_hooks_run_for_the_platform_shortcuts() {
        let (mut runtime, _renderer) = focused_form();
        assert_eq!(
            runtime.dispatch(Event::KeyDown(primary(Key::Character('z'), false))),
            1
        );
        assert_eq!(runtime.app_state_mut::<Form>().undos, 1);
        assert_eq!(
            runtime.dispatch(Event::KeyDown(primary(Key::Character('z'), true))),
            1
        );
        assert_eq!(runtime.app_state_mut::<Form>().redos, 1);
        assert_eq!(
            runtime.dispatch(Event::KeyDown(primary(Key::Character('y'), false))),
            1
        );
        assert_eq!(runtime.app_state_mut::<Form>().redos, 2);
        // cmd/ctrl+A selects the whole text
        assert_eq!(
            runtime.dispatch(Event::KeyDown(primary(Key::Character('a'), false))),
            1
        );
        let id = runtime.tree.focused.expect("input");
        match &runtime.tree.get(id).data {
            NodeData::Input(data) => {
                assert_eq!(data.anchor, 0);
                assert_eq!(data.cursor, data.text.len());
            }
            _ => panic!("expected an input"),
        }
    }

    // ── keyboard activation of toggles and selects ─────────────────────────

    #[derive(Default)]
    struct Options {
        checked: bool,
        changes: u32,
        selected: usize,
        picks: u32,
    }

    impl App for Options {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![
                    checkbox(ToggleOptions {
                        key: Some("notify".into()),
                        label: "Notify".into(),
                        checked: self.checked,
                        on_change: Some(bool_action(|state: &mut Options, checked| {
                            state.checked = checked;
                            state.changes += 1;
                        })),
                        ..Default::default()
                    }),
                    select(SelectOptions {
                        key: Some("plan".into()),
                        options: ["Starter", "Pro", "Enterprise"]
                            .iter()
                            .map(|label| crate::Str::from(*label))
                            .collect(),
                        selected: self.selected,
                        width: Some(150.0),
                        on_change: Some(index_action(|state: &mut Options, index| {
                            state.selected = index;
                            state.picks += 1;
                        })),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            })
        }
    }

    fn options_runtime() -> (Runtime, Renderer) {
        let mut runtime = Runtime::init(Options::default(), RunOptions::default());
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer);
        (runtime, renderer)
    }

    #[test]
    fn space_and_enter_activate_a_focused_toggle_once_each() {
        let (mut runtime, _renderer) = options_runtime();
        assert_eq!(runtime.tree.focused, None);
        runtime.dispatch(Event::KeyDown(key(Key::Tab)));
        let toggle = runtime.tree.focused.expect("Tab focuses the checkbox");
        assert!(matches!(
            &runtime.tree.get(toggle).data,
            NodeData::Toggle(_)
        ));
        runtime.dispatch(Event::KeyDown(key(Key::Character(' '))));
        assert!(runtime.app_state_mut::<Options>().checked);
        assert_eq!(runtime.app_state_mut::<Options>().changes, 1);
        runtime.dispatch(Event::KeyDown(key(Key::Enter)));
        assert!(!runtime.app_state_mut::<Options>().checked);
        assert_eq!(
            runtime.app_state_mut::<Options>().changes,
            2,
            "one callback per key press"
        );
    }

    #[test]
    fn enter_opens_the_popup_and_arrows_move_the_selection() {
        let (mut runtime, _renderer) = options_runtime();
        // Tab reaches the checkbox first and the select second
        runtime.dispatch(Event::KeyDown(key(Key::Tab)));
        runtime.dispatch(Event::KeyDown(key(Key::Tab)));
        let select_id = runtime.find_key("plan").expect("select");
        assert_eq!(runtime.tree.focused, Some(select_id));
        assert_eq!(
            runtime.dispatch(Event::KeyDown(key(Key::Enter))),
            1,
            "Enter opens the popup"
        );
        assert_eq!(runtime.open_select, Some(select_id));
        assert!(matches!(&runtime.tree.get(select_id).data, NodeData::Select(data) if data.open));
        // Down / Up move the selected option and fire the index action
        runtime.dispatch(Event::KeyDown(key(Key::Down)));
        assert_eq!(runtime.app_state_mut::<Options>().selected, 1);
        assert_eq!(runtime.app_state_mut::<Options>().picks, 1);
        runtime.dispatch(Event::KeyDown(key(Key::Down)));
        assert_eq!(runtime.app_state_mut::<Options>().selected, 2);
        runtime.dispatch(Event::KeyDown(key(Key::Up)));
        assert_eq!(runtime.app_state_mut::<Options>().selected, 1);
        assert_eq!(runtime.app_state_mut::<Options>().picks, 3);
        // Escape closes the popup
        runtime.dispatch(Event::KeyDown(key(Key::Escape)));
        assert_eq!(runtime.open_select, None);
        assert!(matches!(&runtime.tree.get(select_id).data, NodeData::Select(data) if !data.open));
    }

    struct Dialogs {
        outer: bool,
        inner: bool,
        closed: Vec<&'static str>,
    }

    impl App for Dialogs {
        fn render(&mut self) -> Node {
            let mut children = Vec::new();
            for (open, name) in [(self.outer, "outer"), (self.inner, "inner")] {
                if open {
                    children.push(crate::components::modal::modal(
                        crate::components::modal::ModalOptions {
                            key: name.into(),
                            title: name.into(),
                            on_close: Some(action(move |s: &mut Dialogs| {
                                s.closed.push(name);
                                if name == "inner" {
                                    s.inner = false;
                                } else {
                                    s.outer = false;
                                }
                            })),
                            ..Default::default()
                        },
                    ));
                }
            }
            column(BoxOptions {
                children,
                ..Default::default()
            })
        }
    }

    /// Esc closes the topmost open modal first, then the one below.
    #[test]
    fn escape_closes_the_topmost_modal() {
        let mut runtime = Runtime::init(
            Dialogs {
                outer: true,
                inner: true,
                closed: Vec::new(),
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(200, 120).expect("renderer");
        frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.dispatch(Event::KeyDown(key(Key::Escape))), 1);
        frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.app_state_mut::<Dialogs>().closed, ["inner"]);
        runtime.dispatch(Event::KeyDown(key(Key::Escape)));
        frame(&mut runtime, &mut renderer);
        assert_eq!(
            runtime.app_state_mut::<Dialogs>().closed,
            ["inner", "outer"]
        );
        // nothing open: Esc only drops the focus
        runtime.dispatch(Event::KeyDown(key(Key::Escape)));
        assert_eq!(runtime.app_state_mut::<Dialogs>().closed.len(), 2);
    }
}
