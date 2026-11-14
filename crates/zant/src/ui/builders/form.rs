//! Form builders of `src/ui.zig`: `input`, `checkbox`, `switch` (WP-2.3).
//!
//! The builders only fill the declaration fields — editing, focus and the
//! toggle animation are runtime behaviour (WP-2.1 / WP-2.2). `radioGroup`,
//! `tabs` and `select` live in the sibling `choice` module.
#![forbid(unsafe_code)]

use crate::core::node::{FlexStyle, NodeKind, ToggleKind, Visual};
use crate::renderer::text as text_mod;

use super::super::context::current_theme;
use super::super::element::{Element, Node};
use super::super::options::{InputOptions, ToggleOptions};

/// `ui::input`: the antd input shell (single line or multiline).
pub fn input(options: InputOptions) -> Node {
    let theme = current_theme();
    let controlled = options.value.is_some();
    Node::new(Element {
        kind: NodeKind::Input,
        key: options.key,
        content: options.value.unwrap_or_default(),
        placeholder: options.placeholder,
        controlled,
        read_only: options.read_only,
        password: options.password,
        multiline: options.multiline,
        focus_ring: options.focus_ring,
        enabled: !options.disabled,
        focusable: true,
        layout: FlexStyle {
            height: Some(options.height),
            width: options.width,
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            radius: theme.border_radius,
            ..Visual::default()
        },
        text_style: text_mod::Style {
            font_size: options.font_size,
            line_height: options.line_height,
            ..text_mod::Style::default()
        },
        on_text: options.on_change,
        on_click: options.on_submit,
        on_undo: options.on_undo,
        on_redo: options.on_redo,
        selection: options.selection,
        on_selection: options.on_selection,
        on_key: options.on_key,
        content_max_width: options.content_max_width.unwrap_or(0.0),
        ..Element::default()
    })
}

/// `ui::checkbox`: a toggle with the checkbox look.
pub fn checkbox(options: ToggleOptions) -> Node {
    Node::new(Element {
        kind: NodeKind::Toggle,
        key: options.key,
        content: options.label,
        toggle_kind: ToggleKind::Checkbox,
        checked: options.checked,
        enabled: !options.disabled,
        focusable: true,
        layout: FlexStyle {
            height: Some(24.0),
            shrink: 0.0,
            ..FlexStyle::default()
        },
        text_style: text_mod::Style {
            font_size: options.font_size,
            ..text_mod::Style::default()
        },
        on_bool: options.on_change,
        ..Element::default()
    })
}

/// `ui::switch`: a toggle with the switch look.
pub fn r#switch(options: ToggleOptions) -> Node {
    Node::new(Element {
        kind: NodeKind::Toggle,
        key: options.key,
        content: options.label,
        toggle_kind: ToggleKind::SwitchControl,
        checked: options.checked,
        enabled: !options.disabled,
        focusable: true,
        layout: FlexStyle {
            height: Some(24.0),
            shrink: 0.0,
            ..FlexStyle::default()
        },
        text_style: text_mod::Style {
            font_size: options.font_size,
            ..text_mod::Style::default()
        },
        on_bool: options.on_change,
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens;
    use crate::ui::actions::{RangeAction, action, bool_action, text_action};

    fn theme() -> tokens::Theme {
        current_theme()
    }

    #[test]
    fn input_maps_the_value_and_editing_fields() {
        let node = input(InputOptions {
            key: Some("name".into()),
            value: Some("Ada".into()),
            placeholder: "Name".into(),
            height: 40.0,
            font_size: 15.0,
            line_height: 1.2,
            on_change: Some(text_action(|s: &mut u32, _| *s += 1)),
            on_submit: Some(action(|s: &mut u32| *s += 2)),
            on_undo: Some(action(|s: &mut u32| *s += 4)),
            on_redo: Some(action(|s: &mut u32| *s += 8)),
            selection: Some([0, 3]),
            on_selection: Some(RangeAction::new(|_, _, _| {})),
            on_key: Some(crate::ui::actions::InputKeyAction::new(|_, _, _, _, _| {
                false
            })),
            content_max_width: Some(600.0),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Input);
        assert_eq!(element.key.as_deref(), Some("name"));
        assert_eq!(element.content, "Ada");
        assert_eq!(element.placeholder, "Name");
        assert!(element.controlled, "a value makes the input controlled");
        assert!(element.enabled);
        assert!(element.focusable);
        assert!(element.focus_ring);
        assert!(!element.read_only);
        assert!(!element.password);
        assert!(!element.multiline);
        assert_eq!(element.layout.height, Some(40.0));
        assert_eq!(element.layout.width, None);
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.visual.radius, theme().border_radius);
        assert_eq!(element.text_style.font_size, 15.0);
        assert_eq!(element.text_style.line_height, 1.2);
        assert_eq!(element.selection, Some([0, 3]));
        assert_eq!(element.content_max_width, 600.0);
        assert!(element.on_text.is_some());
        assert!(element.on_click.is_some());
        assert!(element.on_undo.is_some());
        assert!(element.on_redo.is_some());
        assert!(element.on_selection.is_some());
        assert!(element.on_key.is_some());
    }

    #[test]
    fn uncontrolled_input_has_empty_content_and_defaults() {
        let node = input(InputOptions {
            disabled: true,
            width: Some(240.0),
            ..Default::default()
        });
        let element = node.element();
        assert!(!element.controlled);
        assert_eq!(element.content, "");
        assert!(!element.enabled);
        assert_eq!(element.layout.height, Some(32.0));
        assert_eq!(element.layout.width, Some(240.0));
        assert_eq!(element.text_style.font_size, 14.0);
        assert_eq!(element.content_max_width, 0.0);
        assert_eq!(element.text_style.line_height, 0.0);
        assert_eq!(element.visual.radius, theme().border_radius);
    }

    #[test]
    fn input_flags_are_carried_through() {
        let node = input(InputOptions {
            read_only: true,
            password: true,
            multiline: true,
            focus_ring: false,
            ..Default::default()
        });
        let element = node.element();
        assert!(element.read_only);
        assert!(element.password);
        assert!(element.multiline);
        assert!(!element.focus_ring);
    }

    #[test]
    fn checkbox_and_switch_differ_only_in_the_toggle_kind() {
        let checkbox_node = checkbox(ToggleOptions {
            key: Some("agree".into()),
            label: "Agree".into(),
            checked: true,
            font_size: 13.0,
            on_change: Some(bool_action(|s: &mut bool, checked| *s = checked)),
            ..Default::default()
        });
        let element = checkbox_node.element();
        assert_eq!(element.kind, NodeKind::Toggle);
        assert_eq!(element.key.as_deref(), Some("agree"));
        assert_eq!(element.content, "Agree");
        assert_eq!(element.toggle_kind, ToggleKind::Checkbox);
        assert!(element.checked);
        assert!(element.enabled);
        assert!(element.focusable);
        assert_eq!(element.layout.height, Some(24.0));
        assert_eq!(element.layout.shrink, 0.0);
        assert_eq!(element.text_style.font_size, 13.0);
        assert!(element.on_bool.is_some());

        let switch_node = r#switch(ToggleOptions {
            label: "On".into(),
            disabled: true,
            ..Default::default()
        });
        let element = switch_node.element();
        assert_eq!(element.toggle_kind, ToggleKind::SwitchControl);
        assert!(!element.enabled);
        assert_eq!(element.content, "On");
        assert_eq!(element.layout.height, Some(24.0));
    }

    #[test]
    fn form_builders_read_the_build_theme() {
        crate::ui::context::set_build_theme(tokens::ant_dark());
        let dark = input(InputOptions::default());
        assert_eq!(
            dark.element().visual.radius,
            tokens::ant_dark().border_radius
        );
        crate::ui::context::set_build_theme(tokens::ant_light());
    }
}
