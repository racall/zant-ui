//! Unit tests.

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
