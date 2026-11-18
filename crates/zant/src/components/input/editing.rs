//! Editing the input's text: insertion, deletion, caret moves, selection and IME composition.

use super::*;

/// Replaces the byte range `start..end` with `value`, puts the caret after it
/// and ends any composition. Returns false for an invalid range.
pub(super) fn replace_range(data: &mut InputData, start: usize, end: usize, value: &str) -> bool {
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
    rebuild_display(data);
    true
}

/// Inserts `value` at the caret, replacing the selection.
pub fn insert_text(data: &mut InputData, value: &str) -> bool {
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

/// Deletes the selected range; false when nothing is selected.
pub fn delete_selection(data: &mut InputData) -> bool {
    if data.read_only {
        return false;
    }
    if !has_selection(data) {
        return false;
    }
    let (start, end) = selection_range(data);
    replace_range(data, start, end, "")
}

/// Deletes the selection, or the character before the caret.
pub fn delete_backward(data: &mut InputData) -> bool {
    if data.read_only {
        return false;
    }
    if has_selection(data) {
        return delete_selection(data);
    }
    if data.cursor == 0 {
        return false;
    }
    let start = unicode::prev_grapheme(&data.text, data.cursor);
    replace_range(data, start, data.cursor, "")
}

/// Deletes the selection, or the character after the caret.
pub fn delete_forward(data: &mut InputData) -> bool {
    if data.read_only {
        return false;
    }
    if has_selection(data) {
        return delete_selection(data);
    }
    if data.cursor >= data.text.len() {
        return false;
    }
    let end = unicode::next_grapheme(&data.text, data.cursor);
    replace_range(data, data.cursor, end, "")
}

/// Moves the caret to `target`, extending the selection when `extend` is set.
pub(super) fn move_to(data: &mut InputData, target: usize, extend: bool) {
    let clamped = target.min(data.text.len());
    data.reveal_caret = true;
    data.cursor = clamped;
    if !extend {
        data.anchor = clamped;
    }
}

/// Moves the caret one character to the left.
pub fn move_left(data: &mut InputData, extend: bool) {
    move_to(
        data,
        unicode::prev_grapheme(&data.text, data.cursor),
        extend,
    );
}

/// Moves the caret one character to the right.
pub fn move_right(data: &mut InputData, extend: bool) {
    move_to(
        data,
        unicode::next_grapheme(&data.text, data.cursor),
        extend,
    );
}

/// Moves the caret to the start of the text.
pub fn move_home(data: &mut InputData, extend: bool) {
    move_to(data, 0, extend);
}

/// Moves the caret to the end of the text.
pub fn move_end(data: &mut InputData, extend: bool) {
    move_to(data, data.text.len(), extend);
}

/// Moves the caret to the previous word boundary.
pub fn move_word_left(data: &mut InputData, extend: bool) {
    move_to(data, unicode::word_start(&data.text, data.cursor), extend);
}

/// Moves the caret to the next word boundary.
pub fn move_word_right(data: &mut InputData, extend: bool) {
    move_to(data, unicode::word_end(&data.text, data.cursor), extend);
}

/// Moves the caret one visual line up or down, keeping its
/// column.
pub fn move_vertical(data: &mut InputData, down: bool, extend: bool) {
    if !ensure_layout(data) {
        return;
    }
    let (local, metrics_height) = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        let Some(local) = layout.caret_rect(data.cursor) else {
            return;
        };
        (local, layout.metrics().height)
    };
    let x = data.preferred_x.unwrap_or(local.x);
    data.preferred_x = Some(x);
    let y = if down {
        local.y + local.height + 1.0
    } else {
        local.y - 1.0
    };
    if y < 0.0 {
        return move_to(data, 0, extend);
    }
    if y >= metrics_height {
        return move_to(data, data.text.len(), extend);
    }
    let byte = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        layout.index_at(x, y)
    };
    move_to(data, byte.min(data.text.len()), extend);
}

/// Start (`end == false`) or end of the visual line holding
/// the caret.
pub fn move_line_edge(data: &mut InputData, end: bool, extend: bool) {
    if !ensure_layout(data) {
        return;
    }
    let y = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        let Some(caret) = layout.caret_rect(data.cursor) else {
            return;
        };
        caret.y + caret.height * 0.5
    };
    let byte = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        layout.index_at(if end { 1.0e6 } else { 0.0 }, y)
    };
    move_to(data, byte.min(data.text.len()), extend);
}

/// Selects the whole text.
pub fn select_all(data: &mut InputData) {
    data.anchor = 0;
    data.cursor = data.text.len();
}

/// Places the caret at a window point.
pub fn cursor_from_point(data: &mut InputData, bounds: Rect, x: f32, y: f32) {
    if !ensure_layout(data) {
        return;
    }
    let byte = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        let inner_x = text_left(data, bounds) - data.scroll_x;
        let inner_y = if data.multiline {
            bounds.y + 8.0
        } else {
            bounds.y + (bounds.height - layout.metrics().height) * 0.5
        };
        layout.index_at(x - inner_x, y - inner_y)
    };
    data.cursor = byte.min(data.text.len());
    data.anchor = data.cursor;
    data.preferred_x = None;
    data.reveal_caret = true;
}

/// Starts or updates the IME composition with the marked text.
pub fn begin_composition(
    data: &mut InputData,
    marked: &str,
    selection_start_utf16: usize,
    selection_end_utf16: usize,
) {
    let _ = selection_start_utf16;
    if !data.has_composition && has_selection(data) {
        let _ = delete_selection(data);
    }
    data.has_composition = !marked.is_empty();
    data.composition.clear();
    data.composition.push_str(marked);
    data.composition_cursor = unicode::utf16_to_byte(marked, selection_end_utf16);
    data.anchor = data.cursor;
    data.reveal_caret = true;
    rebuild_display(data);
}

/// Drops the IME composition without committing it.
pub fn cancel_composition(data: &mut InputData) {
    if !data.has_composition && data.composition.is_empty() {
        return;
    }
    data.has_composition = false;
    data.composition.clear();
    data.composition_cursor = 0;
    rebuild_display(data);
}
