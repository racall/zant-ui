//! Choice / navigation builders: `radio_group`, `tabs`, `select`.
//!
//! The builders declare the option list, the selection and the tab ink bar;
//! radio / select popover behaviour is runtime code.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonType, FlexStyle, NodeKind, ToggleKind, Visual};
use crate::motion::motion;

use super::super::context::current_theme;
use super::super::element::{Element, Node};
use super::super::options::{
    BoxOptions, ButtonOptions, DividerOptions, RadioGroupOptions, SelectOptions, TabsOptions,
};
use super::super::styled::{Style, styled};
use super::{button, column, divider, row};

/// `ui::radio_group`: one radio toggle per option in a column.
pub fn radio_group(options: RadioGroupOptions) -> Node {
    let base = options.key.as_deref().unwrap_or("radio-group").to_string();
    let mut children = Vec::with_capacity(options.options.len());
    for (i, option) in options.options.iter().enumerate() {
        children.push(Node::new(Element {
            kind: NodeKind::Toggle,
            key: Some(Str::copy(&format!("{base}-{i}"))),
            content: option.label.clone(),
            toggle_kind: ToggleKind::Radio,
            checked: i == options.selected,
            enabled: !option.disabled,
            focusable: true,
            layout: FlexStyle {
                height: Some(24.0),
                shrink: 0.0,
                ..FlexStyle::default()
            },
            index_action: options.on_change.clone(),
            index: i,
            ..Element::default()
        }));
    }
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: options.direction,
            gap: options.gap,
            ..FlexStyle::default()
        },
        children,
        ..Element::default()
    })
}

/// `ui::tabs`: an antd-style tab strip (tab buttons, ink bar, divider) above
/// the active item's content.
pub fn tabs(options: TabsOptions) -> Node {
    if options.items.is_empty() {
        return column(BoxOptions {
            key: options.key,
            ..Default::default()
        });
    }
    let active = options.active.min(options.items.len() - 1);
    let base = options.key.as_deref().unwrap_or("tab").to_string();
    let mut row_children = Vec::with_capacity(options.items.len() + 1);
    for (i, item) in options.items.iter().enumerate() {
        row_children.push(button(ButtonOptions {
            key: Some(Str::copy(&format!("{base}-{i}"))),
            text: item.label.clone(),
            variant: if i == active {
                ButtonType::Link
            } else {
                ButtonType::Text
            },
            index_action: options.on_change.clone(),
            index: i,
            ..Default::default()
        }));
    }
    // ink bar: follows the active tab (antd: 300ms ease)
    let theme = current_theme();
    row_children.push(styled(
        column(BoxOptions {
            key: Some(Str::copy(&format!("{base}-ink"))),
            children: Vec::new(),
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            anchor: Some(u16::try_from(active).expect("int cast")),
            bottom: Some(0.0),
            height: Some(2.0),
            background: Some(theme.color_primary),
            layout_transition: Some(motion::Transition {
                duration_ms: 300,
                easing: motion::Easing::Ease,
                delay_ms: 0,
            }),
            ..Default::default()
        },
    ));
    let content = options.items[active].content.clone();
    column(BoxOptions {
        key: options.key,
        gap: 0.0,
        children: vec![
            row(BoxOptions {
                gap: 4.0,
                children: row_children,
                ..Default::default()
            }),
            divider(DividerOptions {
                margin: Edges {
                    top: 4.0,
                    bottom: 4.0,
                    ..Edges::default()
                },
                ..Default::default()
            }),
            content,
        ],
        ..Default::default()
    })
}

/// `ui::select`: an antd select control (the popover is runtime behaviour).
pub fn select(options: SelectOptions) -> Node {
    let theme = current_theme();
    Node::new(Element {
        kind: NodeKind::Select,
        key: options.key,
        options: options.options,
        selected: options.selected,
        enabled: !options.disabled,
        focusable: true,
        layout: FlexStyle {
            width: options.width,
            height: Some(options.height),
            shrink: 0.0,
            ..FlexStyle::default()
        },
        visual: Visual {
            radius: theme.border_radius,
            ..Visual::default()
        },
        tooltip: options.tooltip,
        index_action: options.on_change,
        index: options.selected,
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::flex;
    use crate::theme::tokens;
    use crate::ui::options::{RadioOption, TabItem, TextOptions};

    fn theme() -> tokens::Theme {
        current_theme()
    }

    #[test]
    fn radio_group_builds_keyed_radio_toggles() {
        let node = radio_group(RadioGroupOptions {
            key: Some("size".into()),
            options: vec![
                RadioOption {
                    label: "S".into(),
                    ..Default::default()
                },
                RadioOption {
                    label: "M".into(),
                    disabled: true,
                },
                RadioOption {
                    label: "L".into(),
                    ..Default::default()
                },
            ],
            selected: 1,
            gap: 6.0,
            on_change: Some(crate::ui::actions::index_action(|s: &mut u32, index| {
                *s = index as u32
            })),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("size"));
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.gap, 6.0);
        assert_eq!(element.children.len(), 3);
        let keys: Vec<&str> = element
            .children
            .iter()
            .map(|child| child.element().key.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(keys, ["size-0", "size-1", "size-2"]);
        for (i, child) in element.children.iter().enumerate() {
            let radio = child.element();
            assert_eq!(radio.kind, NodeKind::Toggle);
            assert_eq!(radio.toggle_kind, ToggleKind::Radio);
            assert_eq!(radio.index, i);
            assert!(radio.index_action.is_some());
            assert!(radio.focusable);
            assert_eq!(radio.layout.height, Some(24.0));
            assert_eq!(radio.layout.shrink, 0.0);
            assert_eq!(radio.checked, i == 1);
            assert_eq!(radio.enabled, i != 1, "the disabled option is off");
            // `radio_group` does not set a text style (unlike checkbox),
            // so the `text.Style` default applies.
            assert_eq!(radio.text_style.font_size, 14.0);
        }

        // without a key the tabs / radios fall back to the fixed base name
        let unnamed = radio_group(RadioGroupOptions {
            options: vec![RadioOption {
                label: "only".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(
            unnamed.element().children[0].element().key.as_deref(),
            Some("radio-group-0")
        );
    }

    fn table() -> TabsOptions {
        TabsOptions {
            key: Some("tabs".into()),
            items: vec![
                TabItem {
                    label: "One".into(),
                    content: super::super::text("first", TextOptions::default()),
                },
                TabItem {
                    label: "Two".into(),
                    content: super::super::text("second", TextOptions::default()),
                },
            ],
            active: 0,
            on_change: Some(crate::ui::actions::index_action(|s: &mut u32, index| {
                *s = index as u32
            })),
        }
    }

    #[test]
    fn tabs_build_the_header_ink_bar_divider_and_content() {
        let node = tabs(table());
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("tabs"));
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.gap, 0.0);
        assert_eq!(element.children.len(), 3);

        let header = element.children[0].element();
        assert_eq!(header.layout.direction, flex::Direction::Row);
        assert_eq!(header.layout.gap, 4.0);
        assert_eq!(header.children.len(), 3, "two tabs + the ink bar");

        let first = header.children[0].element();
        assert_eq!(first.kind, NodeKind::Button);
        assert_eq!(first.key.as_deref(), Some("tabs-0"));
        assert_eq!(first.content, "One");
        assert_eq!(first.button_type, ButtonType::Link, "the active tab");
        assert_eq!(first.index, 0);
        assert!(first.index_action.is_some());
        let second = header.children[1].element();
        assert_eq!(second.button_type, ButtonType::Text);
        assert_eq!(second.key.as_deref(), Some("tabs-1"));
        assert_eq!(second.index, 1);

        let ink = header.children[2].element();
        assert_eq!(ink.key.as_deref(), Some("tabs-ink"));
        assert!(ink.layout.absolute);
        assert_eq!(ink.layout.anchor, Some(0));
        assert_eq!(ink.layout.bottom, Some(0.0));
        assert_eq!(ink.layout.height, Some(2.0));
        assert_eq!(ink.visual.background, Some(theme().color_primary));
        let transition = ink.motion.layout_transition.expect("ink bar transition");
        assert_eq!(transition.duration_ms, 300);
        assert_eq!(transition.easing, motion::Easing::Ease);

        let divider_element = element.children[1].element();
        assert_eq!(divider_element.layout.height, Some(1.0));
        assert_eq!(divider_element.layout.margin.top, 4.0);
        assert_eq!(divider_element.layout.margin.bottom, 4.0);

        let content = element.children[2].element();
        assert_eq!(content.content, "first");
    }

    #[test]
    fn tabs_clamp_the_active_index_and_keep_the_content_node() {
        let mut options = table();
        options.active = 99;
        let content = options.items[1].content.clone();
        let node = tabs(options);
        let element = node.element();
        let header = element.children[0].element();
        assert_eq!(
            header.children[0].element().button_type,
            ButtonType::Text,
            "out-of-range active falls back to the last tab"
        );
        assert_eq!(header.children[1].element().button_type, ButtonType::Link);
        assert_eq!(header.children[2].element().layout.anchor, Some(1));
        assert_eq!(element.children[2].element().content, "second");
        assert!(
            element.children[2].ptr_eq(&content),
            "the content node is reused, not rebuilt"
        );
    }

    #[test]
    fn tabs_without_items_is_an_empty_column() {
        let node = tabs(TabsOptions {
            key: Some("empty".into()),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("empty"));
        assert!(element.children.is_empty());
    }

    #[test]
    fn select_carries_options_selection_and_tooltip() {
        let node = select(SelectOptions {
            key: Some("pick".into()),
            options: vec!["a".into(), "b".into()],
            selected: 1,
            width: Some(120.0),
            height: 40.0,
            tooltip: Some("Pick one".into()),
            on_change: Some(crate::ui::actions::index_action(|s: &mut u32, index| {
                *s = index as u32
            })),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Select);
        assert_eq!(element.key.as_deref(), Some("pick"));
        assert_eq!(element.options.len(), 2);
        assert_eq!(element.options[1], "b");
        assert_eq!(element.selected, 1);
        assert_eq!(element.index, 1, "the index mirrors the selection");
        assert!(element.index_action.is_some());
        assert_eq!(element.tooltip.as_deref(), Some("Pick one"));
        assert_eq!(element.layout.width, Some(120.0));
        assert_eq!(element.layout.height, Some(40.0));
        assert_eq!(element.visual.radius, theme().border_radius);
        assert!(element.enabled);
        assert!(element.focusable);

        let disabled = select(SelectOptions {
            disabled: true,
            ..Default::default()
        });
        assert!(!disabled.element().enabled);
        assert_eq!(disabled.element().layout.height, Some(32.0));
        assert_eq!(disabled.element().selected, 0);
    }

    #[test]
    fn choice_builders_read_the_build_theme() {
        crate::ui::context::set_build_theme(tokens::ant_dark());
        let dark_select = select(SelectOptions::default());
        assert_eq!(
            dark_select.element().visual.radius,
            tokens::ant_dark().border_radius
        );
        crate::ui::context::set_build_theme(tokens::ant_light());
    }
}
