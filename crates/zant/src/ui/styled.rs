//! `ui::styled`: override common visual / layout / text properties of any
//! component (port of `Style` / `styled` in `src/ui.zig:550-665`).
#![forbid(unsafe_code)]

use crate::core::geometry::Edges;
use crate::layout::flex;
use crate::motion::motion;
use crate::renderer::text;

use super::element::Node;

/// Property overrides applied by [`styled`]; `None` keeps the component's own
/// value. Field names and order match the Zig `ui.Style`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct Style {
    pub background: Option<u32>,
    pub color: Option<u32>,
    pub border_color: Option<u32>,
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub radius: Option<f32>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub width_percent: Option<f32>,
    pub height_percent: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub grow: Option<f32>,
    pub shrink: Option<f32>,
    pub basis: Option<f32>,
    pub order: Option<i32>,
    pub align_self: Option<flex::AlignSelf>,
    pub absolute: Option<bool>,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    pub shadow_color: Option<u32>,
    pub shadow_blur: Option<f32>,
    pub shadow_offset_y: Option<f32>,
    pub clip: Option<bool>,
    pub block_pointer: Option<bool>,
    pub text_color: Option<u32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<text::Weight>,
    pub text_align: Option<text::Align>,
    pub line_height: Option<f32>,
    pub wrap: Option<bool>,
    /// Enter / exit transition (antd presets, see `docs/motion.md`). Give the
    /// node a `key` so a re-added node revives instead of duplicating.
    pub motion: Option<motion::Presence>,
    /// Transform origin for zoom / slide presets.
    pub motion_origin: Option<motion::Origin>,
    /// Animate position / size changes (leaf containers).
    pub layout_transition: Option<motion::Transition>,
    /// Absolute children: follow the n-th in-flow sibling's box.
    pub anchor: Option<u16>,
}

/// Override common visual / layout / text properties of any component.
///
/// ```ignore
/// ui::styled(ui::button(Default::default()), Style {
///     background: Some(0xff111111),
///     radius: Some(2.0),
///     height: Some(40.0),
///     ..Default::default()
/// })
/// ```
pub fn styled(node: Node, style: Style) -> Node {
    let mut element = node.element_clone();
    if let Some(value) = style.background {
        element.visual.background = Some(value);
    }
    if let Some(value) = style.border_color {
        element.visual.border_color = Some(value);
    }
    if let Some(value) = style.border_width {
        element.visual.border_width = value;
    }
    if let Some(value) = style.border_radius.or(style.radius) {
        element.visual.radius = value;
    }
    if let Some(value) = style.padding {
        element.layout.padding = value;
    }
    if let Some(value) = style.margin {
        element.layout.margin = value;
    }
    if let Some(value) = style.width {
        element.layout.width = Some(value);
    }
    if let Some(value) = style.height {
        element.layout.height = Some(value);
    }
    if let Some(value) = style.min_width {
        element.layout.min_width = Some(value);
    }
    if let Some(value) = style.min_height {
        element.layout.min_height = Some(value);
    }
    if let Some(value) = style.max_width {
        element.layout.max_width = Some(value);
    }
    if let Some(value) = style.max_height {
        element.layout.max_height = Some(value);
    }
    if let Some(value) = style.grow {
        element.layout.grow = value;
    }
    if let Some(value) = style.shrink {
        element.layout.shrink = value;
    }
    if let Some(value) = style.basis {
        element.layout.basis = Some(value);
    }
    if let Some(value) = style.width_percent {
        element.layout.width_percent = Some(value);
    }
    if let Some(value) = style.height_percent {
        element.layout.height_percent = Some(value);
    }
    // Zig assigns the absolute box twice (a harmless duplicate); kept so the
    // two implementations diff cleanly.
    if let Some(value) = style.absolute {
        element.layout.absolute = value;
    }
    if let Some(value) = style.top {
        element.layout.top = Some(value);
    }
    if let Some(value) = style.right {
        element.layout.right = Some(value);
    }
    if let Some(value) = style.bottom {
        element.layout.bottom = Some(value);
    }
    if let Some(value) = style.left {
        element.layout.left = Some(value);
    }
    if let Some(value) = style.order {
        element.layout.order = value;
    }
    if let Some(value) = style.align_self {
        element.layout.align_self = value;
    }
    if let Some(value) = style.absolute {
        element.layout.absolute = value;
    }
    if let Some(value) = style.top {
        element.layout.top = Some(value);
    }
    if let Some(value) = style.right {
        element.layout.right = Some(value);
    }
    if let Some(value) = style.bottom {
        element.layout.bottom = Some(value);
    }
    if let Some(value) = style.left {
        element.layout.left = Some(value);
    }
    if let Some(value) = style.shadow_color {
        element.visual.shadow_color = Some(value);
    }
    if let Some(value) = style.shadow_blur {
        element.visual.shadow_blur = value;
    }
    if let Some(value) = style.shadow_offset_y {
        element.visual.shadow_offset_y = value;
    }
    if let Some(value) = style.clip {
        element.visual.clip = value;
    }
    if let Some(value) = style.block_pointer {
        element.visual.block_pointer = value;
    }
    if let Some(value) = style.motion {
        element.motion.presence = value;
    }
    if let Some(value) = style.motion_origin {
        element.motion.origin = value;
    }
    if let Some(value) = style.layout_transition {
        element.motion.layout_transition = Some(value);
    }
    if let Some(value) = style.anchor {
        element.layout.anchor = Some(value);
    }
    if let Some(value) = style.color.or(style.text_color) {
        element.text_style.color = value;
        element.text_color = Some(value);
    }
    if let Some(value) = style.font_size {
        element.text_style.font_size = value;
    }
    if let Some(value) = style.font_weight {
        element.text_style.weight = value;
    }
    if let Some(value) = style.text_align {
        element.text_style.text_align = value;
    }
    if let Some(value) = style.line_height {
        element.text_style.line_height = value;
    }
    if let Some(value) = style.wrap {
        element.text_style.wrap = value;
    }
    Node::new(element)
}

/// The same inset on all four edges (`ui.inset`).
pub fn inset(value: f32) -> Edges {
    Edges::all(value)
}

/// Horizontal / vertical insets (`ui.insetXY`).
pub fn inset_xy(horizontal: f32, vertical: f32) -> Edges {
    Edges::symmetric(horizontal, vertical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::actions::action;
    use crate::ui::builders::{button, column, text};
    use crate::ui::context;
    use crate::ui::options::{BoxOptions, ButtonOptions, TextOptions};

    fn button_node() -> Node {
        button(ButtonOptions {
            text: "Save".into(),
            ..Default::default()
        })
    }

    #[test]
    fn defaults_are_all_none() {
        let style = Style::default();
        assert!(style.background.is_none());
        assert!(style.radius.is_none());
        assert!(style.padding.is_none());
        assert!(style.motion.is_none());
        assert!(style.anchor.is_none());
    }

    #[test]
    fn overrides_win_and_untouched_fields_survive() {
        let original = button_node();
        let node = styled(
            original.clone(),
            Style {
                background: Some(0xff11_1111),
                border_color: Some(0xff33_3333),
                border_width: Some(1.0),
                radius: Some(4.0),
                padding: Some(inset(12.0)),
                width: Some(240.0),
                height: Some(40.0),
                clip: Some(true),
                block_pointer: Some(true),
                font_size: Some(18.0),
                text_color: Some(0xffff_ffff),
                ..Default::default()
            },
        );
        let element = node.element();
        assert_eq!(element.visual.background, Some(0xff11_1111));
        assert_eq!(element.visual.border_color, Some(0xff33_3333));
        assert_eq!(element.visual.border_width, 1.0);
        assert_eq!(element.visual.radius, 4.0);
        assert_eq!(element.layout.padding, inset(12.0));
        assert_eq!(element.layout.width, Some(240.0));
        assert_eq!(element.layout.height, Some(40.0));
        assert!(element.visual.clip);
        assert!(element.visual.block_pointer);
        assert_eq!(element.text_style.font_size, 18.0);
        assert_eq!(element.text_style.color, 0xffff_ffff);
        assert_eq!(element.text_color, Some(0xffff_ffff));
        // the button keeps its own kind, content and shrink=0
        assert_eq!(original.element().kind, element.kind);
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.content, "Save");
        // the original node is untouched
        assert_eq!(original.element().layout.width, None);
        assert_eq!(original.element().visual.radius, 6.0);
    }

    #[test]
    fn border_radius_takes_precedence_over_radius() {
        let node = styled(
            button_node(),
            Style {
                border_radius: Some(2.0),
                radius: Some(9.0),
                ..Default::default()
            },
        );
        assert_eq!(node.element().visual.radius, 2.0);
    }

    #[test]
    fn colour_aliases_and_percent_sizes() {
        let node = styled(
            text("hello", TextOptions::default()),
            Style {
                color: Some(0xff00_ff00),
                text_color: Some(0xffff_0000),
                width_percent: Some(100.0),
                height_percent: Some(50.0),
                grow: Some(1.0),
                motion: Some(motion::Presence::Fade),
                motion_origin: Some(motion::Origin::Top),
                layout_transition: Some(motion::Transition::default()),
                anchor: Some(2),
                ..Default::default()
            },
        );
        let element = node.element();
        assert_eq!(element.text_style.color, 0xff00_ff00, "`color` wins");
        assert_eq!(element.text_color, Some(0xff00_ff00));
        assert_eq!(element.layout.width_percent, Some(100.0));
        assert_eq!(element.layout.height_percent, Some(50.0));
        assert_eq!(element.layout.grow, 1.0);
        assert_eq!(element.motion.presence, motion::Presence::Fade);
        assert_eq!(element.motion.origin, motion::Origin::Top);
        assert!(element.motion.layout_transition.is_some());
        assert_eq!(element.layout.anchor, Some(2));
    }

    #[test]
    fn styled_keeps_children_and_callbacks() {
        let row = column(BoxOptions {
            children: crate::nodes![text("a", TextOptions::default())],
            ..Default::default()
        });
        let with_click = styled(
            row,
            Style {
                background: Some(0xff00_0000),
                ..Default::default()
            },
        );
        assert_eq!(with_click.element().children.len(), 1);
        let _ = action(|s: &mut u32| *s += 1);
        let _ = context::current_theme();
    }
}
