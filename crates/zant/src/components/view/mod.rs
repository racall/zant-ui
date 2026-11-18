//! The general-purpose container.
//!
//! It takes every `ui::BoxOptions` field (same names) with RN / block `div`
//! defaults — a flex column whose children stretch across it and that is not
//! squeezed by its parent — plus a drop shadow, an optional click handler
//! (the whole view becomes clickable, controls inside keep their own clicks)
//! and an inline `style` override applied last.
//!
//! The `BoxOptions` fields are assigned explicitly, so a field added to one
//! struct and not the other is a compile error rather than a silent omission.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::flex;
use crate::motion::motion;
use crate::platform::platform::Cursor;
use crate::ui::actions::{Action, IndexAction};
use crate::ui::builders::{clickable, column};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, ClickableOptions};
use crate::ui::styled::{Style, styled};

#[cfg(test)]
mod tests;

/// Options of [`view`] (`ViewOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct ViewOptions {
    pub key: Option<Str>,
    /// Main axis (RN `flexDirection`, default `.column`); the CSS-style
    /// alias `flex_direction` takes precedence when set.
    pub direction: flex::Direction,
    pub flex_direction: Option<flex::Direction>,
    pub gap: f32,
    pub row_gap: Option<f32>,
    pub column_gap: Option<f32>,
    pub wrap: flex::Wrap,
    pub align_content: flex::AlignContent,
    pub align_self: flex::AlignSelf,
    pub order: i32,
    pub padding: Edges,
    pub margin: Edges,
    pub justify: flex::Justify,
    pub justify_content: Option<flex::Justify>,
    /// Children fill the cross axis, like RN `View` and block `div`.
    pub align_items: flex::Align,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub width_percent: Option<f32>,
    pub height_percent: Option<f32>,
    pub absolute: bool,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub grow: f32,
    /// RN default: a view keeps its content size instead of being squeezed.
    pub shrink: f32,
    pub basis: Option<f32>,
    pub background: Option<u32>,
    pub border_color: Option<u32>,
    pub border_width: f32,
    pub radius: f32,
    pub clip: bool,
    /// Vertical scroll container (see `ui::BoxOptions::scroll`).
    pub scroll: bool,
    pub block_pointer: bool,
    pub selectable: bool,
    pub unselectable: bool,
    /// Background / border while hovered, painted without rebuilding.
    pub hover_background: Option<u32>,
    pub hover_border_color: Option<u32>,
    pub focus_scope: bool,
    pub show_on_hover: bool,
    pub on_resize: Option<Action>,
    /// Enter / exit transition, origin and layout transition (see
    /// `ui::BoxOptions::motion`).
    pub motion: motion::Presence,
    pub motion_origin: motion::Origin,
    pub layout_transition: bool,
    /// Drop shadow; use it sparingly (cards that float, popovers).
    pub shadow_color: Option<u32>,
    pub shadow_blur: f32,
    pub shadow_offset_y: f32,
    /// Makes the whole view clickable (`ui::clickable`).
    pub on_click: Option<Action>,
    /// Called with `index` instead of `on_click` (rows of a list).
    pub index_action: Option<IndexAction>,
    pub index: usize,
    /// Pointer cursor while hovering a clickable view.
    pub cursor: Cursor,
    /// Applied last, like an inline `style` (see `ui::styled`).
    pub style: Option<Style>,
    pub children: Vec<Node>,
}

impl Default for ViewOptions {
    fn default() -> Self {
        ViewOptions {
            key: None,
            direction: flex::Direction::Column,
            flex_direction: None,
            gap: 0.0,
            row_gap: None,
            column_gap: None,
            wrap: flex::Wrap::Nowrap,
            align_content: flex::AlignContent::Start,
            align_self: flex::AlignSelf::Auto,
            order: 0,
            padding: Edges::default(),
            margin: Edges::default(),
            justify: flex::Justify::Start,
            justify_content: None,
            // children stretch across the view (block `div` default)
            align_items: flex::Align::Stretch,
            width: None,
            height: None,
            width_percent: None,
            height_percent: None,
            absolute: false,
            top: None,
            right: None,
            bottom: None,
            left: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            grow: 0.0,
            // RN default
            shrink: 0.0,
            basis: None,
            background: None,
            border_color: None,
            border_width: 0.0,
            radius: 0.0,
            clip: false,
            scroll: false,
            block_pointer: false,
            selectable: false,
            unselectable: false,
            hover_background: None,
            hover_border_color: None,
            focus_scope: false,
            show_on_hover: false,
            on_resize: None,
            motion: motion::Presence::None,
            motion_origin: motion::Origin::Center,
            layout_transition: false,
            shadow_color: None,
            shadow_blur: 0.0,
            shadow_offset_y: 0.0,
            on_click: None,
            index_action: None,
            index: 0,
            cursor: Cursor::PointingHand,
            style: None,
            children: Vec::new(),
        }
    }
}

/// A `column` / `clickable` plus the shadow / style overrides.
pub fn view(options: ViewOptions) -> Node {
    let node = if options.on_click.is_some() || options.index_action.is_some() {
        clickable(ClickableOptions {
            r#box: box_options(&options),
            on_click: options.on_click.clone(),
            index_action: options.index_action.clone(),
            index: options.index,
            cursor: options.cursor,
        })
    } else {
        column(box_options(&options))
    };

    if options.style.is_none() && options.shadow_color.is_none() {
        return node;
    }
    let mut style = options.style.clone().unwrap_or_default();
    if let Some(color) = options.shadow_color {
        if style.shadow_color.is_none() {
            style.shadow_color = Some(color);
        }
        if style.shadow_blur.is_none() {
            style.shadow_blur = Some(options.shadow_blur);
        }
        if style.shadow_offset_y.is_none() {
            style.shadow_offset_y = Some(options.shadow_offset_y);
        }
    }
    styled(node, style)
}

/// The view's container options. Every `BoxOptions` field exists in
/// `ViewOptions` under the same name, so the two never drift apart; the
/// assignment is written out field by field.
pub fn box_options(options: &ViewOptions) -> BoxOptions {
    BoxOptions {
        key: options.key.clone(),
        gap: options.gap,
        row_gap: options.row_gap,
        column_gap: options.column_gap,
        wrap: options.wrap,
        align_content: options.align_content,
        align_self: options.align_self,
        order: options.order,
        padding: options.padding,
        margin: options.margin,
        justify: options.justify,
        justify_content: options.justify_content,
        align_items: options.align_items,
        // `direction` is the fallback when `flex_direction` is unset.
        flex_direction: Some(options.flex_direction.unwrap_or(options.direction)),
        width: options.width,
        height: options.height,
        width_percent: options.width_percent,
        height_percent: options.height_percent,
        absolute: options.absolute,
        top: options.top,
        right: options.right,
        bottom: options.bottom,
        left: options.left,
        min_width: options.min_width,
        min_height: options.min_height,
        max_width: options.max_width,
        max_height: options.max_height,
        grow: options.grow,
        shrink: options.shrink,
        basis: options.basis,
        background: options.background,
        border_color: options.border_color,
        border_width: options.border_width,
        radius: options.radius,
        clip: options.clip,
        scroll: options.scroll,
        block_pointer: options.block_pointer,
        selectable: options.selectable,
        unselectable: options.unselectable,
        hover_background: options.hover_background,
        hover_border_color: options.hover_border_color,
        focus_scope: options.focus_scope,
        show_on_hover: options.show_on_hover,
        on_resize: options.on_resize.clone(),
        motion: options.motion,
        motion_origin: options.motion_origin,
        layout_transition: options.layout_transition,
        children: options.children.clone(),
    }
}
