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
use crate::layout::flex;
use crate::platform::platform::Cursor;
use crate::ui::actions::{Action, IndexAction};
use crate::ui::builders::{clickable, column};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, ClickableOptions};
use crate::ui::styled::{Style, styled};

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
        children: options.children.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::PointerButton;
    use crate::core::node::NodeKind;
    use crate::ui::actions::{ActionCx, PointerInfo, PointerPhase, action, index_action};
    use crate::ui::builders::text;
    use crate::ui::options::TextOptions;

    fn full_options() -> ViewOptions {
        ViewOptions {
            key: Some("box".into()),
            direction: flex::Direction::Column,
            flex_direction: Some(flex::Direction::Column),
            gap: 1.0,
            row_gap: Some(2.0),
            column_gap: Some(3.0),
            wrap: flex::Wrap::Wrap,
            align_content: flex::AlignContent::Center,
            align_self: flex::AlignSelf::End,
            order: 5,
            padding: Edges {
                left: 4.0,
                right: 5.0,
                top: 6.0,
                bottom: 7.0,
            },
            margin: Edges::all(8.0),
            justify: flex::Justify::Center,
            justify_content: Some(flex::Justify::End),
            align_items: flex::Align::Center,
            width: Some(100.0),
            height: Some(120.0),
            width_percent: Some(50.0),
            height_percent: Some(25.0),
            absolute: true,
            top: Some(9.0),
            right: Some(10.0),
            bottom: Some(11.0),
            left: Some(12.0),
            min_width: Some(13.0),
            min_height: Some(14.0),
            max_width: Some(15.0),
            max_height: Some(16.0),
            grow: 2.0,
            shrink: 3.0,
            basis: Some(17.0),
            background: Some(0xff11_1111),
            border_color: Some(0xff22_2222),
            border_width: 4.0,
            radius: 5.0,
            clip: true,
            scroll: true,
            block_pointer: true,
            selectable: true,
            unselectable: true,
            hover_background: Some(0xff33_3333),
            hover_border_color: Some(0xff44_4444),
            focus_scope: true,
            show_on_hover: true,
            on_resize: Some(action(|s: &mut u32| *s += 1)),
            shadow_color: Some(0xff55_5555),
            shadow_blur: 18.0,
            shadow_offset_y: 19.0,
            on_click: Some(action(|s: &mut u32| *s += 1)),
            index_action: Some(index_action(|s: &mut u32, i| *s = i as u32)),
            index: 6,
            cursor: Cursor::Arrow,
            style: Some(Style::default()),
            children: vec![text("child", TextOptions::default())],
        }
    }

    #[test]
    fn view_defaults() {
        let defaults = ViewOptions::default();
        assert!(defaults.key.is_none());
        assert_eq!(defaults.direction, flex::Direction::Column);
        assert!(defaults.flex_direction.is_none());
        assert_eq!(defaults.align_items, flex::Align::Stretch);
        assert_eq!(defaults.shrink, 0.0);
        assert_eq!(defaults.grow, 0.0);
        assert_eq!(defaults.cursor, Cursor::PointingHand);
        assert!(defaults.style.is_none());
        assert!(defaults.on_click.is_none());
        assert!(defaults.index_action.is_none());
        assert!(defaults.shadow_color.is_none());
        assert!(defaults.children.is_empty());
    }

    #[test]
    fn box_options_copies_every_field_of_box_options() {
        let options = full_options();
        let box_options = box_options(&options);
        assert_eq!(box_options.key.as_deref(), Some("box"));
        assert_eq!(box_options.gap, 1.0);
        assert_eq!(box_options.row_gap, Some(2.0));
        assert_eq!(box_options.column_gap, Some(3.0));
        assert_eq!(box_options.wrap, flex::Wrap::Wrap);
        assert_eq!(box_options.align_content, flex::AlignContent::Center);
        assert_eq!(box_options.align_self, flex::AlignSelf::End);
        assert_eq!(box_options.order, 5);
        assert_eq!(box_options.padding, options.padding);
        assert_eq!(box_options.margin, Edges::all(8.0));
        assert_eq!(box_options.justify, flex::Justify::Center);
        assert_eq!(box_options.justify_content, Some(flex::Justify::End));
        assert_eq!(box_options.align_items, flex::Align::Center);
        assert_eq!(box_options.flex_direction, Some(flex::Direction::Column));
        assert_eq!(box_options.width, Some(100.0));
        assert_eq!(box_options.height, Some(120.0));
        assert_eq!(box_options.width_percent, Some(50.0));
        assert_eq!(box_options.height_percent, Some(25.0));
        assert!(box_options.absolute);
        assert_eq!(box_options.top, Some(9.0));
        assert_eq!(box_options.right, Some(10.0));
        assert_eq!(box_options.bottom, Some(11.0));
        assert_eq!(box_options.left, Some(12.0));
        assert_eq!(box_options.min_width, Some(13.0));
        assert_eq!(box_options.min_height, Some(14.0));
        assert_eq!(box_options.max_width, Some(15.0));
        assert_eq!(box_options.max_height, Some(16.0));
        assert_eq!(box_options.grow, 2.0);
        assert_eq!(box_options.shrink, 3.0);
        assert_eq!(box_options.basis, Some(17.0));
        assert_eq!(box_options.background, Some(0xff11_1111));
        assert_eq!(box_options.border_color, Some(0xff22_2222));
        assert_eq!(box_options.border_width, 4.0);
        assert_eq!(box_options.radius, 5.0);
        assert!(box_options.clip);
        assert!(box_options.scroll);
        assert!(box_options.block_pointer);
        assert!(box_options.selectable);
        assert!(box_options.unselectable);
        assert_eq!(box_options.hover_background, Some(0xff33_3333));
        assert_eq!(box_options.hover_border_color, Some(0xff44_4444));
        assert!(box_options.focus_scope);
        assert!(box_options.show_on_hover);
        assert!(box_options.on_resize.is_some());
        assert_eq!(box_options.children.len(), 1);
    }

    #[test]
    fn flex_direction_alias_takes_precedence_over_direction() {
        let options = ViewOptions {
            direction: flex::Direction::Column,
            flex_direction: Some(flex::Direction::Row),
            ..Default::default()
        };
        assert_eq!(
            box_options(&options).flex_direction,
            Some(flex::Direction::Row)
        );
        let plain = ViewOptions {
            direction: flex::Direction::Row,
            ..Default::default()
        };
        assert_eq!(
            box_options(&plain).flex_direction,
            Some(flex::Direction::Row)
        );
    }

    #[test]
    fn view_builds_a_column_or_a_clickable_area() {
        let plain = view(ViewOptions {
            background: Some(0xff11_2222),
            children: vec![text("a", TextOptions::default())],
            ..Default::default()
        });
        let element = plain.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.align_items, flex::Align::Stretch);
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.visual.background, Some(0xff11_2222));
        assert!(element.pointer.is_none(), "no click handler: plain column");
        assert_eq!(element.children.len(), 1);

        let clickable = view(ViewOptions {
            on_click: Some(action(|s: &mut u32| *s += 1)),
            ..Default::default()
        });
        let element = clickable.element();
        assert!(
            element.pointer.is_some(),
            "views with clicks use pointerArea"
        );
        assert_eq!(element.cursor, Some(Cursor::PointingHand));
    }

    #[test]
    fn view_click_fires_through_the_clickable_pointer_area() {
        let mut state = 0u32;
        let node = view(ViewOptions {
            on_click: Some(action(|s: &mut u32| *s += 1)),
            index_action: Some(index_action(|s: &mut u32, i| *s = i as u32)),
            index: 5,
            width: Some(80.0),
            height: Some(40.0),
            ..Default::default()
        });
        let element = node.element();
        let pointer = element.pointer.clone().expect("pointer area");
        let info = PointerInfo {
            phase: PointerPhase::Up,
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 40.0,
            button: PointerButton::Left,
            ..Default::default()
        };
        let mut cx = ActionCx::new(&mut state);
        assert!(pointer.action.call(&mut cx, info));
        assert_eq!(state, 5, "index_action wins over on_click");
    }

    #[test]
    fn view_applies_the_shadow_only_when_it_or_the_style_needs_one() {
        let plain = view(ViewOptions {
            children: vec![text("a", TextOptions::default())],
            ..Default::default()
        });
        assert!(plain.element().visual.shadow_color.is_none());

        let shadowed = view(ViewOptions {
            shadow_color: Some(0x6612_3456),
            shadow_blur: 12.0,
            shadow_offset_y: 3.0,
            ..Default::default()
        });
        let element = shadowed.element();
        assert_eq!(element.visual.shadow_color, Some(0x6612_3456));
        assert_eq!(element.visual.shadow_blur, 12.0);
        assert_eq!(element.visual.shadow_offset_y, 3.0);

        // the inline style wins field by field; only the unset shadow fields
        // are filled from the options
        let styled = view(ViewOptions {
            shadow_color: Some(0xff00_0000),
            shadow_blur: 12.0,
            shadow_offset_y: 3.0,
            style: Some(Style {
                shadow_color: Some(0x1100_0000),
                ..Default::default()
            }),
            ..Default::default()
        });
        let element = styled.element();
        assert_eq!(element.visual.shadow_color, Some(0x1100_0000));
        assert_eq!(element.visual.shadow_blur, 12.0);
        assert_eq!(element.visual.shadow_offset_y, 3.0);

        // a style alone is applied last
        let inline = view(ViewOptions {
            background: Some(0xff11_1111),
            style: Some(Style {
                background: Some(0xff22_2222),
                radius: Some(9.0),
                ..Default::default()
            }),
            ..Default::default()
        });
        let element = inline.element();
        assert_eq!(element.visual.background, Some(0xff22_2222));
        assert_eq!(element.visual.radius, 9.0);
    }
}
