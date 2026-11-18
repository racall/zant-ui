//! Painting the input and its state transitions.

use super::*;

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
