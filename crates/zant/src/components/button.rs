//! Port of `src/components/button.zig` — WP-1.3: antd button paint / measure.
//!
//! Measures and paints the label (optionally with a leading icon), the antd
//! variant fills and borders, the keyboard focus ring and the click wave. The
//! transition values (`hover_t`, `press_t`, `wave_t`) are animated by the
//! runtime through [`animate`]; painting never starts a transition.
#![forbid(unsafe_code)]

use crate::core::geometry::Rect;
use crate::core::icons;
use crate::core::icons::Icon;
use crate::core::node::{ButtonData, ButtonSize, ButtonType, FlexStyle, NodeData, RetainedNode};
use crate::motion::easing;
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::renderer::text;
use crate::theme::tokens;
use crate::theme::tokens::Theme;

pub const HORIZONTAL_PADDING_MIDDLE: f32 = 15.0;
pub const HORIZONTAL_PADDING_SMALL: f32 = 7.0;
pub const HORIZONTAL_PADDING_LARGE: f32 = 15.0;

const ICON_GAP: f32 = 8.0;

/// Windows 11 caption buttons: close hover #C42B1C, pressed at 90 %.
const CAPTION_CLOSE_HOVER: u32 = 0xffc4_2b1c;
const CAPTION_CLOSE_PRESSED: u32 = 0xe6c4_2b1c;

pub fn control_height(size: ButtonSize) -> f32 {
    match size {
        ButtonSize::Small => 24.0,
        ButtonSize::Middle => 32.0,
        ButtonSize::Large => 40.0,
    }
}

pub fn horizontal_padding(size: ButtonSize, button_type: ButtonType) -> f32 {
    if button_type == ButtonType::Text
        || button_type == ButtonType::Link
        || button_type == ButtonType::Plain
        || is_caption(button_type)
    {
        return 0.0;
    }
    match size {
        ButtonSize::Small => HORIZONTAL_PADDING_SMALL,
        ButtonSize::Middle => HORIZONTAL_PADDING_MIDDLE,
        ButtonSize::Large => HORIZONTAL_PADDING_LARGE,
    }
}

pub fn font_size(size: ButtonSize) -> f32 {
    match size {
        ButtonSize::Small => 12.0,
        ButtonSize::Middle => 14.0,
        ButtonSize::Large => 16.0,
    }
}

fn is_caption(button_type: ButtonType) -> bool {
    button_type == ButtonType::Caption || button_type == ButtonType::CaptionClose
}

fn text_color(data: &ButtonData, enabled: bool, theme: Theme) -> u32 {
    if !enabled {
        return theme.color_text_disabled;
    }
    match data.button_type {
        ButtonType::Primary => theme.color_text_light,
        ButtonType::Default => theme.color_text,
        ButtonType::Text => theme.color_text,
        ButtonType::Link => theme.color_primary,
        ButtonType::Plain | ButtonType::Caption | ButtonType::CaptionClose => theme.color_text,
    }
}

pub fn measure_width(
    data: &mut ButtonData,
    style: &FlexStyle,
    enabled: bool,
    theme: Theme,
    max_width: f32,
) -> f32 {
    let _ = max_width;
    if let Some(width) = style.width {
        return width;
    }
    // same colour as paint, so measuring does not re-shape the label
    let color = if enabled {
        data.text_color
            .unwrap_or_else(|| text_color(data, true, theme))
    } else {
        text_color(data, false, theme)
    };
    // antd: an icon before the label, 8px apart
    let icon = if has_icon_and_label(data) {
        label_icon_size(data) + ICON_GAP
    } else {
        0.0
    };
    let padding = horizontal_padding(data.size, data.button_type);
    let Some(layout) = data.ensure_layout(color) else {
        return control_height(data.size) * 2.0;
    };
    (icon + layout.metrics().longest_line + padding * 2.0)
        .min(style.max_width.unwrap_or(f32::INFINITY))
}

pub fn measure_height(style: &FlexStyle, data: &ButtonData) -> f32 {
    if let Some(height) = style.height {
        return height;
    }
    control_height(data.size)
}

struct BaseColors {
    bg: u32,
    hover_bg: u32,
    active_bg: u32,
    border: u32,
    hover_border: u32,
    active_border: u32,
}

fn base_colors(data: &ButtonData, enabled: bool, theme: Theme) -> BaseColors {
    if !enabled {
        // antd: disabled text / link buttons have no fill or border
        if data.button_type == ButtonType::Text
            || data.button_type == ButtonType::Link
            || data.button_type == ButtonType::Plain
            || is_caption(data.button_type)
        {
            let clear = tokens::with_alpha(theme.color_fill_secondary, 0);
            return BaseColors {
                bg: clear,
                hover_bg: clear,
                active_bg: clear,
                border: 0,
                hover_border: 0,
                active_border: 0,
            };
        }
        return BaseColors {
            bg: theme.color_fill_secondary,
            hover_bg: theme.color_fill_secondary,
            active_bg: theme.color_fill_secondary,
            border: theme.color_border,
            hover_border: theme.color_border,
            active_border: theme.color_border,
        };
    }
    match data.button_type {
        ButtonType::Primary => BaseColors {
            bg: theme.color_primary,
            hover_bg: theme.color_primary_hover,
            active_bg: theme.color_primary_active,
            border: theme.color_primary,
            hover_border: theme.color_primary_hover,
            active_border: theme.color_primary_active,
        },
        ButtonType::Default => BaseColors {
            bg: theme.color_bg_container,
            hover_bg: theme.color_bg_container,
            active_bg: theme.color_bg_container,
            border: theme.color_border,
            hover_border: theme.color_primary_hover,
            active_border: theme.color_primary_active,
        },
        // antd: colorBgTextHover = fillSecondary, colorBgTextActive = fill
        ButtonType::Text => BaseColors {
            bg: tokens::with_alpha(theme.color_fill_secondary, 0),
            hover_bg: theme.color_fill_secondary,
            active_bg: theme.color_fill,
            border: 0x0000_0000,
            hover_border: 0x0000_0000,
            active_border: 0x0000_0000,
        },
        // Windows caption buttons: a faint fill of the text colour
        ButtonType::Caption => BaseColors {
            bg: tokens::with_alpha(theme.color_text, 0),
            hover_bg: tokens::with_alpha(theme.color_text, 0x12),
            active_bg: tokens::with_alpha(theme.color_text, 0x0c),
            border: 0x0000_0000,
            hover_border: 0x0000_0000,
            active_border: 0x0000_0000,
        },
        ButtonType::CaptionClose => BaseColors {
            bg: tokens::with_alpha(CAPTION_CLOSE_HOVER, 0),
            hover_bg: CAPTION_CLOSE_HOVER,
            active_bg: CAPTION_CLOSE_PRESSED,
            border: 0x0000_0000,
            hover_border: 0x0000_0000,
            active_border: 0x0000_0000,
        },
        ButtonType::Link | ButtonType::Plain => BaseColors {
            bg: 0x0000_0000,
            hover_bg: 0x0000_0000,
            active_bg: 0x0000_0000,
            border: 0x0000_0000,
            hover_border: 0x0000_0000,
            active_border: 0x0000_0000,
        },
    }
}

pub fn paint(node: &mut RetainedNode, canvas: Canvas, theme: Theme) {
    if !node.visible {
        return;
    }
    let enabled = node.enabled;
    let bounds = node.bounds;
    let visual = node.visual;
    let data = match &mut node.data {
        NodeData::Button(data) => data,
        _ => return,
    };
    // an empty plain button (hit area, dismiss layer) paints only an
    // explicit background
    if data.button_type == ButtonType::Plain && data.icon == Icon::None && data.label.is_empty() {
        if let Some(bg) = visual.background
            && tokens::alpha(bg) > 0
        {
            canvas.fill_round_rect(bounds, visual.radius, bg);
        }
        return;
    }
    let colors = base_colors(data, enabled, theme);
    let hover_t = if enabled { data.hover_t } else { 0.0 };
    let press_t = if enabled { data.press_t } else { 0.0 };
    let animated_bg = tokens::lerp(
        tokens::lerp(colors.bg, colors.hover_bg, hover_t),
        colors.active_bg,
        press_t,
    );
    let animated_border = tokens::lerp(
        tokens::lerp(colors.border, colors.hover_border, hover_t),
        colors.active_border,
        press_t,
    );
    let bg = visual.background.unwrap_or(animated_bg);
    let border = visual.border_color.unwrap_or(animated_border);
    let rect = bounds;

    if tokens::alpha(bg) > 0 {
        canvas.fill_round_rect(rect, visual.radius, bg);
    }
    if data.button_type == ButtonType::Default && tokens::alpha(border) > 0 {
        canvas.stroke_round_rect(
            Rect::init(
                rect.x + 0.5,
                rect.y + 0.5,
                (rect.width - 1.0).max(0.0),
                (rect.height - 1.0).max(0.0),
            ),
            visual.radius,
            border,
            1.0,
        );
    }

    // disabled buttons always use the disabled text colour (antd)
    let mut color = if enabled {
        data.text_color
            .unwrap_or_else(|| text_color(data, true, theme))
    } else {
        text_color(data, false, theme)
    };
    // the close glyph turns white on its red fill
    if data.button_type == ButtonType::CaptionClose {
        color = tokens::lerp(color, 0xffff_ffff, hover_t.max(press_t));
    }
    if has_icon_and_label(data) {
        paint_icon_label(data, bounds, visual.radius, canvas, color);
    } else if data.icon != Icon::None {
        paint_icon(data, bounds, canvas, color);
    } else {
        let padding = label_padding(data);
        let align = data.text_align;
        if let Some(layout) = data.ensure_layout(color) {
            let x = match align {
                text::Align::Left => rect.x + padding,
                text::Align::Right => rect.right() - layout.metrics().longest_line - padding,
                text::Align::Center => rect.x + (rect.width - layout.metrics().longest_line) * 0.5,
            };
            let y = rect.y + (rect.height - layout.metrics().height) * 0.5;
            layout.paint(canvas, x, y);
        }
    }

    if data.focus_visible && enabled {
        let ring = Rect::init(
            rect.x - 2.0,
            rect.y - 2.0,
            rect.width + 4.0,
            rect.height + 4.0,
        );
        canvas.stroke_round_rect(
            ring,
            visual.radius + 2.0,
            tokens::with_alpha(theme.color_primary, 0x66),
            2.0,
        );
    }
    if data.wave_t < 1.0 {
        paint_wave(data, bounds, visual.radius, canvas, theme);
    }
}

/// antd click wave: a primary-coloured ring spreading 6px and fading out.
fn paint_wave(data: &ButtonData, bounds: Rect, radius: f32, canvas: Canvas, theme: Theme) {
    let ms = data.wave_t * 600.0;
    let spread = 6.0 * easing::apply(easing::Easing::EaseOutCirc, (ms / 400.0).min(1.0));
    let fade = 0.2 * (1.0 - easing::apply(easing::Easing::EaseOutCirc, ms / 600.0));
    if spread < 0.1 || fade < 0.004 {
        return;
    }
    let r = bounds;
    let half = spread * 0.5;
    let color = tokens::with_alpha(theme.color_primary, (255.0 * fade) as u8);
    canvas.stroke_round_rect(
        Rect::init(r.x - half, r.y - half, r.width + spread, r.height + spread),
        radius + half,
        color,
        spread,
    );
}

/// Advances hover / press / wave transitions (antd: hover 200ms, active
/// 100ms, ease-in-out). Returns true while another frame is needed.
pub fn animate(data: &mut ButtonData, now: i64, theme: Theme) -> bool {
    let mid = motion::Transition {
        duration_ms: theme.motion_duration_mid as u16,
        easing: easing::Easing::EaseInOut,
        delay_ms: 0,
    };
    let fast = motion::Transition {
        duration_ms: theme.motion_duration_fast as u16,
        easing: easing::Easing::EaseInOut,
        delay_ms: 0,
    };
    data.hover
        .retarget(if data.hovered { 1.0 } else { 0.0 }, now, mid);
    data.press
        .retarget(if data.pressed { 1.0 } else { 0.0 }, now, fast);
    data.hover_t = data.hover.value(now);
    data.press_t = data.press.value(now);
    data.wave_t = data.wave.value(now);
    !(data.hover.done(now) && data.press.done(now) && data.wave.done(now))
}

/// Starts the click wave (primary / default buttons only).
pub fn start_wave(data: &mut ButtonData, now: i64) {
    if data.button_type != ButtonType::Primary && data.button_type != ButtonType::Default {
        return;
    }
    data.wave = motion::Animated::start(
        0.0,
        1.0,
        now,
        motion::Transition {
            duration_ms: 600,
            easing: easing::Easing::Linear,
            delay_ms: 0,
        },
    );
}

pub fn set_enabled(node: &mut RetainedNode, enabled: bool, theme: Theme) {
    if node.enabled == enabled {
        return;
    }
    node.enabled = enabled;
    let data = match &mut node.data {
        NodeData::Button(data) => data,
        _ => return,
    };
    if !enabled {
        data.hovered = false;
        data.pressed = false;
        data.focused = false;
        data.hover = motion::Animated::at(0.0);
        data.press = motion::Animated::at(0.0);
        data.wave = motion::Animated::at(1.0);
        data.hover_t = 0.0;
        data.press_t = 0.0;
        data.wave_t = 1.0;
    }
    let color = text_color(data, enabled, theme);
    let _ = data.ensure_layout(color);
}

fn label_padding(data: &ButtonData) -> f32 {
    if data.label_padding > 0.0 {
        data.label_padding
    } else {
        horizontal_padding(data.size, data.button_type)
    }
}

fn has_icon_and_label(data: &ButtonData) -> bool {
    data.icon != Icon::None && !data.label.is_empty() && !is_caption(data.button_type)
}

/// Next to a label the icon matches the text (antd: the button font size).
fn label_icon_size(data: &ButtonData) -> f32 {
    data.icon_size.min(data.font_size + 2.0)
}

/// Icon and label as one group, aligned like a plain label.
fn paint_icon_label(data: &mut ButtonData, rect: Rect, radius: f32, canvas: Canvas, color: u32) {
    let size = label_icon_size(data);
    let padding = label_padding(data);
    let align = data.text_align;
    let icon = data.icon;
    let Some(layout) = data.ensure_layout(color) else {
        return;
    };
    let width = size + ICON_GAP + layout.metrics().longest_line;
    let x = match align {
        text::Align::Left => rect.x + padding,
        text::Align::Right => rect.right() - width - padding,
        text::Align::Center => rect.x + (rect.width - width) * 0.5,
    };
    icons::paint(
        canvas,
        icon,
        x + size * 0.5,
        rect.y + rect.height * 0.5,
        size,
        color,
        None,
    );
    layout.paint(
        canvas,
        x + size + ICON_GAP,
        rect.y + (rect.height - layout.metrics().height) * 0.5,
    );
    let _ = radius;
}

fn paint_icon(data: &ButtonData, rect: Rect, canvas: Canvas, color: u32) {
    let size = data.icon_size.min(rect.width.min(rect.height));
    // caption glyphs: 1px hairlines on the pixel grid, like Segoe Fluent Icons
    let stroke = if is_caption(data.button_type) {
        Some(1.0)
    } else {
        None
    };
    let cx = rect.x + rect.width * 0.5;
    let cy = rect.y + rect.height * 0.5;
    if stroke.is_some() {
        icons::paint(
            canvas,
            data.icon,
            cx.floor() + 0.5,
            cy.floor() + 0.5,
            size,
            color,
            stroke,
        );
    } else {
        icons::paint(canvas, data.icon, cx, cy, size, color, None);
    }
}
