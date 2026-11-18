//! The antd checkbox / switch / radio: paint, measure and transitions.
//!
//! Measures and paints the indicator (16px box or 40×22 track), the checkmark /
//! dot / handle, the keyboard focus ring and the label. The transition values
//! (`hover_t`, `press_t`, `check_t`, `mark_t`) are animated by the runtime
//! through [`animate`]; painting never starts a transition.
//!
//! Runtime-side (not in this module): hover / press / focus state changes and
//! `Tree::toggle_set` / `toggle_flip` on click or Space / Enter
//! (`core/tree/input_ops.rs`). The runtime must reset `hovered` / `pressed` /
//! `hover` / `press` when a toggle becomes disabled (toggles have no
//! `set_enabled` helper, unlike buttons).
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::node::{FlexStyle, NodeData, RetainedNode, ToggleData, ToggleKind};
use crate::motion::{easing, motion};
use crate::renderer::canvas::Canvas;
use crate::theme::tokens;
use crate::theme::tokens::Theme;

#[cfg(test)]
mod tests;

/// Side of the checkbox / radio indicator.
pub const CHECKBOX_SIZE: f32 = 16.0;

/// Width of the switch track.
pub const SWITCH_WIDTH: f32 = 40.0;

/// Height of the switch track.
pub const SWITCH_HEIGHT: f32 = 22.0;

/// Gap between the indicator and the label.
pub const LABEL_GAP: f32 = 8.0;

/// The antd control height of a toggle row.
pub const CONTROL_HEIGHT: f32 = 24.0;

/// The label colour and the colour the layout is shaped with; measuring and
/// painting must agree or every frame re-shapes.
pub fn label_color(enabled: bool, theme: Theme) -> u32 {
    if enabled {
        theme.color_text
    } else {
        theme.color_text_disabled
    }
}

/// The antd control height of a toggle row ([`CONTROL_HEIGHT`]).
pub fn control_height() -> f32 {
    CONTROL_HEIGHT
}

/// Label font size (`ToggleData::font_size`).
pub fn font_size(data: &ToggleData) -> f32 {
    data.font_size
}

/// The indicator plus the label's longest line, 8px apart.
pub fn measure_width(
    data: &mut ToggleData,
    style: &FlexStyle,
    enabled: bool,
    theme: Theme,
    max_width: f32,
) -> f32 {
    let _ = max_width;
    if let Some(width) = style.width {
        return width;
    }
    let indicator = match data.kind {
        ToggleKind::Checkbox | ToggleKind::Radio => CHECKBOX_SIZE,
        ToggleKind::SwitchControl => SWITCH_WIDTH,
    };
    // no label: just the control (an empty paragraph's line width is not a
    // number, which made a bare switch measure 0 wide)
    if data.label.is_empty() {
        return indicator;
    }
    let layout = match data.ensure_layout(label_color(enabled, theme)) {
        Some(layout) => layout,
        None => return indicator,
    };
    let line = layout.metrics().longest_line;
    indicator + LABEL_GAP + if line.is_finite() { line } else { 0.0 }
}

/// The style height, else the control height.
pub fn measure_height(style: &FlexStyle) -> f32 {
    style.height.unwrap_or(CONTROL_HEIGHT)
}

fn transition(duration_ms: u16, easing: easing::Easing) -> motion::Transition {
    motion::Transition {
        duration_ms,
        easing,
        delay_ms: 0,
    }
}

pub fn paint(node: &mut RetainedNode, canvas: Canvas, theme: Theme) {
    if !node.visible {
        return;
    }
    let enabled = node.enabled;
    let rect = node.bounds;
    let focus_visible = match &node.data {
        NodeData::Toggle(data) => data.focus_visible,
        _ => false,
    };
    let data = match &mut node.data {
        NodeData::Toggle(data) => data,
        _ => return,
    };
    let color = label_color(enabled, theme);
    let hover = if enabled { data.hover_t } else { 0.0 };
    let press = if enabled { data.press_t } else { 0.0 };
    let check = data.check_t;
    let mark = data.mark_t;

    match data.kind {
        ToggleKind::Radio => {
            let box_rect = Rect::init(
                rect.x,
                rect.y + (rect.height - CHECKBOX_SIZE) * 0.5,
                CHECKBOX_SIZE,
                CHECKBOX_SIZE,
            );
            let border = if enabled {
                tokens::lerp(
                    tokens::lerp(theme.color_border, theme.color_primary_hover, hover),
                    theme.color_primary,
                    check,
                )
            } else {
                theme.color_border
            };
            canvas.fill_round_rect(
                box_rect,
                CHECKBOX_SIZE * 0.5,
                if enabled {
                    theme.color_bg_container
                } else {
                    theme.color_fill_secondary
                },
            );
            canvas.stroke_round_rect(box_rect, CHECKBOX_SIZE * 0.5, border, 1.2);
            if mark > 0.02 {
                // antd: the inner dot scales in (dotSize = 8px of 16)
                let dot = CHECKBOX_SIZE * 0.25 * mark;
                let cx = box_rect.x + box_rect.width * 0.5;
                let cy = box_rect.y + box_rect.height * 0.5;
                canvas.fill_round_rect(
                    Rect::init(cx - dot, cy - dot, dot * 2.0, dot * 2.0),
                    dot,
                    theme.color_primary,
                );
            }
            if focus_visible && enabled {
                canvas.stroke_round_rect(
                    Rect::init(
                        box_rect.x - 2.0,
                        box_rect.y - 2.0,
                        box_rect.width + 4.0,
                        box_rect.height + 4.0,
                    ),
                    CHECKBOX_SIZE * 0.5 + 2.0,
                    tokens::with_alpha(theme.color_primary, 0x66),
                    2.0,
                );
            }
            if let Some(layout) = data.ensure_layout(color) {
                layout.paint(
                    canvas,
                    rect.x + CHECKBOX_SIZE + LABEL_GAP,
                    rect.y + (rect.height - layout.metrics().height) * 0.5,
                );
            }
        }
        ToggleKind::Checkbox => {
            let box_rect = Rect::init(
                rect.x,
                rect.y + (rect.height - CHECKBOX_SIZE) * 0.5,
                CHECKBOX_SIZE,
                CHECKBOX_SIZE,
            );
            // disabled + checked keeps a dimmed primary fill so the mark stays visible
            let fill = if enabled {
                tokens::lerp(theme.color_bg_container, theme.color_primary, check)
            } else if check > 0.5 {
                tokens::with_alpha(theme.color_primary, 0x66)
            } else {
                theme.color_fill_secondary
            };
            let border = if enabled {
                tokens::lerp(
                    tokens::lerp(theme.color_border, theme.color_primary_hover, hover),
                    theme.color_primary,
                    check,
                )
            } else {
                theme.color_border
            };
            canvas.fill_round_rect(box_rect, theme.border_radius_sm * 0.5 + 2.0, fill);
            canvas.stroke_round_rect(box_rect, theme.border_radius_sm * 0.5 + 2.0, border, 1.0);
            if mark > 0.02 {
                // the tick pops in from the centre (antd ease-out-back)
                let check_color = tokens::with_alpha(
                    theme.color_text_light,
                    (255.0 * mark.clamp(0.0, 1.0)) as u8,
                );
                let cx = box_rect.x + box_rect.width * 0.5;
                let cy = box_rect.y + box_rect.height * 0.5;
                let k = mark;
                canvas.draw_line(
                    cx - 3.5 * k,
                    cy,
                    cx - 1.0 * k,
                    cy + 2.5 * k,
                    check_color,
                    1.8,
                );
                canvas.draw_line(
                    cx - 1.0 * k,
                    cy + 2.5 * k,
                    cx + 4.0 * k,
                    cy - 3.0 * k,
                    check_color,
                    1.8,
                );
            }
            if focus_visible && enabled {
                canvas.stroke_round_rect(
                    Rect::init(
                        box_rect.x - 2.0,
                        box_rect.y - 2.0,
                        box_rect.width + 4.0,
                        box_rect.height + 4.0,
                    ),
                    theme.border_radius_sm,
                    tokens::with_alpha(theme.color_primary, 0x66),
                    2.0,
                );
            }
            if let Some(layout) = data.ensure_layout(color) {
                let x = rect.x + CHECKBOX_SIZE + LABEL_GAP;
                let y = rect.y + (rect.height - layout.metrics().height) * 0.5;
                layout.paint(canvas, x, y);
            }
        }
        ToggleKind::SwitchControl => {
            let track = Rect::init(
                rect.x,
                rect.y + (rect.height - SWITCH_HEIGHT) * 0.5,
                SWITCH_WIDTH,
                SWITCH_HEIGHT,
            );
            // antd: unchecked track = colorTextQuaternary (darker on hover),
            // checked = primary (primary_hover on hover); disabled at 65% opacity
            let off = tokens::lerp(
                theme.color_text_disabled,
                theme.color_text_secondary,
                hover * 0.5,
            );
            let on = tokens::lerp(theme.color_primary, theme.color_primary_hover, hover);
            let mut track_color = tokens::lerp(off, on, check);
            if !enabled {
                track_color = tokens::with_alpha(
                    track_color,
                    (f32::from(tokens::alpha(track_color)) * 0.65) as u8,
                );
            }
            canvas.fill_round_rect(track, SWITCH_HEIGHT * 0.5, track_color);
            // pressing stretches the handle towards the centre (antd 18 → 24px)
            let base: f32 = SWITCH_HEIGHT - 4.0;
            let knob_w = base + 5.4 * press;
            let travel = SWITCH_WIDTH - 4.0 - base;
            let left = track.x + 2.0 + travel * check;
            let knob_x = left - (knob_w - base) * check;
            let knob = Rect::init(knob_x, track.y + 2.0, knob_w, base);
            // the handle is always white (antd handleBg)
            canvas.fill_round_rect(knob, base * 0.5, 0xffff_ffff);
            if focus_visible && enabled {
                canvas.stroke_round_rect(
                    Rect::init(
                        track.x - 2.0,
                        track.y - 2.0,
                        track.width + 4.0,
                        track.height + 4.0,
                    ),
                    (track.height + 4.0) * 0.5,
                    tokens::with_alpha(theme.color_primary, 0x44),
                    2.0,
                );
            }
            if let Some(layout) = data.ensure_layout(color) {
                let x = rect.x + SWITCH_WIDTH + LABEL_GAP;
                let y = rect.y + (rect.height - layout.metrics().height) * 0.5;
                layout.paint(canvas, x, y);
            }
        }
    }
}

/// Advances the toggle's transitions (antd): checkbox fill 300ms ease and
/// tick 200ms ease-out-back (in) / 100ms ease-in-back (out); radio dot
/// 300ms ease-in-out-circ; switch 200ms ease-in-out. Returns true while
/// another frame is needed.
pub fn animate(data: &mut ToggleData, now: i64, theme: Theme) -> bool {
    let mid = transition(theme.motion_duration_mid as u16, easing::Easing::EaseInOut);
    let fast = transition(theme.motion_duration_fast as u16, easing::Easing::EaseInOut);
    let target: f32 = if data.checked { 1.0 } else { 0.0 };
    data.hover
        .retarget(if data.hovered { 1.0 } else { 0.0 }, now, mid);
    data.press
        .retarget(if data.pressed { 1.0 } else { 0.0 }, now, fast);
    match data.kind {
        ToggleKind::Checkbox => {
            data.check.retarget(
                target,
                now,
                transition(theme.motion_duration_slow as u16, easing::Easing::Ease),
            );
            let mark_tr = if data.checked {
                motion::Transition {
                    duration_ms: 200,
                    easing: easing::Easing::EaseOutBack,
                    delay_ms: 100,
                }
            } else {
                motion::Transition {
                    duration_ms: 100,
                    easing: easing::Easing::EaseInBack,
                    delay_ms: 0,
                }
            };
            data.mark.retarget(target, now, mark_tr);
        }
        ToggleKind::Radio => {
            data.check.retarget(target, now, mid);
            data.mark.retarget(
                target,
                now,
                transition(
                    theme.motion_duration_slow as u16,
                    easing::Easing::EaseInOutCirc,
                ),
            );
        }
        ToggleKind::SwitchControl => {
            data.check.retarget(target, now, mid);
            data.mark.retarget(target, now, mid);
        }
    }
    data.hover_t = data.hover.value(now);
    data.press_t = data.press.value(now);
    data.check_t = data.check.value(now);
    data.mark_t = data.mark.value(now);
    !(data.hover.done(now) && data.press.done(now) && data.check.done(now) && data.mark.done(now))
}
