//! Port of `src/core/tree.zig` — WP-1.3: text / input / toggle state changes.
//!
//! **Ownership note.** Zig's `Tree.inputInsert` & co. delegate to
//! `components/input.zig` (WP-2.1) and invoke the stored callbacks from inside
//! the tree. In Rust the callbacks are `ui::actions` values that need the
//! runtime's `ActionCx`, so the tree only changes state and marks paint dirty;
//! the runtime clones the action out of the node and calls it (plan.md §4).
//! The editing helpers of `components/input.zig` (`replaceRange`,
//! `deleteBackward`, `moveVertical`, `caretRect`, …) are ported here because
//! WP-2.1's `components/input.rs` does not exist yet; when it lands those
//! helpers should move there and these methods should delegate to it.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Rect;
use crate::core::node::{InputData, NodeData, NodeId};
use crate::core::tree::Tree;
use crate::core::unicode;
use crate::renderer::text::{self, TextLayout};

/// Zig `input.Move`: a caret movement.
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

/// `components/input.zig:padding_x`.
const INPUT_PADDING_X: f32 = 11.0;

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

    // ── input editing (see the ownership note) ─────────────────────────────

    pub fn input_insert(&mut self, id: NodeId, value: &str) -> bool {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return false;
        };
        let changed = input_insert_text(data, value);
        if changed {
            self.input_changed(id);
        }
        changed
    }

    pub fn input_delete_backward(&mut self, id: NodeId) -> bool {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return false;
        };
        let changed = input_delete_backward(data);
        if changed {
            self.input_changed(id);
        }
        changed
    }

    pub fn input_delete_forward(&mut self, id: NodeId) -> bool {
        let Some(data) = self.nodes[id as usize].input_mut() else {
            return false;
        };
        let changed = input_delete_forward(data);
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
            Move::Left => input_move_left(data, extend),
            Move::Right => input_move_right(data, extend),
            Move::Home | Move::DocStart => input_move_home(data, extend),
            Move::End | Move::DocEnd => input_move_end(data, extend),
            Move::WordLeft => input_move_word_left(data, extend),
            Move::WordRight => input_move_word_right(data, extend),
            Move::Up => input_move_vertical(data, false, extend),
            Move::Down => input_move_vertical(data, true, extend),
            Move::LineStart => input_move_line_edge(data, false, extend),
            Move::LineEnd => input_move_line_edge(data, true, extend),
        }
        self.mark_dirty(id, false, true);
    }

    pub fn input_select_all(&mut self, id: NodeId) {
        if let Some(data) = self.nodes[id as usize].input_mut() {
            input_select_all(data);
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
            input_begin_composition(data, marked, start, end);
        }
        self.mark_dirty(id, false, true);
    }

    pub fn input_cancel_composition(&mut self, id: NodeId) {
        if let Some(data) = self.nodes[id as usize].input_mut() {
            input_cancel_composition(data);
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
        input_ensure_layout(data);
        input_caret_rect_readonly(data, bounds)
    }

    // ── toggles ────────────────────────────────────────────────────────────

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

// ── `components/input.zig` helpers (temporary home, see module docs) ───────

fn selection_range(data: &InputData) -> (usize, usize) {
    (data.cursor.min(data.anchor), data.cursor.max(data.anchor))
}

fn has_selection(data: &InputData) -> bool {
    data.cursor != data.anchor
}

fn replace_range(data: &mut InputData, start: usize, end: usize, value: &str) -> bool {
    if start > end || end > data.text.len() {
        return false;
    }
    if !data.text.is_char_boundary(start) || !data.text.is_char_boundary(end) {
        return false;
    }
    data.text.replace_range(start..end, value);
    let cursor = start + value.len();
    data.cursor = cursor.min(data.text.len());
    data.anchor = data.cursor;
    data.preferred_x = None;
    data.reveal_caret = true;
    data.has_composition = false;
    data.composition.clear();
    data.composition_cursor = 0;
    data.rebuild_display();
    true
}

fn input_insert_text(data: &mut InputData, value: &str) -> bool {
    if data.read_only || value.is_empty() {
        return false;
    }
    if data.has_composition {
        data.has_composition = false;
        data.composition.clear();
        data.composition_cursor = 0;
    }
    let (start, end) = selection_range(data);
    replace_range(data, start, end, value)
}

fn input_delete_selection(data: &mut InputData) -> bool {
    if data.read_only {
        return false;
    }
    if !has_selection(data) {
        return false;
    }
    let (start, end) = selection_range(data);
    replace_range(data, start, end, "")
}

fn input_delete_backward(data: &mut InputData) -> bool {
    if data.read_only {
        return false;
    }
    if has_selection(data) {
        return input_delete_selection(data);
    }
    if data.cursor == 0 {
        return false;
    }
    let start = unicode::prev_grapheme(&data.text, data.cursor);
    replace_range(data, start, data.cursor, "")
}

fn input_delete_forward(data: &mut InputData) -> bool {
    if data.read_only {
        return false;
    }
    if has_selection(data) {
        return input_delete_selection(data);
    }
    if data.cursor >= data.text.len() {
        return false;
    }
    let end = unicode::next_grapheme(&data.text, data.cursor);
    replace_range(data, data.cursor, end, "")
}

fn input_move_to(data: &mut InputData, target: usize, extend: bool) {
    let clamped = target.min(data.text.len());
    data.reveal_caret = true;
    data.cursor = clamped;
    if !extend {
        data.anchor = clamped;
    }
}

fn input_move_left(data: &mut InputData, extend: bool) {
    let target = unicode::prev_grapheme(&data.text, data.cursor);
    input_move_to(data, target, extend);
}

fn input_move_right(data: &mut InputData, extend: bool) {
    let target = unicode::next_grapheme(&data.text, data.cursor);
    input_move_to(data, target, extend);
}

fn input_move_home(data: &mut InputData, extend: bool) {
    input_move_to(data, 0, extend);
}

fn input_move_end(data: &mut InputData, extend: bool) {
    let len = data.text.len();
    input_move_to(data, len, extend);
}

fn input_move_word_left(data: &mut InputData, extend: bool) {
    let target = unicode::word_start(&data.text, data.cursor);
    input_move_to(data, target, extend);
}

fn input_move_word_right(data: &mut InputData, extend: bool) {
    let target = unicode::word_end(&data.text, data.cursor);
    input_move_to(data, target, extend);
}

/// `input.zig:ensureLayout`.
fn input_ensure_layout(data: &mut InputData) -> Option<&TextLayout> {
    if !data.layout_dirty && data.layout.is_some() {
        return data.layout.as_ref();
    }
    let style = text::Style {
        font_size: data.font_size,
        weight: text::Weight::Regular,
        color: data.text_color,
        wrap: data.multiline,
        text_align: text::Align::Left,
        line_height: data.line_height,
        ..text::Style::default()
    };
    let max_width = if data.multiline {
        data.available_width.max(1.0)
    } else {
        1.0e6
    };
    let layout = TextLayout::init(&data.display_text, style, max_width).ok()?;
    data.layout = Some(layout);
    data.layout_dirty = false;
    data.layout.as_ref()
}

/// Moves the caret one visual line up or down, keeping its column.
fn input_move_vertical(data: &mut InputData, down: bool, extend: bool) {
    let cursor = data.cursor;
    let (caret_x, caret_y, caret_height, layout_height) = {
        let Some(layout) = input_ensure_layout(data) else {
            return;
        };
        let Some(caret) = layout.caret_rect(cursor) else {
            return;
        };
        (caret.x, caret.y, caret.height, layout.metrics().height)
    };
    let x = data.preferred_x.unwrap_or(caret_x);
    data.preferred_x = Some(x);
    let y = if down {
        caret_y + caret_height + 1.0
    } else {
        caret_y - 1.0
    };
    if y < 0.0 {
        input_move_to(data, 0, extend);
        return;
    }
    if y >= layout_height {
        let len = data.text.len();
        input_move_to(data, len, extend);
        return;
    }
    let target = {
        let Some(layout) = input_ensure_layout(data) else {
            return;
        };
        layout.index_at(x, y).min(data.text.len())
    };
    input_move_to(data, target, extend);
}

/// Start (`end == false`) or end of the visual line holding the caret.
fn input_move_line_edge(data: &mut InputData, end: bool, extend: bool) {
    let cursor = data.cursor;
    let y = {
        let Some(layout) = input_ensure_layout(data) else {
            return;
        };
        let Some(caret) = layout.caret_rect(cursor) else {
            return;
        };
        caret.y + caret.height * 0.5
    };
    let target = {
        let Some(layout) = input_ensure_layout(data) else {
            return;
        };
        layout
            .index_at(if end { 1.0e6 } else { 0.0 }, y)
            .min(data.text.len())
    };
    input_move_to(data, target, extend);
}

fn input_select_all(data: &mut InputData) {
    data.anchor = 0;
    data.cursor = data.text.len();
}

fn input_begin_composition(
    data: &mut InputData,
    marked: &str,
    selection_start_utf16: usize,
    selection_end_utf16: usize,
) {
    let _ = selection_start_utf16;
    if !data.has_composition && has_selection(data) {
        let _ = input_delete_selection(data);
    }
    data.has_composition = !marked.is_empty();
    data.composition.clear();
    data.composition.push_str(marked);
    data.composition_cursor = unicode::utf16_to_byte(marked, selection_end_utf16);
    data.anchor = data.cursor;
    data.reveal_caret = true;
    data.rebuild_display();
}

fn input_cancel_composition(data: &mut InputData) {
    if !data.has_composition && data.composition.is_empty() {
        return;
    }
    data.has_composition = false;
    data.composition.clear();
    data.composition_cursor = 0;
    data.rebuild_display();
}

/// `input.zig:textLeft`: left edge of the text column.
fn input_text_left(data: &InputData, bounds: Rect) -> f32 {
    let full = (bounds.width - INPUT_PADDING_X * 2.0).max(0.0);
    if data.content_max_width > 0.0 && full > data.content_max_width {
        return bounds.x + (bounds.width - data.content_max_width) * 0.5;
    }
    bounds.x + INPUT_PADDING_X
}

/// `input.zig:caretRect` (read-only: the layout may be refreshed via
/// `input_ensure_layout` by the caller when needed).
fn input_caret_rect_readonly(data: &InputData, bounds: Rect) -> Option<Rect> {
    let layout = data.layout.as_ref()?;
    let caret_byte = if data.has_composition {
        data.cursor + data.composition_cursor
    } else {
        data.cursor
    };
    let local = layout.caret_rect(caret_byte)?;
    let x = input_text_left(data, bounds) - data.scroll_x + local.x;
    let line_top = (if data.multiline {
        bounds.y + 8.0 - data.scroll_y
    } else {
        bounds.y + (bounds.height - layout.metrics().height) * 0.5
    }) + local.y;
    // Skia reports the whole line box; with a tall line height (e.g. 2.0)
    // that would make the caret twice the text height. Keep the caret at the
    // font's own height and sit it on the baseline: the line box grows
    // proportionally around ascent/descent (~80% / 20%).
    let line_h = local.height.max(data.font_size);
    let caret_h = line_h.min(data.font_size * 1.3);
    let baseline = line_top + line_h * 0.8;
    Some(Rect::init(
        x,
        baseline - caret_h * 0.8,
        local.width.max(1.5),
        caret_h,
    ))
}
