//! Port of `src/components/toggle.zig` — WP-2.2: antd checkbox / switch / radio
//! paint, measure and transitions.
//!
//! Measures and paints the indicator (16px box or 40×22 track), the checkmark /
//! dot / handle, the keyboard focus ring and the label. The transition values
//! (`hover_t`, `press_t`, `check_t`, `mark_t`) are animated by the runtime
//! through [`animate`]; painting never starts a transition.
//!
//! Runtime-side (not in this module, Zig keeps it in `ui_runtime.zig`):
//! hover / press / focus state changes and `Tree::toggle_set` / `toggle_flip`
//! on click or Space / Enter (`core/tree/input_ops.rs`). The runtime must reset
//! `hovered` / `pressed` / `hover` / `press` when a toggle becomes disabled the
//! same way `ui_runtime.zig:applyCommon` does (there is no Zig `setEnabled`
//! helper for toggles, unlike buttons).
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::node::{FlexStyle, NodeData, RetainedNode, ToggleData, ToggleKind};
use crate::motion::{easing, motion};
use crate::renderer::canvas::Canvas;
use crate::theme::tokens;
use crate::theme::tokens::Theme;

/// Zig: `checkbox_size`.
pub const CHECKBOX_SIZE: f32 = 16.0;
/// Zig: `switch_width`.
pub const SWITCH_WIDTH: f32 = 40.0;
/// Zig: `switch_height`.
pub const SWITCH_HEIGHT: f32 = 22.0;
/// Zig: `label_gap`.
pub const LABEL_GAP: f32 = 8.0;
/// Zig: `control_height`.
pub const CONTROL_HEIGHT: f32 = 24.0;

/// Zig: `labelColor` (the label colour and the colour the layout is shaped
/// with; measuring and painting must agree or every frame re-shapes).
pub fn label_color(enabled: bool, theme: Theme) -> u32 {
    if enabled {
        theme.color_text
    } else {
        theme.color_text_disabled
    }
}

/// The antd control height of a toggle row (Zig: the `control_height` const).
pub fn control_height() -> f32 {
    CONTROL_HEIGHT
}

/// Label font size (`ToggleData::font_size`).
pub fn font_size(data: &ToggleData) -> f32 {
    data.font_size
}

/// Zig `measureWidth`: the indicator plus the label's longest line, 8px apart.
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

/// Zig `measureHeight`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Str;
    use crate::core::node::{NodeData, RetainedNode, ToggleData, ToggleKind};
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;

    /// Clear colour: any painted pixel is visibly different from it.
    const CLEAR: u32 = 0xff00_ff00;

    fn node(kind: ToggleKind, checked: bool) -> RetainedNode {
        let v = if checked { 1.0 } else { 0.0 };
        let data = ToggleData {
            label: Str::from("Label"),
            kind,
            checked,
            check: motion::Animated::at(v),
            mark: motion::Animated::at(v),
            check_t: v,
            mark_t: v,
            ..Default::default()
        };
        let mut node = RetainedNode::new(0, NodeData::Toggle(data));
        node.bounds = Rect::init(8.0, 4.0, 160.0, CONTROL_HEIGHT);
        node
    }

    fn paint_into(node: &mut RetainedNode, width: usize, height: usize, theme: Theme) -> Renderer {
        let mut renderer =
            Renderer::create_raster(width as u32, height as u32).expect("raster renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            paint(node, canvas, theme);
        }
        renderer.end();
        renderer
    }

    fn changed_pixels(a: &Renderer, b: &Renderer, width: usize, height: usize) -> usize {
        let mut changed = 0;
        for y in 0..height {
            for x in 0..width {
                if a.pixel_at(x, y) != b.pixel_at(x, y) {
                    changed += 1;
                }
            }
        }
        changed
    }

    #[test]
    fn checkbox_paints_its_fill_and_tick() {
        let theme = ant_light();
        let mut unchecked = node(ToggleKind::Checkbox, false);
        let plain = paint_into(&mut unchecked, 160, 32, theme);
        // 16px box at (8, 4 + (24 - 16) / 2 = 8); (13, 13) is inside the fill
        assert_eq!(plain.pixel_at(13, 13), Some(theme.color_bg_container));

        let mut checked = node(ToggleKind::Checkbox, true);
        let ticked = paint_into(&mut checked, 160, 32, theme);
        assert_eq!(ticked.pixel_at(13, 13), Some(theme.color_primary));

        // the white tick only shows once the mark animation has started
        let mut no_mark = node(ToggleKind::Checkbox, true);
        no_mark.toggle_mut().expect("toggle").mark_t = 0.0;
        let bare = paint_into(&mut no_mark, 160, 32, theme);
        assert!(changed_pixels(&ticked, &bare, 160, 32) > 0);
        assert_eq!(bare.pixel_at(13, 13), Some(theme.color_primary));
    }

    #[test]
    fn switch_paints_its_track_and_handle() {
        let theme = ant_light();
        // track is 40×22 at (8, 5); the handle is 18 wide at the left / right
        let mut unchecked = node(ToggleKind::SwitchControl, false);
        let off = paint_into(&mut unchecked, 160, 32, theme);
        // knob sits at the left when off: the right end of the track is the
        // (disabled-text coloured) off track
        let off_pixel = off.pixel_at(8 + 35, 4 + 12).expect("pixel");
        assert_ne!(off_pixel, CLEAR);
        assert_ne!(off_pixel, 0xffff_ffff);

        let mut checked = node(ToggleKind::SwitchControl, true);
        let on = paint_into(&mut checked, 160, 32, theme);
        // checked: handle moved right, track left of it is primary
        assert_eq!(on.pixel_at(8 + 4, 4 + 12), Some(theme.color_primary));
        assert_eq!(on.pixel_at(8 + 35, 4 + 12), Some(0xffff_ffff));
        assert_ne!(off_pixel, on.pixel_at(8 + 35, 4 + 12).expect("pixel"));
    }

    #[test]
    fn radio_paints_its_dot() {
        let theme = ant_light();
        let mut unchecked = node(ToggleKind::Radio, false);
        let empty = paint_into(&mut unchecked, 160, 32, theme);
        assert_eq!(empty.pixel_at(16, 16), Some(theme.color_bg_container));

        let mut checked = node(ToggleKind::Radio, true);
        let dotted = paint_into(&mut checked, 160, 32, theme);
        // dotSize = 8 of a 16px box, so the centre 4px are primary
        assert_eq!(dotted.pixel_at(16, 16), Some(theme.color_primary));
        assert!(changed_pixels(&empty, &dotted, 160, 32) > 0);
    }

    #[test]
    fn hover_and_focus_change_pixels() {
        let theme = ant_light();
        let mut plain = node(ToggleKind::Checkbox, false);
        let base = paint_into(&mut plain, 160, 32, theme);

        let mut hovered = node(ToggleKind::Checkbox, false);
        hovered.toggle_mut().expect("toggle").hover_t = 1.0;
        let hover = paint_into(&mut hovered, 160, 32, theme);
        assert!(
            changed_pixels(&base, &hover, 160, 32) > 0,
            "hover must brighten the border"
        );

        let mut focused = node(ToggleKind::Checkbox, false);
        focused.toggle_mut().expect("toggle").focus_visible = true;
        let focus = paint_into(&mut focused, 160, 32, theme);
        assert!(
            changed_pixels(&base, &focus, 160, 32) > 0,
            "keyboard focus draws a ring outside the box"
        );
    }

    #[test]
    fn disabled_checkbox_keeps_a_dimmed_primary_fill() {
        let theme = ant_light();
        let mut checked = node(ToggleKind::Checkbox, true);
        let enabled = paint_into(&mut checked, 160, 32, theme);
        let mut disabled = node(ToggleKind::Checkbox, true);
        disabled.enabled = false;
        let dimmed = paint_into(&mut disabled, 160, 32, theme);
        assert_ne!(dimmed.pixel_at(13, 13), enabled.pixel_at(13, 13));
        // the fill is the primary colour at 40 % over the background
        let mut reference =
            Renderer::create_raster(160, 32).expect("raster renderer for the reference fill");
        {
            let canvas = reference.begin(1.0).expect("reference canvas");
            canvas.clear(CLEAR);
            canvas.fill_rect(
                Rect::init(0.0, 0.0, 160.0, 32.0),
                tokens::with_alpha(theme.color_primary, 0x66),
            );
        }
        reference.end();
        assert_eq!(dimmed.pixel_at(13, 13), reference.pixel_at(13, 13));
    }

    #[test]
    fn checkbox_checked_animation_interpolates() {
        let theme = ant_light();
        let mut data = ToggleData {
            checked: true,
            ..Default::default()
        };

        assert!(
            animate(&mut data, 1_000, theme),
            "a transition just started"
        );
        assert_eq!(data.check_t, 0.0);
        assert_eq!(data.mark_t, 0.0);

        // halfway through the 300ms fill
        animate(&mut data, 1_150, theme);
        assert!(data.check_t > 0.0 && data.check_t < 1.0);

        // well past every transition: settled at the checked state
        assert!(!animate(&mut data, 10_000, theme));
        assert_eq!(data.check_t, 1.0);
        assert_eq!(data.mark_t, 1.0);
        assert_eq!(data.hover_t, 0.0);
    }

    #[test]
    fn switch_animate_retargets_check_on_flip() {
        let theme = ant_light();
        let mut data = ToggleData {
            kind: ToggleKind::SwitchControl,
            ..Default::default()
        };
        animate(&mut data, 1_000, theme);
        assert_eq!(data.check_t, 0.0);
        data.checked = true;
        assert!(animate(&mut data, 1_000, theme));
        animate(&mut data, 10_000, theme);
        assert_eq!(data.check_t, 1.0);
    }

    #[test]
    fn tree_paint_dispatches_to_the_toggle_module() {
        use crate::core::geometry::Size;
        use crate::core::tree::Tree;
        let theme = ant_light();
        let mut tree = Tree::init(theme);
        let root = tree.root;
        tree.add_toggle(Some(root), "A", ToggleKind::Checkbox, true);
        tree.layout(Size {
            width: 200.0,
            height: 60.0,
        });
        let mut renderer = Renderer::create_raster(200, 60).expect("raster renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            tree.paint(canvas);
        }
        renderer.end();
        // the 16px box at (0, 4); (5, 5) is its primary fill (no tick there)
        assert_eq!(renderer.pixel_at(5, 5), Some(theme.color_primary));
    }

    #[test]
    fn measure_width_and_height_follow_the_label_and_style() {
        let theme = ant_light();
        let style = FlexStyle::default();
        // no label: just the indicator (Zig special-cases the empty paragraph)
        let mut bare = ToggleData {
            kind: ToggleKind::Checkbox,
            ..Default::default()
        };
        assert_eq!(
            measure_width(&mut bare, &style, true, theme, 1000.0),
            CHECKBOX_SIZE
        );
        let mut switch = ToggleData {
            kind: ToggleKind::SwitchControl,
            ..Default::default()
        };
        assert_eq!(
            measure_width(&mut switch, &style, true, theme, 1000.0),
            SWITCH_WIDTH
        );
        // a label adds its longest line plus the 8px gap
        let mut labelled = ToggleData {
            label: Str::from("Save"),
            ..Default::default()
        };
        let width = measure_width(&mut labelled, &style, true, theme, 1000.0);
        assert!(width > CHECKBOX_SIZE + LABEL_GAP);
        // explicit style sizes win
        let sized = FlexStyle {
            width: Some(200.0),
            height: Some(40.0),
            ..Default::default()
        };
        assert_eq!(
            measure_width(&mut labelled, &sized, true, theme, 0.0),
            200.0
        );
        assert_eq!(measure_height(&sized), 40.0);
        assert_eq!(measure_height(&style), CONTROL_HEIGHT);
    }
}
