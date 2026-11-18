//! Caret geometry, scrolling the caret into view and the blink.

use super::*;

/// The largest vertical scroll offset for a view of `view_height`.
pub fn max_scroll_y(data: &mut InputData, view_height: f32) -> f32 {
    if !ensure_layout(data) {
        return 0.0;
    }
    let height = data
        .layout
        .as_ref()
        .map_or(0.0, |layout| layout.metrics().height);
    (height - view_height.max(1.0)).max(0.0)
}

/// Scrolls the content so the caret stays inside the view.
pub fn ensure_caret_visible(data: &mut InputData, available_width: f32, view_height: f32) {
    if !ensure_layout(data) {
        return;
    }
    let (caret, longest_line) = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        let caret_byte = if data.has_composition {
            data.cursor + data.composition_cursor
        } else {
            data.cursor
        };
        let Some(caret) = layout.caret_rect(caret_byte) else {
            return;
        };
        (caret, layout.metrics().longest_line)
    };
    let width = available_width.max(1.0);
    // Only follow the caret after an edit / caret move / controlled
    // selection; otherwise just clamp, so the user can scroll away freely.
    if !data.reveal_caret {
        let max_x = (longest_line - width + 2.0).max(0.0);
        data.scroll_x = data.scroll_x.min(max_x).max(0.0);
        if data.multiline {
            data.scroll_y = data.scroll_y.min(max_scroll_y(data, view_height)).max(0.0);
        }
        return;
    }
    data.reveal_caret = false;
    if caret.x - data.scroll_x > width - 2.0 {
        data.scroll_x = caret.x - width + 2.0;
    } else if caret.x - data.scroll_x < 0.0 {
        data.scroll_x = (caret.x - 2.0).max(0.0);
    }
    let max_scroll = (longest_line - width + 2.0).max(0.0);
    data.scroll_x = data.scroll_x.min(max_scroll).max(0.0);
    if data.multiline {
        let height = view_height.max(1.0);
        if caret.y + caret.height - data.scroll_y > height {
            data.scroll_y = caret.y + caret.height - height;
        } else if caret.y - data.scroll_y < 0.0 {
            data.scroll_y = caret.y;
        }
        data.scroll_y = data.scroll_y.min(max_scroll_y(data, height)).max(0.0);
    } else {
        data.scroll_y = 0.0;
    }
}

/// The caret rectangle in window space, if the layout is available.
pub fn caret_rect(data: &mut InputData, bounds: Rect) -> Option<Rect> {
    if !ensure_layout(data) {
        return None;
    }
    let layout = data.layout.as_ref()?;
    let caret_byte = if data.has_composition {
        data.cursor + data.composition_cursor
    } else {
        data.cursor
    };
    let local = layout.caret_rect(caret_byte)?;
    let x = text_left(data, bounds) - data.scroll_x + local.x;
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

/// The caret blink; true when the blink state flipped and
/// the input must repaint.
pub fn update_blink(data: &mut InputData, enabled: bool, now_ms: i64) -> bool {
    if !data.focused || !enabled {
        data.blink_on = true;
        data.next_blink_ms = 0;
        return false;
    }
    if data.next_blink_ms == 0 {
        data.blink_on = true;
        data.next_blink_ms = now_ms + BLINK_INTERVAL_MS;
        return false;
    }
    if now_ms >= data.next_blink_ms {
        data.blink_on = !data.blink_on;
        data.next_blink_ms = now_ms + BLINK_INTERVAL_MS;
        return true;
    }
    false
}
