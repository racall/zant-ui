//! The UI tree of `zant-components`.
#![forbid(unsafe_code)]

use std::rc::Rc;

use zant::Str;
use zant::components::widgets::{MarkdownOptions, markdown};
use zant::core::geometry::Edges;
use zant::core::node::{ButtonSize, ButtonType, Icon};
use zant::flex;
use zant::markdown::render::options::Caret;
use zant::platform::platform::Cursor;
use zant::renderer::text::Weight;
use zant::ui::{
    self, AlertOptions, AlertType, AvatarOptions, BoxOptions, ButtonOptions, CardOptions,
    IconButtonOptions, IconOptions, ImageOptions, InputOptions, LinkOptions, ListOptions, MenuItem,
    MenuOptions, MessageKind, MessageOptions, ModalOptions, PdfViewerOptions, PointerAreaOptions,
    ProgressOptions, RadioGroupOptions, RadioOption, SegmentedOption, SegmentedOptions,
    SelectOptions, ShortcutListOptions, Style, TabItem, TabsOptions, TagOptions, TextOptions,
    ToggleOptions, TooltipOptions, pdf_viewer,
};

use zant::theme::tokens::Theme;

use super::{ComponentsApp, DISPLAY_ICONS, shortcut_groups};

mod controls;
mod display;
mod interaction;

use self::controls::*;
use self::display::*;
use self::interaction::*;

/// A `ui::card` section with a title.
fn section(title: &str, children: Vec<ui::Node>) -> ui::Node {
    ui::card(CardOptions {
        title: Some(Str::copy(title)),
        gap: 12.0,
        padding: Edges::all(16.0),
        children,
        ..Default::default()
    })
}

/// A row with `gap` / centered children (the example's common `ui.row`).
fn row(gap: f32, children: Vec<ui::Node>) -> ui::Node {
    ui::row(BoxOptions {
        gap,
        align_items: flex::Align::Center,
        children,
        ..Default::default()
    })
}

/// One row of the virtual list.
fn list_row(i: usize) -> ui::Node {
    let theme = ui::current_theme();
    ui::row(BoxOptions {
        height: Some(32.0),
        padding: ui::inset_xy(12.0, 0.0),
        gap: 8.0,
        align_items: flex::Align::Center,
        radius: 6.0,
        hover_background: Some(theme.color_fill_secondary),
        children: vec![
            ui::text(
                format!("第 {} 行", i + 1),
                TextOptions {
                    size: 13.0,
                    ..Default::default()
                },
            ),
            ui::text(
                "虚拟列表只构建可见的行",
                TextOptions {
                    size: 12.0,
                    color: Some(theme.color_text_secondary),
                    ..Default::default()
                },
            ),
        ],
        ..Default::default()
    })
}

/// The whole window tree.
pub(crate) fn render(app: &mut ComponentsApp) -> ui::Node {
    if app.autoplay {
        app.autoplay_tick();
    }
    let theme = ui::current_theme();

    let buttons = buttons();
    let form = form(app);
    let nav = nav(app);
    let display = display(app, theme);
    let feedback = feedback(app);
    let interaction = interaction(app, theme);
    let documents = documents(app);

    let mut children: Vec<ui::Node> = vec![ui::column(BoxOptions {
        key: Some("page".into()),
        grow: 1.0,
        scroll: true,
        align_items: flex::Align::Stretch,
        padding: Edges {
            left: 24.0,
            right: 24.0,
            top: 20.0,
            bottom: 24.0,
        },
        gap: 16.0,
        children: vec![
            row(
                12.0,
                vec![
                    ui::text(
                        "组件总览",
                        TextOptions {
                            size: 20.0,
                            weight: Weight::Semibold,
                            ..Default::default()
                        },
                    ),
                    ui::text(
                        app.last_event()
                            .map(Str::copy)
                            .unwrap_or_else(|| Str::Static("（点击任意组件）")),
                        TextOptions {
                            key: Some("last-event".into()),
                            size: 12.0,
                            color: Some(theme.color_text_secondary),
                            ..Default::default()
                        },
                    ),
                ],
            ),
            ui::row(BoxOptions {
                gap: 16.0,
                align_items: flex::Align::Start,
                children: vec![
                    ui::column(BoxOptions {
                        grow: 1.0,
                        basis: Some(0.0),
                        gap: 16.0,
                        align_items: flex::Align::Stretch,
                        children: vec![buttons, form, nav],
                        ..Default::default()
                    }),
                    ui::column(BoxOptions {
                        grow: 1.0,
                        basis: Some(0.0),
                        gap: 16.0,
                        align_items: flex::Align::Stretch,
                        children: vec![display, feedback, interaction],
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            documents,
        ],
        ..Default::default()
    })];

    if app.menu_open {
        children.push(ui::dismiss_layer(zant::ui::DismissLayerOptions {
            on_dismiss: Some(ui::action(|s: &mut ComponentsApp| s.close_menu())),
            ..Default::default()
        }));
        // below its button (antd placement bottomLeft), above it when the
        // window has no room; boxes come from the previous frame's layout
        let menu_w: f32 = 200.0;
        let measured = ui::key_bounds("menu");
        let menu_h: f32 = measured.map_or(160.0, |m| m.height);
        // first frame: place with an estimate, rebuild once laid out
        if measured.is_none() {
            ui::wake_after(1);
        }
        let mut left: f32 = 40.0;
        let mut top: f32 = 120.0;
        let mut above = false;
        if let Some(button_bounds) = ui::key_bounds("menu-btn") {
            let page_h = ui::key_bounds("page").map_or(820.0, |page| page.height);
            left = button_bounds.x;
            above = button_bounds.bottom() + 4.0 + menu_h > page_h - 8.0;
            top = if above {
                button_bounds.y - 4.0 - menu_h
            } else {
                button_bounds.bottom() + 4.0
            };
        }
        children.push(ui::styled(
            ui::dropdown_menu(MenuOptions {
                key: Some("menu".into()),
                width: menu_w,
                motion_origin: if above {
                    zant::motion::motion::Origin::BottomLeft
                } else {
                    zant::motion::motion::Origin::TopLeft
                },
                items: vec![
                    MenuItem::group("文件"),
                    MenuItem {
                        key: Some("menu-new".into()),
                        label: "新建".into(),
                        icon: Icon::File,
                        extra: zant::core::keys::label("mod+n").into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.menu_new())),
                        ..Default::default()
                    },
                    MenuItem {
                        key: Some("menu-disabled".into()),
                        label: "不可用".into(),
                        icon: Icon::Settings,
                        disabled: true,
                        ..Default::default()
                    },
                    MenuItem::DIVIDER,
                    MenuItem {
                        key: Some("menu-delete".into()),
                        label: "删除".into(),
                        icon: Icon::Trash,
                        danger: true,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.menu_delete())),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            Style {
                absolute: Some(true),
                top: Some(top),
                left: Some(left),
                ..Default::default()
            },
        ));
    }
    if app.modal_open {
        children.push(ui::modal(ModalOptions {
            key: "modal".into(),
            title: "键盘快捷键".into(),
            width: None,
            on_close: Some(ui::action(|s: &mut ComponentsApp| s.close_modal())),
            content: ui::shortcut_list(ShortcutListOptions {
                key: "modal-shortcuts".into(),
                groups: shortcut_groups(),
                ..Default::default()
            }),
            footer: Some(ui::button(ButtonOptions {
                key: Some("modal-ok".into()),
                text: "知道了".into(),
                variant: ButtonType::Primary,
                on_click: Some(ui::action(|s: &mut ComponentsApp| s.close_modal())),
                ..Default::default()
            })),
            ..Default::default()
        }));
    }
    let now = ui::now_ms();
    if now < app.message_until_ms {
        children.push(ui::message(MessageOptions {
            key: "message".into(),
            text: "已保存".into(),
            kind: MessageKind::Success,
            ..Default::default()
        }));
        // the earlier of the autoplay step and the message expiry wins
        let autoplay_ms = if app.autoplay {
            (app.autoplay_at_ms - now).max(1)
        } else {
            app.message_until_ms - now
        };
        ui::wake_after((app.message_until_ms - now).min(autoplay_ms) as u32);
    }

    ui::column(BoxOptions {
        grow: 1.0,
        align_items: flex::Align::Stretch,
        children,
        ..Default::default()
    })
}
