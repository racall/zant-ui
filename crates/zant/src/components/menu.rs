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
use crate::layout::flex;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeKind;
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;
    use crate::ui::actions::{ActionCx, action, index_action};
    use crate::ui::app::App;
    use crate::ui::builders::column;
    use crate::ui::context::set_build_theme;
    use crate::ui::{BoxOptions, RunOptions};

    const CLEAR: u32 = 0xff00_ff00;

    fn options() -> MenuOptions {
        MenuOptions {
            items: vec![
                MenuItem {
                    key: Some("open".into()),
                    label: "Open".into(),
                    icon: Icon::File,
                    extra: "⌘O".into(),
                    selected: true,
                    index: 3,
                    on_click: Some(action(|s: &mut u32| *s += 1)),
                    index_action: Some(index_action(|s: &mut u32, i| *s = i as u32)),
                    ..Default::default()
                },
                MenuItem::DIVIDER,
                MenuItem::group("Files"),
                MenuItem {
                    key: Some("delete".into()),
                    label: "Delete".into(),
                    danger: true,
                    disabled: true,
                    indent: 2,
                    ..Default::default()
                },
                MenuItem {
                    content: Some(text("custom", TextOptions::default())),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn menu_option_defaults() {
        let defaults = MenuOptions::default();
        assert_eq!(defaults.key.as_deref(), Some("dropdown-menu"));
        assert!(defaults.items.is_empty());
        assert_eq!(defaults.width, 220.0);
        assert!(defaults.max_height.is_none());
        assert_eq!(defaults.font_size, 14.0);
        assert!(defaults.background.is_none());
        assert!(defaults.text_color.is_none());
        assert!(defaults.extra_color.is_none());
        assert!(defaults.border_color.is_none());
        assert_eq!(defaults.motion, motion::Presence::SlideUp);
        assert_eq!(defaults.motion_origin, motion::Origin::TopRight);
        let item = MenuItem::default();
        assert_eq!(item.kind, MenuItemKind::Item);
        assert!(item.key.is_none());
        assert!(item.label.is_empty());
        assert_eq!(item.icon, Icon::None);
        assert!(!item.disabled && !item.danger && !item.selected);
        assert_eq!(item.indent, 0);
        assert_eq!(item.index, 0);
        assert_eq!(MenuItem::DIVIDER.kind, MenuItemKind::Divider);
        assert_eq!(MenuItem::group("x").kind, MenuItemKind::Group);
    }

    #[test]
    fn menu_builds_rows_dividers_groups_and_hit_targets() {
        set_build_theme(ant_light());
        let theme = ant_light();
        let node = dropdown_menu(options());
        let panel = node.element();
        assert_eq!(panel.kind, NodeKind::Container);
        assert_eq!(panel.key.as_deref(), Some("dropdown-menu"));
        assert_eq!(panel.layout.width, Some(220.0));
        assert_eq!(panel.layout.padding, inset(4.0));
        assert!(!panel.layout.scroll, "no max height: not scrollable");
        assert_eq!(panel.layout.height, None);
        assert_eq!(panel.visual.background, Some(theme.color_bg_elevated));
        assert_eq!(
            panel.visual.border_color,
            Some(tokens::with_alpha(theme.color_split, 0x10))
        );
        assert_eq!(panel.visual.border_width, 1.0);
        assert_eq!(panel.visual.radius, 8.0);
        assert_eq!(panel.visual.shadow_color, Some(0x1f00_0000));
        assert_eq!(panel.visual.shadow_blur, 16.0);
        assert_eq!(panel.visual.shadow_offset_y, 6.0);
        assert!(panel.visual.clip);
        assert!(panel.visual.block_pointer);
        assert_eq!(panel.motion.presence, motion::Presence::SlideUp);
        assert_eq!(panel.motion.origin, motion::Origin::TopRight);
        assert_eq!(panel.children.len(), 5);

        // 1: a normal item row (hit target + icon + label + extra)
        let open = panel.children[0].element();
        assert_eq!(open.key.as_deref(), Some("open"));
        assert_eq!(open.kind, NodeKind::Container);
        assert_eq!(open.layout.width, Some(212.0));
        assert_eq!(open.layout.height, Some(32.0));
        assert_eq!(open.layout.gap, 8.0);
        assert_eq!(open.layout.align_items, flex::Align::Center);
        assert_eq!(open.layout.padding.left, 12.0);
        assert_eq!(open.layout.padding.right, 12.0);
        assert_eq!(open.children.len(), 4);
        let hit = open.children[0].element();
        assert_eq!(hit.key.as_deref(), Some("open-hit"));
        assert_eq!(hit.kind, NodeKind::Button);
        assert_eq!(hit.button_type, ButtonType::Text);
        assert!(!hit.focusable);
        assert!(hit.layout.absolute);
        assert_eq!(hit.layout.left, Some(0.0));
        assert_eq!(hit.layout.top, Some(0.0));
        assert_eq!(hit.layout.width_percent, Some(100.0));
        assert_eq!(hit.layout.height_percent, Some(100.0));
        assert_eq!(hit.visual.radius, 4.0);
        assert_eq!(
            hit.visual.background,
            Some(theme.color_primary_bg),
            "the selected row is highlighted"
        );
        assert_eq!(hit.index, 3);
        assert!(hit.on_click.is_some());
        assert!(hit.index_action.is_some());
        let menu_icon = open.children[1].element();
        assert_eq!(menu_icon.kind, NodeKind::Vector);
        assert!(menu_icon.drawing.is_some());
        let label = open.children[2].element();
        assert_eq!(label.kind, NodeKind::Text);
        assert_eq!(label.content, "Open");
        assert_eq!(label.text_style.font_size, 14.0);
        assert_eq!(label.text_style.color, theme.color_primary);
        assert_eq!(label.text_style.max_lines, 1);
        assert_eq!(label.layout.grow, 1.0);
        assert_eq!(label.layout.shrink, 1.0);
        let extra = open.children[3].element();
        assert_eq!(extra.content, "⌘O");
        assert_eq!(extra.text_style.font_size, 13.0);
        assert_eq!(extra.text_style.color, theme.color_text_secondary);

        // 2: divider
        let line = panel.children[1].element();
        assert_eq!(line.key.as_deref(), Some("dropdown-menu-1"));
        assert_eq!(line.layout.height, Some(1.0));
        assert_eq!(line.layout.margin.top, 4.0);
        assert_eq!(line.layout.margin.bottom, 4.0);
        assert_eq!(line.visual.background, Some(theme.color_split));

        // 3: group title
        let group = panel.children[2].element();
        assert_eq!(group.key.as_deref(), Some("dropdown-menu-2"));
        assert_eq!(group.layout.width, Some(212.0));
        assert_eq!(group.layout.height, Some(28.0));
        assert_eq!(group.layout.padding.left, 12.0);
        let title = group.children[0].element();
        assert_eq!(title.content, "Files");
        assert_eq!(title.text_style.font_size, 12.0);
        assert_eq!(title.text_style.color, theme.color_text_disabled);

        // 4: danger + disabled + indent (disabled wins for the label colour)
        let delete = panel.children[3].element();
        assert_eq!(delete.key.as_deref(), Some("delete"));
        assert_eq!(delete.children.len(), 3, "hit, indent spacer, label");
        let spacer = delete.children[1].element();
        assert_eq!(spacer.layout.width, Some(24.0));
        let danger_label = delete.children[2].element();
        assert_eq!(danger_label.content, "Delete");
        assert_eq!(danger_label.text_style.color, theme.color_text_disabled);

        // 5: custom content keeps the 32 px row and the hit target
        let custom = panel.children[4].element();
        assert_eq!(custom.key.as_deref(), Some("dropdown-menu-4"));
        assert_eq!(custom.layout.height, Some(32.0));
        assert_eq!(custom.children.len(), 2);
        assert_eq!(custom.children[1].element().content, "custom");
    }

    #[test]
    fn menu_item_keys_fall_back_to_the_base_key_and_index() {
        let node = dropdown_menu(MenuOptions {
            key: Some("actions".into()),
            items: vec![MenuItem::DIVIDER, MenuItem::group("More")],
            ..Default::default()
        });
        let panel = node.element();
        assert_eq!(panel.key.as_deref(), Some("actions"));
        let keys: Vec<&str> = panel
            .children
            .iter()
            .map(|child| child.element().key.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(keys, ["actions-0", "actions-1"]);
        // an explicit item key wins over the fallback
        let keyed = dropdown_menu(MenuOptions {
            items: vec![MenuItem {
                key: Some("explicit".into()),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(
            keyed.element().children[0].element().key.as_deref(),
            Some("explicit")
        );
    }

    #[test]
    fn menu_colour_and_size_overrides_are_applied() {
        let node = dropdown_menu(MenuOptions {
            width: 300.0,
            max_height: Some(200.0),
            font_size: 16.0,
            background: Some(0xff11_1111),
            text_color: Some(0xff22_2222),
            extra_color: Some(0xff33_3333),
            border_color: Some(0xff44_4444),
            motion: motion::Presence::Fade,
            motion_origin: motion::Origin::BottomLeft,
            items: vec![
                MenuItem {
                    label: "Item".into(),
                    extra: "K".into(),
                    danger: true,
                    ..Default::default()
                },
                MenuItem {
                    label: "Plain".into(),
                    extra: "J".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        let panel = node.element();
        assert_eq!(panel.layout.height, Some(200.0));
        assert!(panel.layout.scroll, "a max height makes the panel scroll");
        assert_eq!(panel.visual.background, Some(0xff11_1111));
        assert_eq!(panel.visual.border_color, Some(0xff44_4444));
        assert_eq!(panel.motion.presence, motion::Presence::Fade);
        assert_eq!(panel.motion.origin, motion::Origin::BottomLeft);
        assert_eq!(panel.layout.width, Some(300.0));
        // the danger item uses the danger colour, the plain one the override
        assert_eq!(
            panel.children[0].element().children[1]
                .element()
                .text_style
                .color,
            ant_light().color_error
        );
        let plain = panel.children[1].element();
        assert_eq!(plain.children[1].element().text_style.font_size, 16.0);
        assert_eq!(plain.children[1].element().text_style.color, 0xff22_2222);
        assert_eq!(plain.children[2].element().text_style.font_size, 15.0);
        assert_eq!(plain.children[2].element().text_style.color, 0xff33_3333);
    }

    #[test]
    fn menu_callbacks_run_against_the_root_state() {
        let mut state = 0u32;
        let node = dropdown_menu(MenuOptions {
            items: vec![MenuItem {
                on_click: Some(action(|s: &mut u32| *s += 1)),
                index_action: Some(index_action(|s: &mut u32, i| *s = 10 + i as u32)),
                index: 4,
                ..Default::default()
            }],
            ..Default::default()
        });
        let hit = node.element().children[0].element().children[0].element();
        {
            let mut cx = ActionCx::new(&mut state);
            if let Some(callback) = hit.on_click.clone() {
                callback.call(&mut cx);
            }
        }
        assert_eq!(state, 1);
        {
            let mut cx = ActionCx::new(&mut state);
            if let Some(callback) = hit.index_action.clone() {
                callback.call(&mut cx, hit.index);
            }
        }
        assert_eq!(state, 14);
    }

    struct MenuApp;

    impl App for MenuApp {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![dropdown_menu(options())],
                ..Default::default()
            })
        }
    }

    /// Settles every presence transition so the next frame paints at rest.
    fn settle(runtime: &mut crate::runtime::Runtime) {
        for node in &mut runtime.tree.nodes {
            if !node.alive {
                continue;
            }
            if let Some(anim) = node.anim.as_mut() {
                anim.presence = motion::Animated::at(1.0);
                anim.fx = motion::Fx::default();
            }
        }
        runtime.tree.mark_paint_dirty();
    }

    fn paint(app: impl App, width: usize, height: usize) -> (crate::runtime::Runtime, Renderer) {
        let mut runtime = crate::runtime::Runtime::init(app, RunOptions::default());
        let mut renderer =
            Renderer::create_raster(width as u32, height as u32).expect("raster renderer");
        for _ in 0..2 {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            runtime.render_frame(canvas, width as f32, height as f32, 1.0);
            renderer.end();
            settle(&mut runtime);
        }
        (runtime, renderer)
    }

    #[test]
    fn menu_panel_paints_its_surface_and_selected_row() {
        let theme = ant_light();
        let (runtime, renderer) = paint(MenuApp, 260, 220);
        let panel = runtime.find_key("dropdown-menu").expect("panel");
        let bounds = runtime.tree.bounds(panel);
        assert_eq!(bounds.width, 220.0);
        // 4 px padding: a point just inside the panel edge is the surface
        let x = bounds.x as usize + 2;
        let y = (bounds.y + bounds.height * 0.5) as usize;
        let inside = renderer.pixel_at(x, y).expect("panel pixel");
        assert_eq!(inside, theme.color_bg_elevated);
        // the selected row's hit button paints the primary background
        let open = runtime.find_key("open").expect("first row");
        let row = runtime.tree.bounds(open);
        let selected = renderer
            .pixel_at(row.x as usize + 2, (row.y + row.height * 0.5) as usize)
            .expect("selected row pixel");
        assert_eq!(selected, theme.color_primary_bg);
        // outside the panel the root background shows
        let outside = renderer
            .pixel_at((bounds.right() + 20.0) as usize, 4)
            .expect("root pixel");
        assert_ne!(outside, CLEAR);
        assert_ne!(outside, theme.color_bg_elevated);
    }
}
