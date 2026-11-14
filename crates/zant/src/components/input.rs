//! The antd text input: measure, editing operations, caret and paint.
//!
//! The input is an antd-style text field: rounded shell with a border that
//! eases to `primary-hover` on hover and `primary` on focus (plus the soft
//! focus ring), the shaped value / placeholder, the selection highlight, the
//! caret and — for multiline inputs — the composition underline and a scroll
//! indicator. `Animated` blur/focus transitions of the shell are sampled by
//! [`animate`], the caret blink by [`update_blink`].
//!
//! The editing operations live here too (`insert_text`, `delete_backward`,
//! `move_*`, `begin_composition`, …). They are the single implementation:
//! `core/tree/input_ops.rs` (the tree-level state changes) and
//! `runtime/ime.rs` (the runtime helpers) delegate to the functions below.
//!
//! `bounds` / `enabled` are passed as values (`Rect`, `bool`) like the
//! neighbouring component modules, and `measure_width` / `measure_height`
//! take `&FlexStyle` like `components::{button,select,toggle}`.
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::node::{FlexStyle, InputData, NodeData, RetainedNode};
use crate::core::unicode;
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, TextLayout};
use crate::theme::tokens;
use crate::theme::tokens::Theme;
use zant_sys::skia::ZantRect;

/// The antd middle control width.
pub const DEFAULT_WIDTH: f32 = 220.0;
/// The antd middle control height.
pub const DEFAULT_HEIGHT: f32 = 32.0;
/// Horizontal padding between the shell and the text column.
pub const PADDING_X: f32 = 11.0;
/// Half period of the caret blink.
pub const BLINK_INTERVAL_MS: i64 = 530;

/// The editing command of `input_move`, defined next to the tree-level
/// editing operations in `core::tree`.
pub use crate::core::tree::Move;

/// The default height of an input.
pub fn control_height() -> f32 {
    DEFAULT_HEIGHT
}

/// Left edge of the text column (centred when
/// `content_max_width` is set).
pub fn text_left(data: &InputData, bounds: Rect) -> f32 {
    let full = (bounds.width - PADDING_X * 2.0).max(0.0);
    if data.content_max_width > 0.0 && full > data.content_max_width {
        return bounds.x + (bounds.width - data.content_max_width) * 0.5;
    }
    bounds.x + PADDING_X
}

/// Width of the text column.
pub fn text_width(data: &InputData, bounds: Rect) -> f32 {
    let full = (bounds.width - PADDING_X * 2.0).max(0.0);
    if data.content_max_width > 0.0 {
        return full.min(data.content_max_width);
    }
    full
}

/// The styled width, or the default control width.
pub fn measure_width(style: &FlexStyle) -> f32 {
    style.width.unwrap_or(DEFAULT_WIDTH)
}

/// The styled height, or the default control height.
pub fn measure_height(style: &FlexStyle) -> f32 {
    style.height.unwrap_or(DEFAULT_HEIGHT)
}

/// The selection as an ordered `(start, end)` byte range.
pub fn selection_range(data: &InputData) -> (usize, usize) {
    (data.cursor.min(data.anchor), data.cursor.max(data.anchor))
}

/// Whether a non-empty range is selected.
pub fn has_selection(data: &InputData) -> bool {
    data.cursor != data.anchor
}

/// Rebuilds the displayed text (value plus composition) from the editing state.
pub fn rebuild_display(data: &mut InputData) {
    data.rebuild_display();
}

/// Shapes `display_text` (cached until the next
/// invalidation). Returns false when shaping fails.
pub fn ensure_layout(data: &mut InputData) -> bool {
    if !data.layout_dirty && data.layout.is_some() {
        return true;
    }
    data.layout = None;
    let style = text::Style {
        font_size: data.font_size,
        weight: text::Weight::Regular,
        color: data.text_color,
        wrap: data.multiline,
        text_align: text::Align::Left,
        line_height: data.line_height,
        ..Default::default()
    };
    let max_width = if data.multiline {
        data.available_width.max(1.0)
    } else {
        1.0e6
    };
    match TextLayout::init(&data.display_text, style, max_width) {
        Ok(layout) => {
            data.layout = Some(layout);
            data.layout_dirty = false;
            true
        }
        Err(_) => false,
    }
}

/// Shapes the placeholder once and returns its cached layout.
fn ensure_placeholder(data: &mut InputData, theme: Theme) -> Option<&TextLayout> {
    if data.placeholder_layout.is_none() {
        let style = text::Style {
            font_size: data.font_size,
            weight: text::Weight::Regular,
            color: theme.color_text_disabled,
            wrap: false,
            line_height: data.line_height,
            ..Default::default()
        };
        match TextLayout::init(&data.placeholder, style, 1.0e6) {
            Ok(layout) => data.placeholder_layout = Some(layout),
            Err(_) => return None,
        }
    }
    data.placeholder_layout.as_ref()
}

/// Replaces the byte range `start..end` with `value`, puts the caret after it
/// and ends any composition. Returns false for an invalid range.
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
fn move_to(data: &mut InputData, target: usize, extend: bool) {
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

/// Paints the shell, the value or placeholder, the selection and the caret.
pub fn paint(node: &mut RetainedNode, canvas: Canvas, theme: Theme) {
    if !node.visible {
        return;
    }
    let rect = node.bounds;
    let enabled = node.enabled;
    let visual = node.visual;
    let data = match &mut node.data {
        NodeData::Input(data) => data,
        _ => return,
    };
    let desired_text_color = if enabled {
        theme.color_text
    } else {
        theme.color_text_disabled
    };
    if data.text_color != desired_text_color {
        data.text_color = desired_text_color;
        data.invalidate_layout();
    }
    let bg = visual.background.unwrap_or(if enabled {
        theme.color_bg_container
    } else {
        theme.color_fill_secondary
    });
    // antd: border eases to primary-hover on hover and primary on focus,
    // with a soft outline ring fading in
    let hover_t = if enabled { data.hover_t } else { 0.0 };
    let focus_t = if enabled { data.focus_t } else { 0.0 };
    let default_border = if !enabled {
        theme.color_border
    } else {
        tokens::lerp(
            tokens::lerp(theme.color_border, theme.color_primary_hover, hover_t),
            theme.color_primary,
            focus_t,
        )
    };
    let border = visual.border_color.unwrap_or(default_border);

    if focus_t > 0.01 && data.focus_ring {
        canvas.fill_round_rect(
            Rect::init(
                rect.x - 3.0,
                rect.y - 3.0,
                rect.width + 6.0,
                rect.height + 6.0,
            ),
            visual.radius + 3.0,
            tokens::with_alpha(theme.color_primary, (0x18 as f32 * focus_t) as u8),
        );
    }
    canvas.fill_round_rect(rect, visual.radius, bg);
    canvas.stroke_round_rect(
        Rect::init(
            rect.x + 0.5,
            rect.y + 0.5,
            (rect.width - 1.0).max(0.0),
            (rect.height - 1.0).max(0.0),
        ),
        visual.radius,
        border,
        1.0 + 0.5 * focus_t,
    );

    let new_width = text_width(data, rect);
    if data.multiline && (new_width - data.available_width).abs() > 0.5 {
        data.invalidate_layout();
    }
    data.available_width = new_width;
    let inner = Rect::init(
        text_left(data, rect),
        rect.y + 1.0,
        data.available_width,
        (rect.height - 2.0).max(0.0),
    );
    canvas.save();
    canvas.clip_rect(inner);

    if !ensure_layout(data) {
        canvas.restore();
        return;
    }
    ensure_caret_visible(data, inner.width, inner.height);
    let (text_y, origin_x) = {
        let layout = data.layout.as_ref().expect("layout was just ensured");
        let text_y = if data.multiline {
            rect.y + 8.0 - data.scroll_y
        } else {
            rect.y + (rect.height - layout.metrics().height) * 0.5
        };
        (text_y, text_left(data, rect) - data.scroll_x)
    };

    if data.text.is_empty() && !data.has_composition {
        let placeholder_x = text_left(data, rect);
        if let Some(placeholder) = ensure_placeholder(data, theme) {
            placeholder.paint(canvas, placeholder_x, text_y);
        }
    } else {
        if !data.has_composition && has_selection(data) {
            let (range_start, range_end) = selection_range(data);
            // only the visible part of the selection (long selections in the
            // editor would otherwise exceed the rect buffer)
            let (vis_start, vis_end) = {
                let layout = data.layout.as_ref().expect("layout was just ensured");
                if data.multiline {
                    (
                        layout.index_at(0.0, data.scroll_y - 2.0),
                        layout.index_at(1.0e6, data.scroll_y + inner.height + 2.0),
                    )
                } else {
                    (0, data.text.len())
                }
            };
            let sel_start = range_start.max(vis_start.min(range_end));
            let sel_end = range_end.min(vis_end.max(sel_start));
            if sel_end > sel_start {
                let layout = data.layout.as_ref().expect("layout was just ensured");
                let zero = ZantRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                };
                let mut rects = [zero; 256];
                let count = layout.range_rects(sel_start, sel_end, &mut rects);
                for native in &rects[..count] {
                    if native.width <= 0.0 || native.height <= 0.0 {
                        continue;
                    }
                    canvas.fill_rect(
                        Rect::init(
                            origin_x + native.x,
                            text_y + native.y,
                            native.width,
                            native.height,
                        ),
                        tokens::with_alpha(theme.color_primary, 0x2e),
                    );
                }
            }
        }
        {
            let layout = data.layout.as_ref().expect("layout was just ensured");
            layout.paint(canvas, origin_x, text_y);

            if data.has_composition && !data.composition.is_empty() {
                let start = data.cursor;
                let end = data.cursor + data.composition.len();
                let zero = ZantRect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                };
                let mut rects = [zero; 256];
                let count = layout.range_rects(start, end, &mut rects);
                for native in &rects[..count] {
                    canvas.draw_line(
                        origin_x + native.x,
                        text_y + native.y + native.height - 1.0,
                        origin_x + native.x + native.width,
                        text_y + native.y + native.height - 1.0,
                        theme.color_text,
                        1.0,
                    );
                }
            }
        }
    }

    if data.focused
        && enabled
        && data.blink_on
        && let Some(caret) = caret_rect(data, rect)
    {
        canvas.fill_rect(
            Rect::init(caret.x, caret.y, 1.5, caret.height),
            theme.color_text,
        );
    }

    canvas.restore();

    // overlay scroll indicator on the input's right edge
    if data.multiline {
        let view = (rect.height - 16.0).max(1.0);
        let max_scroll = max_scroll_y(data, view);
        if max_scroll > 0.5 {
            let content = view + max_scroll;
            let track = rect.height - 8.0;
            let thumb = (track * view / content).max(28.0);
            let y = rect.y + 4.0 + (track - thumb) * (data.scroll_y / max_scroll);
            canvas.fill_round_rect(
                Rect::init(rect.right() - 7.0, y, 3.5, thumb),
                1.75,
                tokens::with_alpha(theme.color_text, 0x40),
            );
        }
    }
}

/// Border / focus-ring transitions (antd 200ms ease-in-out).
/// Returns true while another frame is needed. The caret blink is driven
/// separately.
pub fn animate(data: &mut InputData, now: i64, theme: Theme) -> bool {
    let mid = motion::Transition {
        duration_ms: u16::try_from(theme.motion_duration_mid).expect("int cast"),
        easing: motion::Easing::EaseInOut,
        delay_ms: 0,
    };
    data.hover
        .retarget(if data.hovered { 1.0 } else { 0.0 }, now, mid);
    data.focus_anim
        .retarget(if data.focused { 1.0 } else { 0.0 }, now, mid);
    data.hover_t = data.hover.value(now);
    data.focus_t = data.focus_anim.value(now);
    !(data.hover.done(now) && data.focus_anim.done(now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeData;
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;

    const CLEAR: u32 = 0xff00_ff00;
    const WIDTH: usize = 240;
    const HEIGHT: usize = 64;

    fn input_node() -> RetainedNode {
        let mut node = RetainedNode::new(0, NodeData::Input(InputData::default()));
        node.bounds = Rect::init(10.0, 10.0, 200.0, 32.0);
        node.visual.radius = 6.0;
        node.focusable = true;
        node
    }

    fn paint_into(node: &mut RetainedNode) -> Renderer {
        let mut renderer =
            Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            paint(node, canvas, ant_light());
        }
        renderer.end();
        renderer
    }

    fn data(node: &mut RetainedNode) -> &mut InputData {
        match &mut node.data {
            NodeData::Input(data) => data,
            _ => panic!("not an input node"),
        }
    }

    #[test]
    fn measure_width_and_height_use_the_default_control_size() {
        assert_eq!(measure_width(&FlexStyle::default()), 220.0);
        assert_eq!(measure_height(&FlexStyle::default()), 32.0);
        assert_eq!(control_height(), 32.0);
        let style = FlexStyle {
            width: Some(240.0),
            height: Some(40.0),
            ..Default::default()
        };
        assert_eq!(measure_width(&style), 240.0);
        assert_eq!(measure_height(&style), 40.0);
    }

    #[test]
    fn text_column_honours_padding_and_content_max_width() {
        let bounds = Rect::init(10.0, 10.0, 200.0, 32.0);
        let data = InputData::default();
        assert_eq!(text_left(&data, bounds), 21.0);
        assert_eq!(text_width(&data, bounds), 178.0);
        let centred = InputData {
            content_max_width: 100.0,
            ..Default::default()
        };
        assert_eq!(text_left(&centred, bounds), 10.0 + 50.0);
        assert_eq!(text_width(&centred, bounds), 100.0);
        // a content width wider than the field keeps the padding
        let wide = InputData {
            content_max_width: 400.0,
            ..Default::default()
        };
        assert_eq!(text_left(&wide, bounds), 21.0);
        assert_eq!(text_width(&wide, bounds), 178.0);
    }

    #[test]
    fn paint_draws_the_shell_value_and_centres_the_text() {
        let theme = ant_light();
        let mut node = input_node();
        data(&mut node).text = String::from("Zant UI");
        data(&mut node).text_color = theme.color_text;
        data(&mut node).rebuild_display();
        let renderer = paint_into(&mut node);
        // inside the shell: the container background
        assert_eq!(renderer.pixel_at(20, 26), Some(theme.color_bg_container));
        // a rounded corner stays clear
        assert_eq!(renderer.pixel_at(10, 10), Some(CLEAR));
        // the value darkens some pixels of the text column
        let ink = (0..32)
            .flat_map(|y| (0..178).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                renderer.pixel_at(21 + x as usize, 11 + y as usize)
                    != Some(theme.color_bg_container)
            })
            .count();
        assert!(ink > 20, "the value is painted ({ink} px)");
    }

    #[test]
    fn paint_shows_the_placeholder_when_empty() {
        let theme = ant_light();
        let mut node = input_node();
        data(&mut node).placeholder = crate::Str::Static("Type something…");
        let renderer = paint_into(&mut node);
        let ink = (0..32)
            .flat_map(|y| (0..178).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                renderer.pixel_at(21 + x as usize, 11 + y as usize)
                    != Some(theme.color_bg_container)
            })
            .count();
        assert!(ink > 20, "the placeholder is painted ({ink} px)");
    }

    #[test]
    fn paint_draws_the_caret_only_when_focused_and_blinking() {
        let mut blink_off = input_node();
        data(&mut blink_off).text = String::from("ab");
        data(&mut blink_off).text_color = ant_light().color_text;
        data(&mut blink_off).rebuild_display();
        let unfocused = paint_into(&mut blink_off);

        let mut blink_on = input_node();
        {
            let data = data(&mut blink_on);
            data.text = String::from("ab");
            data.text_color = ant_light().color_text;
            data.focused = true;
            data.blink_on = true;
            data.rebuild_display();
        }
        let focused = paint_into(&mut blink_on);

        let mut differing = 0;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if focused.pixel_at(x, y) != unfocused.pixel_at(x, y) {
                    differing += 1;
                }
            }
        }
        assert!(differing > 3, "the caret is painted ({differing} px)");
    }

    #[test]
    fn update_blink_toggles_every_blink_interval() {
        let mut data = InputData {
            focused: true,
            ..Default::default()
        };
        assert!(!update_blink(&mut data, true, 1_000));
        assert!(data.blink_on);
        assert_eq!(data.next_blink_ms, 1_000 + BLINK_INTERVAL_MS);
        assert!(!update_blink(&mut data, true, 1_500));
        assert!(update_blink(&mut data, true, 1_000 + BLINK_INTERVAL_MS));
        assert!(!data.blink_on);
        // losing focus resets the blink
        assert!(!update_blink(&mut data, false, 9_999));
        assert!(data.blink_on);
        assert_eq!(data.next_blink_ms, 0);
    }

    #[test]
    fn editing_helpers_keep_the_caret_and_selection_consistent() {
        let mut data = InputData::default();
        assert!(insert_text(&mut data, "abc"));
        assert_eq!(data.text, "abc");
        assert_eq!((data.cursor, data.anchor), (3, 3));
        select_all(&mut data);
        assert_eq!((data.cursor, data.anchor), (3, 0));
        assert!(has_selection(&data));
        // typing replaces the selection
        assert!(insert_text(&mut data, "Z"));
        assert_eq!(data.text, "Z");
        assert_eq!((data.cursor, data.anchor), (1, 1));
        // backward / forward deletion
        assert!(insert_text(&mut data, "ant"));
        assert_eq!(data.text, "Zant");
        assert!(delete_backward(&mut data));
        assert_eq!(data.text, "Zan");
        move_home(&mut data, false);
        assert!(delete_forward(&mut data));
        assert_eq!(data.text, "an");
        // read-only inputs reject edits
        data.read_only = true;
        assert!(!insert_text(&mut data, "x"));
        assert!(!delete_backward(&mut data));
    }

    #[test]
    fn animate_retargets_the_shell_transitions() {
        crate::motion::motion::set_enabled(true);
        let theme = ant_light();
        let mut data = InputData {
            hovered: true,
            ..Default::default()
        };
        // the first sample starts the transition; the target is not reached
        assert!(animate(&mut data, 1_000, theme));
        assert!(data.hover_t < 1.0);
        let done = !animate(
            &mut data,
            1_000 + i64::from(theme.motion_duration_mid),
            theme,
        );
        assert!(done, "the transition settles after its duration");
        assert_eq!(data.hover_t, 1.0);
    }
}
