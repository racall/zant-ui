//! The antd Dropdown menu panel (Dropdown + Menu tokens).
//!
//! 8 px radius, 4 px padding, 32 px item rows with 12 px horizontal padding
//! and a 4 px radius, a hover / press fill painted by the row's hit button,
//! icon + label + extra (shortcut) per item, dividers, group titles, disabled
//! and danger items.
//!
//! Position the panel with `ui::styled(.., Style { absolute: .. })` and put a
//! `ui::dismiss_layer` before it so outside clicks close it; the runtime owns
//! the open / close state and the `ui::key_bounds` positioning.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonType, Icon};
use crate::flex;
use crate::motion::motion;
use crate::theme::tokens;
use crate::ui::actions::{Action, IndexAction};
use crate::ui::builders::{button, column, divider, icon, row, space, text};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{
    BoxOptions, ButtonOptions, DividerOptions, IconOptions, SpaceOptions, TextOptions,
};
use crate::ui::styled::{Style, inset, styled};

#[cfg(test)]
mod tests;

/// What a menu row is (`MenuItemKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MenuItemKind {
    #[default]
    Item,
    Divider,
    Group,
}

/// One row of a dropdown menu (`MenuItem`).
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct MenuItem {
    pub kind: MenuItemKind,
    pub key: Option<Str>,
    pub label: Str,
    pub icon: Icon,
    /// Right-aligned secondary text (keyboard shortcut).
    pub extra: Str,
    pub disabled: bool,
    pub danger: bool,
    /// Highlighted (selected) item.
    pub selected: bool,
    /// Nesting level (indents the label by 12 px per level).
    pub indent: u8,
    pub on_click: Option<Action>,
    pub index_action: Option<IndexAction>,
    pub index: usize,
    /// Custom row content instead of icon / label / extra (still 32 px high).
    pub content: Option<Node>,
}

impl MenuItem {
    /// A 1 px separator row.
    pub const DIVIDER: MenuItem = MenuItem {
        kind: MenuItemKind::Divider,
        key: None,
        label: Str::Static(""),
        icon: Icon::None,
        extra: Str::Static(""),
        disabled: false,
        danger: false,
        selected: false,
        indent: 0,
        on_click: None,
        index_action: None,
        index: 0,
        content: None,
    };

    /// A small disabled-coloured section title.
    pub fn group(label: impl Into<Str>) -> MenuItem {
        MenuItem {
            kind: MenuItemKind::Group,
            label: label.into(),
            ..Default::default()
        }
    }
}

/// Options of [`dropdown_menu`] (`MenuOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct MenuOptions {
    pub key: Option<Str>,
    pub items: Vec<MenuItem>,
    /// Panel width (antd: content-sized with a 120 px minimum).
    pub width: f32,
    pub max_height: Option<f32>,
    pub font_size: f32,
    /// Colour overrides (default: active theme tokens).
    pub background: Option<u32>,
    pub text_color: Option<u32>,
    pub extra_color: Option<u32>,
    pub border_color: Option<u32>,
    /// Enter / exit transition (antd dropdown: slide-up from the top edge).
    pub motion: motion::Presence,
    /// Corner the panel grows from (the side its trigger is on).
    pub motion_origin: motion::Origin,
}

impl Default for MenuOptions {
    fn default() -> Self {
        MenuOptions {
            key: Some(Str::Static("dropdown-menu")),
            items: Vec::new(),
            width: 220.0,
            max_height: None,
            font_size: 14.0,
            background: None,
            text_color: None,
            extra_color: None,
            border_color: None,
            motion: motion::Presence::SlideUp,
            motion_origin: motion::Origin::TopRight,
        }
    }
}

/// The item's own key, else `"{base}-{index}"`.
fn item_key(base: &str, item: &MenuItem, index: usize) -> Str {
    match &item.key {
        Some(key) => key.clone(),
        None => Str::from(format!("{base}-{index}")),
    }
}

/// The antd dropdown panel.
pub fn dropdown_menu(o: MenuOptions) -> Node {
    let theme = current_theme();
    let fg = o.text_color.unwrap_or(theme.color_text);
    let extra_fg = o.extra_color.unwrap_or(theme.color_text_secondary);
    let inner_width = o.width - 8.0;
    let base_key = o.key.clone().unwrap_or(Str::Static("dropdown-menu"));
    let mut rows: Vec<Node> = Vec::with_capacity(o.items.len());
    for (i, item) in o.items.iter().enumerate() {
        let key = item_key(base_key.as_str(), item, i);
        match item.kind {
            MenuItemKind::Divider => rows.push(divider(DividerOptions {
                key: Some(key),
                color: Some(theme.color_split),
                margin: Edges {
                    top: 4.0,
                    bottom: 4.0,
                    ..Edges::default()
                },
            })),
            MenuItemKind::Group => rows.push(row(BoxOptions {
                key: Some(key),
                width: Some(inner_width),
                height: Some(28.0),
                padding: Edges {
                    left: 12.0,
                    right: 12.0,
                    ..Edges::default()
                },
                align_items: flex::Align::Center,
                children: vec![text(
                    item.label.clone(),
                    TextOptions {
                        size: o.font_size - 2.0,
                        color: Some(theme.color_text_disabled),
                        ..Default::default()
                    },
                )],
                ..Default::default()
            })),
            MenuItemKind::Item => {
                let label_color = if item.disabled {
                    theme.color_text_disabled
                } else if item.danger {
                    theme.color_error
                } else {
                    fg
                };
                let mut parts: Vec<Node> = Vec::new();
                // the hit target sits behind the content so hover / press
                // backgrounds are painted under the label
                parts.push(styled(
                    button(ButtonOptions {
                        key: Some(Str::from(format!("{}-hit", key.as_str()))),
                        variant: ButtonType::Text,
                        disabled: item.disabled,
                        focusable: false,
                        on_click: item.on_click.clone(),
                        index_action: item.index_action.clone(),
                        index: item.index,
                        ..Default::default()
                    }),
                    Style {
                        absolute: Some(true),
                        left: Some(0.0),
                        top: Some(0.0),
                        width_percent: Some(100.0),
                        height_percent: Some(100.0),
                        radius: Some(4.0),
                        background: if item.selected {
                            Some(theme.color_primary_bg)
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                ));
                if let Some(content) = &item.content {
                    parts.push(content.clone());
                } else {
                    if item.indent > 0 {
                        parts.push(space(SpaceOptions {
                            width: Some(f32::from(item.indent) * 12.0),
                            ..Default::default()
                        }));
                    }
                    if item.icon != Icon::None {
                        parts.push(icon(IconOptions {
                            name: item.icon,
                            size: o.font_size,
                            color: Some(if item.selected {
                                theme.color_primary
                            } else {
                                label_color
                            }),
                            ..Default::default()
                        }));
                    }
                    parts.push(styled(
                        text(
                            item.label.clone(),
                            TextOptions {
                                size: o.font_size,
                                color: Some(if item.selected {
                                    theme.color_primary
                                } else {
                                    label_color
                                }),
                                max_lines: 1,
                                ..Default::default()
                            },
                        ),
                        Style {
                            grow: Some(1.0),
                            shrink: Some(1.0),
                            ..Default::default()
                        },
                    ));
                    if !item.extra.is_empty() {
                        parts.push(text(
                            item.extra.clone(),
                            TextOptions {
                                size: o.font_size - 1.0,
                                color: Some(if item.disabled {
                                    theme.color_text_disabled
                                } else {
                                    extra_fg
                                }),
                                ..Default::default()
                            },
                        ));
                    }
                }
                rows.push(row(BoxOptions {
                    key: Some(key),
                    width: Some(inner_width),
                    height: Some(32.0),
                    padding: Edges {
                        left: 12.0,
                        right: 12.0,
                        ..Edges::default()
                    },
                    gap: 8.0,
                    align_items: flex::Align::Center,
                    children: parts,
                    ..Default::default()
                }));
            }
        }
    }
    styled(
        column(BoxOptions {
            key: Some(base_key),
            width: Some(o.width),
            padding: inset(4.0),
            children: rows,
            scroll: o.max_height.is_some(),
            height: o.max_height,
            ..Default::default()
        }),
        Style {
            background: Some(o.background.unwrap_or(theme.color_bg_elevated)),
            border_color: Some(
                o.border_color
                    .unwrap_or(tokens::with_alpha(theme.color_split, 0x10)),
            ),
            border_width: Some(1.0),
            radius: Some(8.0),
            shadow_color: Some(0x1f00_0000),
            shadow_blur: Some(16.0),
            shadow_offset_y: Some(6.0),
            clip: Some(true),
            block_pointer: Some(true),
            motion: Some(o.motion),
            motion_origin: Some(o.motion_origin),
            ..Default::default()
        },
    )
}
