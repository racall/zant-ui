//! The `danger`, `loading` and `block` button rows of the buttons section.
#![forbid(unsafe_code)]

use zant::core::node::{ButtonType, Icon};
use zant::ui::{self, ButtonOptions};

use super::ComponentsApp;

fn logged(message: &'static str) -> Option<ui::Action> {
    Some(ui::action(move |s: &mut ComponentsApp| {
        s.event(String::from(message))
    }))
}

/// antd `danger` in every variant, and a disabled one.
pub(super) fn danger_buttons() -> Vec<ui::Node> {
    let danger = |key: &'static str, text: &'static str, variant: ButtonType| {
        ui::button(ButtonOptions {
            key: Some(key.into()),
            text: text.into(),
            variant,
            danger: true,
            on_click: logged("click danger"),
            ..Default::default()
        })
    };
    vec![
        danger("btn-danger-primary", "Delete", ButtonType::Primary),
        danger("btn-danger-default", "Delete", ButtonType::Default),
        danger("btn-danger-text", "Text", ButtonType::Text),
        danger("btn-danger-link", "Link", ButtonType::Link),
        ui::button(ButtonOptions {
            key: Some("btn-danger-disabled".into()),
            text: "Disabled".into(),
            variant: ButtonType::Primary,
            danger: true,
            disabled: true,
            ..Default::default()
        }),
    ]
}

/// antd `loading`: the spinner replaces the icon and clicks are ignored.
pub(super) fn loading_buttons() -> Vec<ui::Node> {
    let loading = |key: &'static str, text: &'static str, variant: ButtonType, danger: bool| {
        ui::button(ButtonOptions {
            key: Some(key.into()),
            text: text.into(),
            variant,
            danger,
            loading: true,
            icon: Icon::Check,
            on_click: logged("click loading"),
            ..Default::default()
        })
    };
    vec![
        loading("btn-loading-primary", "Loading", ButtonType::Primary, false),
        loading("btn-loading-default", "Loading", ButtonType::Default, false),
        loading("btn-loading-danger", "Deleting", ButtonType::Primary, true),
        ui::button(ButtonOptions {
            key: Some("btn-loading-icon".into()),
            variant: ButtonType::Primary,
            loading: true,
            icon: Icon::Search,
            ..Default::default()
        }),
    ]
}

/// antd `block`: as wide as the card.
pub(super) fn block_button() -> ui::Node {
    ui::button(ButtonOptions {
        key: Some("btn-block".into()),
        text: "Block".into(),
        variant: ButtonType::Primary,
        block: true,
        on_click: logged("click block"),
        ..Default::default()
    })
}
