//! The `ui::*` builders of `src/ui.zig`.
//!
//! WP-1.2 ported the contract and the `column` / `row` / `text` / `button`
//! builders; WP-2.3 added every remaining builder of `ui.zig` except `memo`
//! (WP-2.4) and the composite components of `components/` (WP-2.5). Every
//! builder creates a fresh `Element` and wraps it in an `Rc`: Zig's `make()`
//! deep-copied into the frame arena, which Rust does not need because nodes
//! own their data.
//!
//! Layout: this file keeps the Zig helpers (`make` → `Node::new`,
//! `layoutFrom`, `visualFrom`, `hoverFrom`, `textStyle`) and the four WP-1.2
//! builders; the rest live in the submodules below, grouped like the Zig
//! sections they come from (each file is kept under ~500 lines).
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::node::{FlexStyle, HoverVisual, NodeKind, Visual};
use crate::layout::flex;
use crate::renderer::text;

use super::context::current_theme;
use super::element::{Element, Node};
use super::options::{BoxOptions, ButtonOptions, TextOptions};

pub mod choice;
pub mod containers;
pub mod feedback;
pub mod form;
pub mod interaction;
pub mod media;
pub mod pdf_preview;
pub mod typography;

pub use choice::{radio_group, select, tabs};
pub use containers::{card, divider, space, tag};
pub use feedback::{alert, avatar, progress};
pub use form::{checkbox, input, r#switch};
pub use interaction::{clickable, dismiss_layer, pointer_area, tooltip};
pub use media::{icon, icon_button, image, vector};
pub use pdf_preview::pdf_preview;
pub use typography::{heading, link, paragraph, rich_text, title};

/// Zig `layoutFrom`: the `BoxOptions` half of a container's `FlexStyle`.
fn layout_from(box_options: &BoxOptions, direction: flex::Direction) -> FlexStyle {
    FlexStyle {
        direction: box_options.flex_direction.unwrap_or(direction),
        justify: box_options.justify_content.unwrap_or(box_options.justify),
        align_items: box_options.align_items,
        gap: box_options.gap,
        row_gap: box_options.row_gap,
        column_gap: box_options.column_gap,
        wrap: box_options.wrap,
        align_content: box_options.align_content,
        align_self: box_options.align_self,
        order: box_options.order,
        padding: box_options.padding,
        margin: box_options.margin,
        width: box_options.width,
        height: box_options.height,
        width_percent: box_options.width_percent,
        height_percent: box_options.height_percent,
        absolute: box_options.absolute,
        top: box_options.top,
        right: box_options.right,
        bottom: box_options.bottom,
        left: box_options.left,
        min_width: box_options.min_width,
        min_height: box_options.min_height,
        max_width: box_options.max_width,
        max_height: box_options.max_height,
        grow: box_options.grow,
        shrink: box_options.shrink,
        basis: box_options.basis,
        scroll: box_options.scroll,
        // `anchor` is not part of `BoxOptions` (Zig keeps the struct default).
        anchor: None,
    }
}

/// Zig `visualFrom`: the painted half of a container's look.
fn visual_from(box_options: &BoxOptions) -> Visual {
    Visual {
        background: box_options.background,
        border_color: box_options.border_color,
        border_width: box_options.border_width,
        radius: box_options.radius,
        // a scroll container clips its content
        clip: box_options.clip || box_options.scroll,
        block_pointer: box_options.block_pointer,
        selectable: box_options.selectable,
        unselectable: box_options.unselectable,
        focus_scope: box_options.focus_scope,
        show_on_hover: box_options.show_on_hover,
        ..Visual::default()
    }
}

/// Zig `hoverFrom`: a hover look only when one of the colours is set.
fn hover_from(box_options: &BoxOptions) -> Option<HoverVisual> {
    if box_options.hover_background.is_none() && box_options.hover_border_color.is_none() {
        return None;
    }
    Some(HoverVisual {
        background: box_options.hover_background,
        border_color: box_options.hover_border_color,
    })
}

/// Zig `textStyle`.
fn text_style(options: &TextOptions, default_color: u32, wrap_default: bool) -> text::Style {
    text::Style {
        font_size: options.size,
        weight: options.weight,
        color: options.color.unwrap_or(default_color),
        text_align: options.text_align,
        // Zig: `if (options.wrap) true else wrap_default`
        wrap: options.wrap || wrap_default,
        line_height: options.line_height,
        max_lines: options.max_lines,
        family: options.family,
    }
}

/// `ui::column`: a vertical flex container.
pub fn column(options: BoxOptions) -> Node {
    let layout = layout_from(&options, flex::Direction::Column);
    let visual = visual_from(&options);
    let hover_visual = hover_from(&options);
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout,
        visual,
        children: options.children,
        on_resize: options.on_resize,
        hover_visual,
        ..Element::default()
    })
}

/// `ui::row`: a horizontal flex container.
pub fn row(options: BoxOptions) -> Node {
    let layout = layout_from(&options, flex::Direction::Row);
    let visual = visual_from(&options);
    let hover_visual = hover_from(&options);
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout,
        visual,
        children: options.children,
        on_resize: options.on_resize,
        hover_visual,
        ..Element::default()
    })
}

/// `ui::text`: a single-line text node (wrap it with `ui::styled` or use
/// `TextOptions::wrap` / `ui::paragraph` for wrapping text).
pub fn text(value: impl Into<Str>, options: TextOptions) -> Node {
    let theme = current_theme();
    let style = text_style(&options, theme.color_text, false);
    Node::new(Element {
        kind: NodeKind::Text,
        key: options.key,
        content: value.into(),
        layout: FlexStyle {
            shrink: 0.0,
            ..FlexStyle::default()
        },
        text_style: style,
        ..Element::default()
    })
}

/// `ui::button`: the antd button shell (the component paints it, WP-1.3).
pub fn button(options: ButtonOptions) -> Node {
    let height = match options.size {
        crate::core::node::ButtonSize::Small => 24.0,
        crate::core::node::ButtonSize::Middle => 32.0,
        crate::core::node::ButtonSize::Large => 40.0,
    };
    let font_size = options.font_size.unwrap_or(match options.size {
        crate::core::node::ButtonSize::Small => 12.0,
        crate::core::node::ButtonSize::Middle => 14.0,
        crate::core::node::ButtonSize::Large => 16.0,
    });
    let theme = current_theme();
    Node::new(Element {
        kind: NodeKind::Button,
        key: options.key,
        content: options.text,
        button_type: options.variant,
        button_size: options.size,
        enabled: !options.disabled,
        focusable: options.focusable,
        layout: FlexStyle {
            height: Some(height),
            width: options.width,
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            radius: theme.border_radius,
            ..Visual::default()
        },
        text_style: text::Style {
            font_size,
            text_align: options.text_align,
            ..text::Style::default()
        },
        text_color: options.text_color,
        label_padding: options.label_padding,
        icon: options.icon,
        icon_size: options.icon_size,
        on_click: options.on_click,
        index_action: options.index_action,
        index: options.index,
        ..Element::default()
    })
}

// ── option structs of the builders owned by WP-2.1 … WP-2.5 ─────────────────
// WP-1.2 declares their types only (see the module docs); the builders land in
// those work packages.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Edges;
    use crate::core::node::{ButtonSize, ButtonType, Icon};
    use crate::theme::tokens;
    use crate::ui::actions::{action, index_action};
    use crate::ui::context::set_build_theme;

    #[test]
    fn column_builds_a_container_with_zig_defaults() {
        let node = column(BoxOptions::default());
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.justify, flex::Justify::Start);
        assert_eq!(element.layout.align_items, flex::Align::Start);
        assert_eq!(element.layout.gap, 0.0);
        assert_eq!(element.layout.shrink, 0.0);
        assert!(element.children.is_empty());
        assert!(element.hover_visual.is_none());
        assert!(element.key.is_none());
    }

    #[test]
    fn column_and_row_layout_from_box_options() {
        let node = row(BoxOptions {
            key: Some("row".into()),
            gap: 4.0,
            justify: flex::Justify::Center,
            justify_content: Some(flex::Justify::End),
            align_items: flex::Align::Stretch,
            flex_direction: Some(flex::Direction::Column),
            padding: Edges::all(2.0),
            children: crate::nodes![Node::default()],
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.key.as_deref(), Some("row"));
        assert_eq!(
            element.layout.direction,
            flex::Direction::Column,
            "flex_direction wins"
        );
        assert_eq!(
            element.layout.justify,
            flex::Justify::End,
            "justify_content wins"
        );
        assert_eq!(element.layout.align_items, flex::Align::Stretch);
        assert_eq!(element.layout.gap, 4.0);
        assert_eq!(element.layout.padding, Edges::all(2.0));
        assert_eq!(element.children.len(), 1);

        let plain = row(BoxOptions::default());
        assert_eq!(plain.element().layout.direction, flex::Direction::Row);
    }

    #[test]
    fn containers_keep_look_clip_and_hover() {
        let node = column(BoxOptions {
            background: Some(0xff00_0000),
            border_color: Some(0xffff_ffff),
            border_width: 1.0,
            radius: 8.0,
            clip: false,
            scroll: true,
            block_pointer: true,
            selectable: true,
            unselectable: true,
            focus_scope: true,
            show_on_hover: true,
            hover_background: Some(0x1100_0000),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.visual.background, Some(0xff00_0000));
        assert_eq!(element.visual.border_color, Some(0xffff_ffff));
        assert_eq!(element.visual.border_width, 1.0);
        assert_eq!(element.visual.radius, 8.0);
        assert!(element.visual.clip, "a scroll container clips");
        assert!(element.visual.block_pointer);
        assert!(element.visual.selectable);
        assert!(element.visual.unselectable);
        assert!(element.visual.focus_scope);
        assert!(element.visual.show_on_hover);
        assert!(element.layout.scroll);
        let hover = element.hover_visual.expect("hover look");
        assert_eq!(hover.background, Some(0x1100_0000));
        assert_eq!(hover.border_color, None, "the other colour stays unset");
    }

    #[test]
    fn text_uses_the_build_theme_and_keeps_no_shrink() {
        let theme = current_theme();
        let node = text(
            "Hello",
            TextOptions {
                key: Some("greeting".into()),
                size: 18.0,
                weight: text::Weight::Bold,
                text_align: text::Align::Center,
                line_height: 1.2,
                max_lines: 1,
                family: text::Family::Serif,
                ..Default::default()
            },
        );
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Text);
        assert_eq!(element.key.as_deref(), Some("greeting"));
        assert_eq!(element.content, "Hello");
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.text_style.font_size, 18.0);
        assert_eq!(element.text_style.weight, text::Weight::Bold);
        assert_eq!(element.text_style.color, theme.color_text);
        assert_eq!(element.text_style.text_align, text::Align::Center);
        assert_eq!(element.text_style.line_height, 1.2);
        assert_eq!(element.text_style.max_lines, 1);
        assert_eq!(element.text_style.family, text::Family::Serif);
        assert!(
            !element.text_style.wrap,
            "ui::text does not wrap by default"
        );
    }

    #[test]
    fn text_colour_and_wrap_come_from_the_options() {
        set_build_theme(tokens::ant_dark());
        let dark = text("dark", TextOptions::default());
        assert_eq!(
            dark.element().text_style.color,
            tokens::ant_dark().color_text
        );
        set_build_theme(tokens::ant_light());

        let custom = text(
            "custom",
            TextOptions {
                color: Some(0xff12_3456),
                wrap: true,
                ..Default::default()
            },
        );
        assert_eq!(custom.element().text_style.color, 0xff12_3456);
        assert!(custom.element().text_style.wrap);
        // a non-static value is copied into the node
        let owned = String::from("runtime");
        let node = text(Str::copy(&owned), TextOptions::default());
        assert_eq!(node.element().content, "runtime");
    }

    #[test]
    fn button_sizes_map_to_the_antd_heights_and_fonts() {
        let expected = [
            (ButtonSize::Small, 24.0, 12.0),
            (ButtonSize::Middle, 32.0, 14.0),
            (ButtonSize::Large, 40.0, 16.0),
        ];
        for (size, height, font_size) in expected {
            let node = button(ButtonOptions {
                size,
                ..Default::default()
            });
            let element = node.element();
            assert_eq!(element.kind, NodeKind::Button);
            assert_eq!(element.button_size, size);
            assert_eq!(element.layout.height, Some(height));
            assert_eq!(element.layout.shrink, 0.0);
            assert_eq!(element.text_style.font_size, font_size);
            assert_eq!(element.text_style.text_align, text::Align::Center);
            assert_eq!(element.visual.radius, current_theme().border_radius);
            assert!(element.enabled);
            assert!(element.focusable);
        }
    }

    #[test]
    fn button_carries_label_state_and_callbacks() {
        let node = button(ButtonOptions {
            key: Some("save".into()),
            text: "Save".into(),
            variant: ButtonType::Primary,
            disabled: true,
            width: Some(120.0),
            font_size: Some(15.0),
            text_color: Some(0xffff_ffff),
            label_padding: 3.0,
            icon: Icon::Save,
            icon_size: 20.0,
            focusable: false,
            index_action: Some(index_action(|s: &mut u32, index| *s = index as u32)),
            index: 4,
            on_click: Some(action(|s: &mut u32| *s += 1)),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.key.as_deref(), Some("save"));
        assert_eq!(element.content, "Save");
        assert_eq!(element.button_type, ButtonType::Primary);
        assert!(!element.enabled);
        assert!(!element.focusable);
        assert_eq!(element.layout.width, Some(120.0));
        assert_eq!(element.text_style.font_size, 15.0);
        assert_eq!(element.text_color, Some(0xffff_ffff));
        assert_eq!(element.label_padding, 3.0);
        assert_eq!(element.icon, Icon::Save);
        assert_eq!(element.icon_size, 20.0);
        assert_eq!(element.index, 4);
        assert!(element.on_click.is_some());
        assert!(element.index_action.is_some());

        // the callbacks run against the root state
        let mut state = 0u32;
        {
            let mut cx = crate::ui::actions::ActionCx::new(&mut state);
            if let Some(callback) = element.on_click.clone() {
                callback.call(&mut cx);
            }
            if let Some(callback) = element.index_action.clone() {
                callback.call(&mut cx, 9);
            }
        }
        assert_eq!(state, 9);
    }

    #[test]
    fn builders_create_fresh_elements() {
        let a = column(BoxOptions::default());
        let b = column(BoxOptions::default());
        assert!(!a.ptr_eq(&b), "every builder returns its own element");
        let shared = a.clone();
        assert!(a.ptr_eq(&shared), "cloning a node shares the element");
    }
}
