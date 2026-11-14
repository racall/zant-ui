//! Status / display builders of `src/ui.zig`: `alert`, `avatar`,
//! `progress` (WP-2.3).
//!
//! Antd status surfaces: a translucent tint + border for alerts, a round label
//! chip for avatars, and a track + filled bar for progress.
#![forbid(unsafe_code)]

use crate::core::node::{FlexStyle, NodeKind, Visual};
use crate::layout::flex;
use crate::renderer::text as text_mod;
use crate::theme::tokens;

use super::super::context::current_theme;
use super::super::element::{Element, Node};
use super::super::options::{AlertOptions, AlertType, AvatarOptions, ProgressOptions, TextOptions};
use super::super::styled::inset_xy;
use super::text;

/// `ui::alert`: an antd inline alert with an optional description.
pub fn alert(options: AlertOptions) -> Node {
    let theme = current_theme();
    let color: u32 = match options.r#type {
        AlertType::Info => theme.color_primary,
        AlertType::Success => theme.color_success,
        AlertType::Warning => theme.color_warning,
        AlertType::ErrorState => theme.color_error,
    };
    let mut children = vec![text(
        options.title,
        TextOptions {
            size: 14.0,
            weight: text_mod::Weight::Medium,
            color: Some(theme.color_text),
            wrap: true,
            ..Default::default()
        },
    )];
    if !options.description.is_empty() {
        children.push(text(
            options.description,
            TextOptions {
                size: 13.0,
                color: Some(theme.color_text_secondary),
                wrap: true,
                ..Default::default()
            },
        ));
    }
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: flex::Direction::Column,
            gap: 4.0,
            padding: inset_xy(14.0, 10.0),
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(tokens::with_alpha(color, 0x0f)),
            border_color: Some(tokens::with_alpha(color, 0x40)),
            border_width: 1.0,
            radius: 8.0,
            ..Visual::default()
        },
        children,
        ..Element::default()
    })
}

/// `ui::avatar`: a round label chip (antd avatar).
pub fn avatar(options: AvatarOptions) -> Node {
    let theme = current_theme();
    // light primary tint by default; on a custom background white text
    // (antd), so the label never takes the background's colour
    let color = options.color.unwrap_or(if options.background.is_some() {
        0xffff_ffff
    } else {
        theme.color_primary
    });
    let children = vec![text(
        options.text,
        TextOptions {
            size: (options.size * 0.4).max(12.0),
            weight: text_mod::Weight::Medium,
            color: Some(color),
            text_align: text_mod::Align::Center,
            wrap: false,
            ..Default::default()
        },
    )];
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: flex::Direction::Row,
            align_items: flex::Align::Center,
            justify: flex::Justify::Center,
            width: Some(options.size),
            height: Some(options.size),
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(options.background.unwrap_or(theme.color_primary_bg)),
            radius: options.size * 0.5,
            clip: true,
            ..Visual::default()
        },
        children,
        ..Element::default()
    })
}

/// `ui::progress`: an antd progress bar (track + filled bar).
pub fn progress(options: ProgressOptions) -> Node {
    let theme = current_theme();
    let percent = options.percent.clamp(0.0, 100.0);
    let children = vec![Node::new(Element {
        kind: NodeKind::Container,
        layout: FlexStyle {
            width: Some((options.width * percent / 100.0).max(0.0)),
            height: Some(options.height),
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(options.color.unwrap_or(theme.color_primary)),
            radius: options.height * 0.5,
            ..Visual::default()
        },
        ..Element::default()
    })];
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.key,
        layout: FlexStyle {
            direction: flex::Direction::Row,
            align_items: flex::Align::Center,
            width: Some(options.width),
            height: Some(options.height),
            ..FlexStyle::default()
        },
        visual: Visual {
            background: Some(theme.color_fill_secondary),
            radius: options.height * 0.5,
            clip: true,
            ..Visual::default()
        },
        children,
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> tokens::Theme {
        current_theme()
    }

    #[test]
    fn alert_uses_the_status_colour_with_alpha() {
        let cases = [
            (AlertType::Info, theme().color_primary),
            (AlertType::Success, theme().color_success),
            (AlertType::Warning, theme().color_warning),
            (AlertType::ErrorState, theme().color_error),
        ];
        for (kind, color) in cases {
            let node = alert(AlertOptions {
                r#type: kind,
                title: "Heads up".into(),
                ..Default::default()
            });
            let element = node.element();
            assert_eq!(element.kind, NodeKind::Container);
            assert_eq!(element.layout.direction, flex::Direction::Column);
            assert_eq!(element.layout.gap, 4.0);
            assert_eq!(element.layout.padding, inset_xy(14.0, 10.0));
            assert_eq!(
                element.visual.background,
                Some(tokens::with_alpha(color, 0x0f))
            );
            assert_eq!(
                element.visual.border_color,
                Some(tokens::with_alpha(color, 0x40))
            );
            assert_eq!(element.visual.border_width, 1.0);
            assert_eq!(element.visual.radius, 8.0);
            assert_eq!(element.children.len(), 1, "no description");
            let title = element.children[0].element();
            assert_eq!(title.content, "Heads up");
            assert_eq!(title.text_style.font_size, 14.0);
            assert_eq!(title.text_style.weight, text_mod::Weight::Medium);
            assert!(title.text_style.wrap);
        }

        let with_description = alert(AlertOptions {
            title: "Title".into(),
            description: "Details".into(),
            ..Default::default()
        });
        let element = with_description.element();
        assert_eq!(element.children.len(), 2);
        let description = element.children[1].element();
        assert_eq!(description.content, "Details");
        assert_eq!(description.text_style.font_size, 13.0);
        assert_eq!(description.text_style.color, theme().color_text_secondary);
        assert!(description.text_style.wrap);
    }

    #[test]
    fn avatar_is_round_and_uses_the_primary_tint() {
        let node = avatar(AvatarOptions::default());
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.layout.width, Some(32.0));
        assert_eq!(element.layout.height, Some(32.0));
        assert_eq!(element.layout.justify, flex::Justify::Center);
        assert_eq!(element.layout.align_items, flex::Align::Center);
        assert_eq!(element.visual.background, Some(theme().color_primary_bg));
        assert_eq!(element.visual.radius, 16.0);
        assert!(element.visual.clip);
        let label = element.children[0].element();
        assert_eq!(label.content, "?", "the Zig default label");
        assert!(
            (label.text_style.font_size - 12.8).abs() < 0.001,
            "40% of the size, min 12"
        );
        assert_eq!(label.text_style.color, theme().color_primary);
        assert_eq!(label.text_style.text_align, text_mod::Align::Center);

        let small = avatar(AvatarOptions {
            text: "Z".into(),
            size: 20.0,
            ..Default::default()
        });
        assert_eq!(
            small.element().children[0].element().text_style.font_size,
            12.0
        );

        let custom = avatar(AvatarOptions {
            text: "A".into(),
            background: Some(0xff11_2233),
            ..Default::default()
        });
        let element = custom.element();
        assert_eq!(element.visual.background, Some(0xff11_2233));
        assert_eq!(
            element.children[0].element().text_style.color,
            0xffff_ffff,
            "white on a custom background"
        );
    }

    #[test]
    fn progress_clamps_the_percent_and_sizes_the_bar() {
        let node = progress(ProgressOptions {
            key: Some("p".into()),
            percent: 50.0,
            width: 200.0,
            height: 8.0,
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("p"));
        assert_eq!(element.layout.width, Some(200.0));
        assert_eq!(element.layout.height, Some(8.0));
        assert_eq!(
            element.visual.background,
            Some(theme().color_fill_secondary)
        );
        assert_eq!(element.visual.radius, 4.0);
        assert!(element.visual.clip);
        let bar = element.children[0].element();
        assert_eq!(bar.layout.width, Some(100.0));
        assert_eq!(bar.layout.height, Some(8.0));
        assert_eq!(bar.visual.background, Some(theme().color_primary));
        assert_eq!(bar.visual.radius, 4.0);

        let over = progress(ProgressOptions {
            percent: 150.0,
            ..Default::default()
        });
        assert_eq!(
            over.element().children[0].element().layout.width,
            Some(200.0)
        );
        let under = progress(ProgressOptions {
            percent: -10.0,
            color: Some(0xff00_ff00),
            ..Default::default()
        });
        let under = under.element();
        assert_eq!(under.children[0].element().layout.width, Some(0.0));
        assert_eq!(
            under.children[0].element().visual.background,
            Some(0xff00_ff00)
        );
    }
}
