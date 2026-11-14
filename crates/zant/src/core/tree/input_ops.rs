//! Port of `src/core/tree.zig` — WP-1.3: text / input / toggle state changes.
//!
//! **Ownership note.** Zig's `Tree.inputInsert` & co. delegate to
//! `components/input.zig` and invoke the stored callbacks from inside the tree
//! (`inputChanged`). In Rust the callbacks are `ui::actions` values that need
//! the runtime's `ActionCx`, so the tree only changes state and marks paint
//! dirty; the runtime clones the action out of the node and calls it
//! (plan.md §4). The editing helpers themselves live in
//! [`crate::components::input`] (WP-2.1); the methods below only look the node
//! up and call them.
#![forbid(unsafe_code)]

use crate::Str;
use crate::components::input;
use crate::core::geometry::Rect;
use crate::core::node::{NodeData, NodeId};
use crate::core::tree::Tree;

/// Zig `input.Move`: a caret movement. It lives here rather than in
/// `components::input` because the tree-level editing port (WP-1.3) declared it
/// first; `components::input` re-exports it under the Zig module's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Left,
    Right,
    Home,
    End,
    WordLeft,
    WordRight,
    Up,
    Down,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
}

impl Tree {
    pub fn set_text(&mut self, id: NodeId, value: &str) {
        let Some(data) = self.nodes[id as usize].text_mut() else {
            return;
        };
        data.content = Str::copy(value);
        data.invalidate();
        self.mark_dirty(id, true, true);
    }

    /// Updates a text label whose outer bounds are fixed by its parent. This is
    /// used by the live input read-out so typing does not trigger a tree layout.
    pub fn set_text_no_layout(&mut self, id: NodeId, value: &str) {
        let Some(data) = self.nodes[id as usize].text_mut() else {
            return;
        };
        data.content = Str::copy(value);
        data.invalidate();
        self.mark_dirty(id, false, true);
    }

    pub fn set_node_enabled(&mut self, id: NodeId, enabled: bool) {
        self.nodes[id as usize].enabled = enabled;
        match &mut self.nodes[id as usize].data {
            NodeData::Button(data) => {
                if !enabled {
                    data.hovered = false;
                    data.pressed = false;
                    data.focused = false;
                    data.hover_t = 0.0;
                    data.press_t = 0.0;
                }
                data.layout_dirty = true;
            }
            NodeData::Input(data) => {
                if !enabled {
                    data.focused = false;
                    data.hovered = false;
                }
            }
            _ => {}
        }
        self.mark_dirty(id, true, true);
    }

    // ── input editing (`components/input.zig`) ─────────────────────────────

    pub fn input_insert(&mut self, id: NodeId, value: &str) -> bool {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return false;
        };
        let changed = input::insert_text(data, value);
        if changed {
            self.input_changed(id);
        }
        changed
    }

    pub fn input_delete_backward(&mut self, id: NodeId) -> bool {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return false;
        };
        let changed = input::delete_backward(data);
        if changed {
            self.input_changed(id);
        }
        changed
    }

    pub fn input_delete_forward(&mut self, id: NodeId) -> bool {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return false;
        };
        let changed = input::delete_forward(data);
        if changed {
            self.input_changed(id);
        }
        changed
    }

    pub fn input_move(&mut self, id: NodeId, target: Move, extend: bool) {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return;
        };
        if target != Move::Up && target != Move::Down {
            data.preferred_x = None;
        }
        match target {
            Move::Left => input::move_left(data, extend),
            Move::Right => input::move_right(data, extend),
            Move::Home | Move::DocStart => input::move_home(data, extend),
            Move::End | Move::DocEnd => input::move_end(data, extend),
            Move::WordLeft => input::move_word_left(data, extend),
            Move::WordRight => input::move_word_right(data, extend),
            Move::Up => input::move_vertical(data, false, extend),
            Move::Down => input::move_vertical(data, true, extend),
            Move::LineStart => input::move_line_edge(data, false, extend),
            Move::LineEnd => input::move_line_edge(data, true, extend),
        }
        self.mark_dirty(id, false, true);
    }

    pub fn input_select_all(&mut self, id: NodeId) {
        if let Some(data) = self.nodes[id as usize].input_mut() {
            input::select_all(data);
        }
        self.mark_dirty(id, false, true);
    }

    /// Marks the paint dirty after an edit (the runtime calls the actions).
    pub fn input_changed(&mut self, id: NodeId) {
        // Inputs are single-line controls with a fixed outer size; editing
        // changes paint only and must not force a whole-tree layout on every
        // keystroke.
        self.mark_dirty(id, false, true);
    }

    pub fn input_composition(&mut self, id: NodeId, marked: &str, start: usize, end: usize) {
        if let Some(data) = self.nodes[id as usize].input_mut() {
            input::begin_composition(data, marked, start, end);
        }
        self.mark_dirty(id, false, true);
    }

    pub fn input_cancel_composition(&mut self, id: NodeId) {
        if let Some(data) = self.nodes[id as usize].input_mut() {
            input::cancel_composition(data);
        }
        self.mark_dirty(id, false, true);
    }

    pub fn input_set_value(&mut self, id: NodeId, value: &str) {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return;
        };
        data.text.clear();
        data.text.push_str(value);
        data.cursor = data.cursor.min(data.text.len());
        data.anchor = data.anchor.min(data.text.len());
        data.has_composition = false;
        data.composition.clear();
        data.rebuild_display();
        self.mark_dirty(id, false, true);
    }

    pub fn input_set_placeholder(&mut self, id: NodeId, value: &str) {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return;
        };
        data.placeholder = Str::copy(value);
        data.placeholder_layout = None;
        self.mark_dirty(id, false, true);
    }

    pub fn input_caret_rect(&mut self, id: NodeId) -> Option<Rect> {
        let bounds = self.nodes[id as usize].bounds;
        let data = match &mut self.nodes[id as usize].data {
            NodeData::Input(data) => data,
            _ => return None,
        };
        input::caret_rect(data, bounds)
    }

    // ── toggles ────────────────────────────────────────────────────────────

    /// Sets a toggle's checked state and queues its transition. Zig's
    /// `Tree.toggleSet` also called the `index_action` / `action`; the Rust
    /// runtime does that after cloning the callback out of the node
    /// (`Runtime::toggle_set` in `runtime/pointer.rs`, plan.md §4).
    pub fn toggle_set(&mut self, id: NodeId, checked: bool) {
        if !matches!(self.nodes[id as usize].data, NodeData::Toggle(_)) {
            return;
        }
        if let NodeData::Toggle(data) = &mut self.nodes[id as usize].data {
            data.checked = checked;
        }
        self.poke(id);
    }

    pub fn toggle_flip(&mut self, id: NodeId) {
        let NodeData::Toggle(data) = &self.nodes[id as usize].data else {
            return;
        };
        let checked = !data.checked;
        self.toggle_set(id, checked);
    }
}
