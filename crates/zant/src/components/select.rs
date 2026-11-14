//! Port of `src/components/select.zig` — WP-2.2: the closed antd select
//! control (field, selected label, chevron) plus the data helpers the runtime
//! needs for its overlay: the popup geometry / paint and the tooltip layer.
//!
//! The popup and tooltip *layers* are owned by the runtime (`ui_runtime.zig`:
//! `paintSelectPopup` / `showTooltip` / `paintTooltip`, WP-1.4/2.5). This
//! module only provides the component-side helpers (mirroring the Zig module /
//! runtime split 1:1): [`popup_rect`], [`option_at`], [`paint_popup`],
//! [`animate`] and the [`tooltip`] helpers. The runtime still has to own the
//! open/popup `NodeId`, the hovered option pointer handling, keyboard
//! navigation and the tooltip's delay / layout / transition state.
#![forbid(unsafe_code)]

use crate::core::geometry::{Rect, Size};
use crate::core::node::{FlexStyle, NodeData, RetainedNode, SelectData};
use crate::motion::motion;
use crate::renderer::canvas::Canvas;
use crate::renderer::text::{self, TextLayout};
use crate::theme::tokens;
use crate::theme::tokens::Theme;

/// Zig: `field_height` (the antd middle control height).
pub const FIELD_HEIGHT: f32 = 32.0;
/// Zig: `padding_x`.
pub const PADDING_X: f32 = 11.0;
/// Zig: `row_height`.
pub const ROW_HEIGHT: f32 = 30.0;
/// Zig: `popup_padding`.
pub const POPUP_PADDING: f32 = 4.0;

/// The antd control height of the closed field (`field_height`).
pub fn control_height() -> f32 {
    FIELD_HEIGHT
}

/// Horizontal padding between the field edge and the selected label.
pub fn horizontal_padding() -> f32 {
    PADDING_X
}

/// Label font size (`SelectData::font_size`).
pub fn font_size(data: &SelectData) -> f32 {
    data.font_size
}

/// Zig `measureWidth`.
pub fn measure_width(style: &FlexStyle) -> f32 {
    style.width.unwrap_or(220.0)
}

/// Zig `measureHeight`.
pub fn measure_height(style: &FlexStyle) -> f32 {
    style.height.unwrap_or(FIELD_HEIGHT)
}

/// Zig `selectedLabel`: the option at `selected`, or the "Select…" placeholder.
pub fn selected_label(data: &SelectData) -> &str {
    if data.selected < data.options.len() {
        data.options[data.selected].label.as_str()
    } else {
        "Select…"
    }
}

/// Zig `ensureLayout`: the selected label's layout and one layout per option
/// (the popup paints without re-shaping). Rebuilds per colour, like the Zig
/// cache; the label and the option set are keyed by colour / count.
pub fn ensure_layout(data: &mut SelectData, color: u32, option_color: u32) -> Option<&TextLayout> {
    let label_ok = !data.layout_dirty && data.layout.is_some() && data.layout_color == color;
    let options_ok =
        data.option_layouts.len() == data.options.len() && data.option_color == option_color;
    if label_ok && options_ok {
        return data.layout.as_ref();
    }
    if !label_ok {
        data.layout = None;
        let style = text::Style {
            font_size: data.font_size,
            color,
            wrap: false,
            ..Default::default()
        };
        match TextLayout::init(selected_label(data), style, 1.0e6) {
            Ok(layout) => data.layout = Some(layout),
            Err(_) => return None,
        }
        data.layout_color = color;
    }
    if !options_ok {
        data.option_color = option_color;
        data.option_layouts.clear();
        for index in 0..data.options.len() {
            let label = data.options[index].label.clone();
            let style = text::Style {
                font_size: data.font_size,
                color: option_color,
                wrap: false,
                ..Default::default()
            };
            match TextLayout::init(&label, style, 1.0e6) {
                Ok(layout) => data.option_layouts.push(layout),
                Err(_) => return None,
            }
        }
    }
    data.layout_dirty = false;
    data.layout.as_ref()
}

fn select_data(node: &RetainedNode) -> Option<&SelectData> {
    match &node.data {
        NodeData::Select(data) => Some(data),
        _ => None,
    }
}

/// Zig `popupRect`: dropdown size / position below the field, flipped above
/// near the window bottom, clamped to 8px margins.
pub fn popup_rect(node: &RetainedNode, viewport: Size) -> Rect {
    let Some(data) = select_data(node) else {
        return Rect::default();
    };
    let height = data.options.len() as f32 * ROW_HEIGHT + POPUP_PADDING * 2.0;
    let mut rect = Rect::init(
        node.bounds.x,
        node.bounds.bottom() + 4.0,
        node.bounds.width,
        height,
    );
    if rect.bottom() > viewport.height - 8.0 {
        rect.y = node.bounds.y - rect.height - 4.0;
    }
    if rect.y < 8.0 {
        rect.y = 8.0;
    }
    rect
}

/// Zig `optionAt`: the option index under a window-space point, if any.
pub fn option_at(node: &RetainedNode, x: f32, y: f32, viewport: Size) -> Option<usize> {
    let data = select_data(node)?;
    let rect = popup_rect(node, viewport);
    if !rect.contains(x, y) {
        return None;
    }
    let local = y - rect.y - POPUP_PADDING;
    if local < 0.0 {
        return None;
    }
    let index = (local / ROW_HEIGHT).floor() as usize;
    if index >= data.options.len() {
        return None;
    }
    Some(index)
}

pub fn paint(node: &mut RetainedNode, canvas: Canvas, theme: Theme) {
    let rect = node.bounds;
    let enabled = node.enabled;
    let visual = node.visual;
    let data = match &mut node.data {
        NodeData::Select(data) => data,
        _ => return,
    };
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
    let bg = visual.background.unwrap_or(if enabled {
        theme.color_bg_container
    } else {
        theme.color_fill_secondary
    });
    if focus_t > 0.01 {
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
    let color = if enabled {
        theme.color_text
    } else {
        theme.color_text_disabled
    };
    if let Some(layout) = ensure_layout(data, color, theme.color_text) {
        layout.paint(
            canvas,
            rect.x + PADDING_X,
            rect.y + (rect.height - layout.metrics().height) * 0.5,
        );
    }
    let cx = rect.right() - 14.0;
    let cy = rect.y + rect.height * 0.5;
    let dir: f32 = if data.open { -1.0 } else { 1.0 };
    canvas.draw_line(
        cx - 4.0,
        cy - 2.0 * dir,
        cx,
        cy + 2.0 * dir,
        theme.color_text_secondary,
        1.2,
    );
    canvas.draw_line(
        cx,
        cy + 2.0 * dir,
        cx + 4.0,
        cy - 2.0 * dir,
        theme.color_text_secondary,
        1.2,
    );
}

/// Zig `paintPopup`: the antd slide-up dropdown, painted by the runtime above
/// the tree (it is not a tree node). Kept here so the transition and the row
/// layout stay next to the closed control's paint.
pub fn paint_popup(node: &RetainedNode, canvas: Canvas, theme: Theme, viewport: Size) {
    let Some(data) = select_data(node) else {
        return;
    };
    if data.options.is_empty() {
        return;
    }
    // antd slide-up: grows from the edge next to the field while fading in
    let fx = motion::presence_fx(motion::Presence::SlideUp, data.open_t);
    if fx.opacity <= 0.003 {
        return;
    }
    let rect = popup_rect(node, viewport);
    let above = rect.y < node.bounds.y;
    let origin = motion::origin_point(
        if above {
            motion::Origin::Bottom
        } else {
            motion::Origin::Top
        },
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    );
    canvas.save();
    canvas.translate(origin[0], origin[1]);
    canvas.scale(fx.scale_x, fx.scale_y);
    canvas.translate(-origin[0], -origin[1]);
    let layer = fx.opacity < 0.997;
    if layer {
        canvas.save_layer_alpha(
            Rect::init(
                rect.x - 24.0,
                rect.y - 24.0,
                rect.width + 48.0,
                rect.height + 48.0,
            ),
            fx.opacity,
        );
    }
    let mut shadow = rect;
    shadow.y += 6.0;
    canvas.shadow_round_rect(shadow, 8.0, 0x1f00_0000, 16.0);
    canvas.fill_round_rect(rect, 8.0, theme.color_bg_elevated);
    canvas.stroke_round_rect(rect, 8.0, theme.color_border_secondary, 1.0);
    canvas.save();
    canvas.clip_rect(rect);
    for i in 0..data.options.len() {
        let row = Rect::init(
            rect.x + POPUP_PADDING,
            rect.y + POPUP_PADDING + i as f32 * ROW_HEIGHT,
            rect.width - POPUP_PADDING * 2.0,
            ROW_HEIGHT,
        );
        if data.hovered_option == Some(i) {
            canvas.fill_round_rect(row, 4.0, theme.color_fill_secondary);
        }
        if i == data.selected {
            canvas.fill_round_rect(
                Rect::init(row.x, row.y + 7.0, 2.0, row.height - 14.0),
                1.0,
                theme.color_primary,
            );
        }
        if i < data.option_layouts.len() {
            let layout = &data.option_layouts[i];
            layout.paint(
                canvas,
                row.x + 8.0,
                row.y + (row.height - layout.metrics().height) * 0.5,
            );
        }
    }
    canvas.restore();
    if layer {
        canvas.restore();
    }
    canvas.restore();
}

/// Border / ring / popup transitions (antd 200ms). Returns true while
/// another frame is needed.
pub fn animate(data: &mut SelectData, now: i64, theme: Theme) -> bool {
    let mid = motion::Transition {
        duration_ms: theme.motion_duration_mid as u16,
        easing: crate::motion::easing::Easing::EaseInOut,
        delay_ms: 0,
    };
    data.hover
        .retarget(if data.hovered { 1.0 } else { 0.0 }, now, mid);
    data.focus_anim
        .retarget(if data.focused || data.open { 1.0 } else { 0.0 }, now, mid);
    data.open_anim.retarget(
        if data.open { 1.0 } else { 0.0 },
        now,
        if data.open {
            motion::presence_enter(motion::Presence::SlideUp)
        } else {
            motion::presence_exit(motion::Presence::SlideUp)
        },
    );
    data.hover_t = data.hover.value(now);
    data.focus_t = data.focus_anim.value(now);
    data.open_t = data.open_anim.value(now);
    !(data.hover.done(now) && data.focus_anim.done(now) && data.open_anim.done(now))
}

/// Component-side helpers for the runtime's tooltip overlay.
///
/// Port of the tooltip half of `ui_runtime.zig` (`showTooltip` /
/// `paintTooltip` / `clearTooltip`): the layout style, the centred-below /
/// flipped-above rectangle, the enter / exit transitions and the paint. The
/// real layer — the tooltip target `NodeId`, the 450ms delay timer, keeping
/// the layout alive through the zoom-out, and calling this after the tree
/// paint — lives in the runtime (WP-1.4/2.5), not here.
pub mod tooltip {
    use super::{Canvas, Rect, Size, TextLayout, motion, text};

    /// Hover delay before the tooltip appears (`ui_runtime.zig:showTooltip`).
    pub const DELAY_MS: i64 = 450;
    pub const FONT_SIZE: f32 = 12.0;
    pub const PADDING_X: f32 = 8.0;
    pub const PADDING_Y: f32 = 4.0;
    /// Gap between the target and the tooltip box.
    pub const GAP: f32 = 6.0;
    /// Minimum distance to the window edges.
    pub const MARGIN: f32 = 8.0;
    pub const RADIUS: f32 = 6.0;
    /// antd `colorBgSpotlight`.
    pub const BACKGROUND: u32 = 0xd900_0000;
    pub const TEXT_COLOR: u32 = 0xffff_ffff;
    /// antd `zoom-big-fast`.
    pub const PRESENCE: motion::Presence = motion::Presence::ZoomBigFast;

    /// The text style `showTooltip` lays the content out with.
    pub fn text_style() -> text::Style {
        text::Style {
            font_size: FONT_SIZE,
            color: TEXT_COLOR,
            wrap: false,
            ..Default::default()
        }
    }

    /// The tooltip box size for a laid-out content (`longest_line` + padding).
    pub fn measure(layout: &TextLayout) -> (f32, f32) {
        (
            layout.metrics().longest_line + PADDING_X * 2.0,
            layout.metrics().height + PADDING_Y * 2.0,
        )
    }

    /// `showTooltip`'s geometry: centred below `bounds`, flipped above when it
    /// would overflow the window bottom, clamped 8px from the left / right.
    /// Returns `(rect, above)`, the runtime's `tooltip_rect` / `tooltip_above`.
    pub fn rect(bounds: Rect, viewport: Size, layout: &TextLayout) -> (Rect, bool) {
        let (width, height) = measure(layout);
        let mut x = bounds.x + (bounds.width - width) * 0.5;
        let mut y = bounds.bottom() + GAP;
        x = x.clamp(MARGIN, (viewport.width - width - MARGIN).max(MARGIN));
        let mut above = false;
        if y + height > viewport.height - MARGIN {
            y = (bounds.y - height - GAP).max(MARGIN);
            above = true;
        }
        (Rect::init(x, y, width, height), above)
    }

    /// The enter transition `showTooltip` starts (`Animated::start(0, 1, …)`).
    pub fn enter() -> motion::Transition {
        motion::presence_enter(PRESENCE)
    }

    /// The exit transition `clearTooltip` retargets to.
    pub fn exit() -> motion::Transition {
        motion::presence_exit(PRESENCE)
    }

    /// `paintTooltip`: zoom / fade around the edge next to the target, then a
    /// round spotlight box with the laid-out content.
    pub fn paint(canvas: Canvas, layout: &TextLayout, rect: Rect, above: bool, anim_value: f32) {
        let fx = motion::presence_fx(PRESENCE, anim_value);
        if fx.opacity <= 0.003 {
            return;
        }
        let origin = motion::origin_point(
            if above {
                motion::Origin::Bottom
            } else {
                motion::Origin::Top
            },
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        canvas.save();
        canvas.translate(origin[0], origin[1]);
        canvas.scale(fx.scale_x, fx.scale_y);
        canvas.translate(-origin[0], -origin[1]);
        let layer = fx.opacity < 0.997;
        if layer {
            canvas.save_layer_alpha(
                Rect::init(
                    rect.x - 4.0,
                    rect.y - 4.0,
                    rect.width + 8.0,
                    rect.height + 8.0,
                ),
                fx.opacity,
            );
        }
        canvas.fill_round_rect(rect, RADIUS, BACKGROUND);
        layout.paint(canvas, rect.x + PADDING_X, rect.y + PADDING_Y);
        if layer {
            canvas.restore();
        }
        canvas.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Str;
    use crate::core::node::{NodeData, RetainedNode, SelectData, SelectOption};
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;

    const CLEAR: u32 = 0xff00_ff00;
    const WIDTH: usize = 400;
    const HEIGHT: usize = 300;

    fn node(options: &[&str], selected: usize) -> RetainedNode {
        let data = SelectData {
            options: options
                .iter()
                .map(|label| SelectOption {
                    label: Str::copy(label),
                })
                .collect(),
            selected,
            ..Default::default()
        };
        let mut node = RetainedNode::new(0, NodeData::Select(data));
        node.bounds = Rect::init(8.0, 8.0, 220.0, FIELD_HEIGHT);
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
    fn selected_label_falls_back_to_the_placeholder() {
        let data = SelectData {
            options: vec![
                SelectOption {
                    label: Str::from("A"),
                },
                SelectOption {
                    label: Str::from("B"),
                },
            ],
            selected: 1,
            ..Default::default()
        };
        assert_eq!(selected_label(&data), "B");
        assert_eq!(
            selected_label(&SelectData {
                selected: 5,
                ..Default::default()
            }),
            "Select…"
        );
        assert_eq!(selected_label(&SelectData::default()), "Select…");
    }

    #[test]
    fn ensure_layout_caches_the_label_and_every_option() {
        let theme = ant_light();
        let mut data = SelectData {
            options: vec![
                SelectOption {
                    label: Str::from("Alpha"),
                },
                SelectOption {
                    label: Str::from("Beta"),
                },
            ],
            ..Default::default()
        };
        let layout =
            ensure_layout(&mut data, theme.color_text, theme.color_text).expect("label layout");
        assert!(layout.metrics().longest_line > 0.0);
        assert_eq!(data.option_layouts.len(), 2);
        assert_eq!(data.layout_color, theme.color_text);
        assert!(!data.layout_dirty);
        // a different colour re-shapes the label, an unchanged colour reuses it
        let first = data
            .layout
            .as_ref()
            .map(|layout| layout.metrics().longest_line);
        ensure_layout(&mut data, theme.color_text, theme.color_text).expect("cached layout");
        assert_eq!(
            data.layout
                .as_ref()
                .map(|layout| layout.metrics().longest_line),
            first
        );
        ensure_layout(&mut data, theme.color_text_disabled, theme.color_text)
            .expect("relaid-out label");
        assert_eq!(data.layout_color, theme.color_text_disabled);
    }

    #[test]
    fn closed_control_paints_its_fill_and_chevron() {
        let theme = ant_light();
        let mut closed = node(&["A", "B"], 0);
        let down = paint_into(&mut closed, WIDTH, HEIGHT, theme);
        // rounded field at (8, 8): (14, 24) is left padding, the container fill
        assert_eq!(down.pixel_at(14, 24), Some(theme.color_bg_container));

        let mut open = node(&["A", "B"], 0);
        open_select(&mut open, true);
        let up = paint_into(&mut open, WIDTH, HEIGHT, theme);
        assert!(
            changed_pixels(&down, &up, WIDTH, HEIGHT) > 0,
            "the chevron flips when the popup is open"
        );
    }

    fn open_select(node: &mut RetainedNode, open: bool) {
        if let NodeData::Select(data) = &mut node.data {
            data.open = open;
        }
    }

    #[test]
    fn hover_focus_and_disabled_change_the_field() {
        let theme = ant_light();
        let mut plain = node(&["A", "B"], 0);
        let base = paint_into(&mut plain, WIDTH, HEIGHT, theme);

        let mut hovered = node(&["A", "B"], 0);
        if let NodeData::Select(data) = &mut hovered.data {
            data.hover_t = 1.0;
        }
        let hover = paint_into(&mut hovered, WIDTH, HEIGHT, theme);
        assert!(changed_pixels(&base, &hover, WIDTH, HEIGHT) > 0);

        let mut focused = node(&["A", "B"], 0);
        if let NodeData::Select(data) = &mut focused.data {
            data.focus_t = 1.0;
        }
        let focus = paint_into(&mut focused, WIDTH, HEIGHT, theme);
        assert!(
            changed_pixels(&base, &focus, WIDTH, HEIGHT) > 0,
            "the focus ring and active border show"
        );

        let mut disabled = node(&["A", "B"], 0);
        disabled.enabled = false;
        let dimmed = paint_into(&mut disabled, WIDTH, HEIGHT, theme);
        assert_ne!(dimmed.pixel_at(14, 24), base.pixel_at(14, 24));
    }

    #[test]
    fn popup_rect_sits_below_flips_above_and_clamps() {
        let node = node(&["A", "B", "C"], 0);
        let rect = popup_rect(
            &node,
            Size {
                width: 400.0,
                height: 300.0,
            },
        );
        assert_eq!(
            rect,
            Rect::init(8.0, 44.0, 220.0, 3.0 * ROW_HEIGHT + POPUP_PADDING * 2.0)
        );

        // near the bottom edge the dropdown flips above and clamps to 8px
        let rect = popup_rect(
            &node,
            Size {
                width: 400.0,
                height: 100.0,
            },
        );
        assert_eq!(rect.y, 8.0);
    }

    #[test]
    fn option_at_hits_the_rows_only() {
        let node = node(&["A", "B", "C"], 0);
        let viewport = Size {
            width: 400.0,
            height: 300.0,
        };
        assert_eq!(
            option_at(
                &node,
                100.0,
                44.0 + POPUP_PADDING + ROW_HEIGHT * 2.5,
                viewport
            ),
            Some(2)
        );
        assert_eq!(option_at(&node, 100.0, 45.0, viewport), None, "padding");
        assert_eq!(option_at(&node, 300.0, 100.0, viewport), None, "sideways");
        assert_eq!(
            option_at(
                &node,
                100.0,
                44.0 + 2.0 * POPUP_PADDING + 3.0 * ROW_HEIGHT - 1.0,
                viewport
            ),
            None,
            "below the rows"
        );
    }

    #[test]
    fn popup_paints_rows_hover_and_selection() {
        let theme = ant_light();
        let mut node = node(&["A", "B", "C"], 1);
        if let NodeData::Select(data) = &mut node.data {
            data.open = true;
            data.open_t = 1.0;
            data.hovered_option = Some(2);
        }
        ensure_layout(
            match &mut node.data {
                NodeData::Select(data) => data,
                _ => unreachable!(),
            },
            theme.color_text,
            theme.color_text,
        )
        .expect("layouts");
        let viewport = Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        };
        let rect = popup_rect(&node, viewport);
        let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            paint_popup(&node, canvas, theme, viewport);
        }
        renderer.end();
        // rows are 30px tall from y = rect.y + 4, 4px inside the popup x
        let row = |index: usize| rect.y + POPUP_PADDING + index as f32 * ROW_HEIGHT;
        let plain_pixel = renderer.pixel_at(100, row(1) as usize + 15).expect("pixel");
        assert_eq!(plain_pixel, theme.color_bg_elevated);
        let hovered_pixel = renderer.pixel_at(100, row(2) as usize + 15).expect("pixel");
        assert_ne!(hovered_pixel, plain_pixel);
        // selected row 1 keeps the 2px primary marker at its left
        assert_eq!(
            renderer.pixel_at(
                rect.x as usize + POPUP_PADDING as usize,
                row(1) as usize + 15
            ),
            Some(theme.color_primary)
        );
    }

    #[test]
    fn popup_paints_nothing_until_it_opens() {
        let theme = ant_light();
        let mut node = node(&["A", "B"], 0);
        let viewport = Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        };
        let closed = {
            let mut renderer =
                Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
            {
                let canvas = renderer.begin(1.0).expect("frame canvas");
                canvas.clear(CLEAR);
                paint_popup(&node, canvas, theme, viewport);
            }
            renderer.end();
            renderer
        };
        let mut blank = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = blank.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
        }
        blank.end();
        assert_eq!(changed_pixels(&closed, &blank, WIDTH, HEIGHT), 0);

        if let NodeData::Select(data) = &mut node.data {
            data.open_t = 0.5;
        }
        let opening = {
            let mut renderer =
                Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
            {
                let canvas = renderer.begin(1.0).expect("frame canvas");
                canvas.clear(CLEAR);
                paint_popup(&node, canvas, theme, viewport);
            }
            renderer.end();
            renderer
        };
        assert!(changed_pixels(&opening, &blank, WIDTH, HEIGHT) > 0);
    }

    #[test]
    fn animate_tracks_hover_focus_and_open() {
        let theme = ant_light();
        let mut data = SelectData {
            open: true,
            hovered: true,
            ..Default::default()
        };
        assert!(animate(&mut data, 1_000, theme));
        assert_eq!(data.open_t, 0.0);
        animate(&mut data, 1_100, theme);
        assert!(data.hover_t > 0.0 && data.hover_t < 1.0);
        assert!(data.open_t > 0.0 && data.open_t < 1.0);
        assert!(!animate(&mut data, 10_000, theme));
        assert_eq!(data.hover_t, 1.0);
        assert_eq!(data.focus_t, 1.0, "open counts as focused");
        assert_eq!(data.open_t, 1.0);
    }

    #[test]
    fn tree_paint_dispatches_to_the_select_module() {
        use crate::core::tree::Tree;
        let theme = ant_light();
        let mut tree = Tree::init(theme);
        let root = tree.root;
        tree.add_select(Some(root), &["A", "B"], 1);
        tree.layout(Size {
            width: 400.0,
            height: 100.0,
        });
        let mut renderer = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            tree.paint(canvas);
        }
        renderer.end();
        // the field at (0, 0, 220, 32): (4, 16) is the container fill, left of
        // the label
        assert_eq!(renderer.pixel_at(4, 16), Some(theme.color_bg_container));
    }

    #[test]
    fn measure_and_padding_follow_the_style() {
        let style = FlexStyle::default();
        assert_eq!(measure_width(&style), 220.0);
        assert_eq!(measure_height(&style), FIELD_HEIGHT);
        let sized = FlexStyle {
            width: Some(160.0),
            height: Some(40.0),
            ..Default::default()
        };
        assert_eq!(measure_width(&sized), 160.0);
        assert_eq!(measure_height(&sized), 40.0);
        assert_eq!(control_height(), FIELD_HEIGHT);
        assert_eq!(horizontal_padding(), PADDING_X);
        assert_eq!(
            font_size(&SelectData {
                font_size: 16.0,
                ..Default::default()
            }),
            16.0
        );
    }

    #[test]
    fn tooltip_rect_centres_below_and_flips_above() {
        let layout = TextLayout::init("Tip", tooltip::text_style(), 1.0e6).expect("tooltip layout");
        let (width, height) = tooltip::measure(&layout);
        let bounds = Rect::init(100.0, 100.0, 80.0, 24.0);
        let (rect, above) = tooltip::rect(
            bounds,
            Size {
                width: 400.0,
                height: 300.0,
            },
            &layout,
        );
        assert!(!above);
        assert_eq!(rect.width, width);
        assert_eq!(rect.height, height);
        assert_eq!(rect.y, 124.0 + tooltip::GAP);
        assert_eq!(rect.x, 100.0 + (80.0 - width) * 0.5);

        // near the bottom it flips above the target, still 8px from the top
        let (rect, above) = tooltip::rect(
            Rect::init(100.0, 250.0, 80.0, 24.0),
            Size {
                width: 400.0,
                height: 300.0,
            },
            &layout,
        );
        assert!(above);
        assert_eq!(rect.y, 250.0 - height - tooltip::GAP);
        assert!(rect.y + rect.height <= 250.0);

        // clamped to the left margin when the target sticks out on the left
        let (rect, _) = tooltip::rect(
            Rect::init(-100.0, 100.0, 10.0, 24.0),
            Size {
                width: 400.0,
                height: 300.0,
            },
            &layout,
        );
        assert_eq!(rect.x, tooltip::MARGIN);
    }

    #[test]
    fn tooltip_paint_fades_in() {
        let layout = TextLayout::init("Tip", tooltip::text_style(), 1.0e6).expect("tooltip layout");
        let (rect, above) = tooltip::rect(
            Rect::init(100.0, 100.0, 80.0, 24.0),
            Size {
                width: WIDTH as f32,
                height: HEIGHT as f32,
            },
            &layout,
        );
        let render = |value: f32| {
            let mut renderer =
                Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
            {
                let canvas = renderer.begin(1.0).expect("frame canvas");
                canvas.clear(CLEAR);
                tooltip::paint(canvas, &layout, rect, above, value);
            }
            renderer.end();
            renderer
        };
        let hidden = render(0.0);
        let shown = render(1.0);
        assert!(changed_pixels(&hidden, &shown, WIDTH, HEIGHT) > 0);
        // the spotlight box covers the middle of the rect
        let pixel = shown
            .pixel_at(
                (rect.x + 20.0) as usize,
                (rect.y + rect.height - 3.0) as usize,
            )
            .expect("pixel");
        assert_ne!(pixel, CLEAR);
        let mut blank = Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("renderer");
        {
            let canvas = blank.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
        }
        blank.end();
        assert_eq!(changed_pixels(&hidden, &blank, WIDTH, HEIGHT), 0);
    }
}
