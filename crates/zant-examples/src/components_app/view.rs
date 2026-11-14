//! The UI tree of `zant-components` (Zig `render` / `section` / `label` /
//! `listRow` of `examples/components.zig`).
#![forbid(unsafe_code)]

use std::rc::Rc;

use zant::Str;
use zant::components::widgets::{MarkdownOptions, markdown};
use zant::core::geometry::Edges;
use zant::core::node::{ButtonSize, ButtonType, Icon};
use zant::layout::flex;
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

use super::{ComponentsApp, DISPLAY_ICONS, shortcut_groups};

/// `ui.card` section with a title (Zig `section`).
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

/// Zig `State.listRow`: one row of the virtual list.
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

/// Zig `render`.
pub(crate) fn render(app: &mut ComponentsApp) -> ui::Node {
    if app.autoplay {
        app.autoplay_tick();
    }
    let theme = ui::current_theme();

    // ---- buttons ----------------------------------------------------------
    let buttons = section(
        "按钮 Button / IconButton / Link",
        vec![
            row(
                8.0,
                vec![
                    ui::button(ButtonOptions {
                        key: Some("btn-primary".into()),
                        text: "Primary".into(),
                        variant: ButtonType::Primary,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_primary())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("btn-default".into()),
                        text: "Default".into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_default())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("btn-text".into()),
                        text: "Text".into(),
                        variant: ButtonType::Text,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_text())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("btn-link".into()),
                        text: "Link".into(),
                        variant: ButtonType::Link,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_link_button())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("btn-disabled".into()),
                        text: "Disabled".into(),
                        disabled: true,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_disabled())),
                        ..Default::default()
                    }),
                ],
            ),
            row(
                8.0,
                vec![
                    ui::button(ButtonOptions {
                        key: Some("btn-small".into()),
                        text: "Small".into(),
                        size: ButtonSize::Small,
                        variant: ButtonType::Primary,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_primary())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("btn-large".into()),
                        text: "Large".into(),
                        size: ButtonSize::Large,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_default())),
                        ..Default::default()
                    }),
                    ui::icon_button(IconButtonOptions {
                        key: Some("icon-btn".into()),
                        icon: Icon::Search,
                        tooltip: Some("搜索".into()),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_icon())),
                        ..Default::default()
                    }),
                    ui::link(LinkOptions {
                        key: Some("link".into()),
                        text: "文档链接".into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.click_link())),
                    }),
                ],
            ),
        ],
    );

    // ---- form -------------------------------------------------------------
    let form = section(
        "表单 Input / Checkbox / Switch / Radio / Select",
        vec![
            ui::input(InputOptions {
                key: Some("input".into()),
                placeholder: "输入文字后回车".into(),
                width: Some(320.0),
                on_change: Some(ui::text_action(|s: &mut ComponentsApp, value| {
                    s.on_input(value)
                })),
                on_submit: Some(ui::action(|s: &mut ComponentsApp| s.on_submit())),
                ..Default::default()
            }),
            ui::input(InputOptions {
                key: Some("input-password".into()),
                placeholder: "密码".into(),
                password: true,
                width: Some(320.0),
                ..Default::default()
            }),
            row(
                24.0,
                vec![
                    ui::checkbox(ToggleOptions {
                        key: Some("checkbox".into()),
                        label: "通知".into(),
                        checked: app.notifications,
                        on_change: Some(ui::bool_action(|s: &mut ComponentsApp, value| {
                            s.on_notify(value)
                        })),
                        ..Default::default()
                    }),
                    ui::r#switch(ToggleOptions {
                        key: Some("switch".into()),
                        label: "测试版".into(),
                        checked: app.beta,
                        on_change: Some(ui::bool_action(|s: &mut ComponentsApp, value| {
                            s.on_beta(value)
                        })),
                        ..Default::default()
                    }),
                ],
            ),
            ui::radio_group(RadioGroupOptions {
                key: Some("radio".into()),
                direction: flex::Direction::Row,
                gap: 16.0,
                selected: app.plan,
                options: vec![
                    RadioOption {
                        label: "基础".into(),
                        disabled: false,
                    },
                    RadioOption {
                        label: "专业".into(),
                        disabled: false,
                    },
                    RadioOption {
                        label: "企业".into(),
                        disabled: true,
                    },
                ],
                on_change: Some(ui::index_action(|s: &mut ComponentsApp, index| {
                    s.on_plan(index)
                })),
            }),
            ui::select(SelectOptions {
                key: Some("select".into()),
                width: Some(200.0),
                options: vec!["华东".into(), "华北".into(), "华南".into()],
                selected: app.region,
                on_change: Some(ui::index_action(|s: &mut ComponentsApp, index| {
                    s.on_region(index)
                })),
                ..Default::default()
            }),
        ],
    );

    // ---- navigation -------------------------------------------------------
    let nav = section(
        "导航 Segmented / Tabs",
        vec![
            ui::segmented(SegmentedOptions {
                key: Some("segmented".into()),
                selected: app.view,
                on_change: Some(ui::index_action(|s: &mut ComponentsApp, index| {
                    s.on_view(index)
                })),
                options: vec![
                    SegmentedOption {
                        label: "编辑".into(),
                        icon: Icon::Pencil,
                        tooltip: None,
                    },
                    SegmentedOption {
                        label: "分栏".into(),
                        icon: Icon::Columns,
                        tooltip: None,
                    },
                    SegmentedOption {
                        label: "预览".into(),
                        icon: Icon::BookOpen,
                        tooltip: None,
                    },
                ],
                ..Default::default()
            }),
            ui::tabs(TabsOptions {
                key: Some("tabs".into()),
                active: app.tab,
                on_change: Some(ui::index_action(|s: &mut ComponentsApp, index| {
                    s.on_tab(index)
                })),
                items: vec![
                    TabItem {
                        label: "概览".into(),
                        content: ui::text("第一个标签页的内容", TextOptions::default()),
                    },
                    TabItem {
                        label: "成员".into(),
                        content: ui::text("第二个标签页的内容", TextOptions::default()),
                    },
                ],
            }),
        ],
    );

    // ---- display ----------------------------------------------------------
    let percent_text = format!("{}%", app.progress as u32);
    let display = section(
        "展示 Tag / Avatar / Progress / Icon / Image / Tooltip",
        vec![
            row(
                8.0,
                vec![
                    ui::tag(TagOptions {
                        text: "默认".into(),
                        ..Default::default()
                    }),
                    // antd green / orange presets, light and dark
                    ui::tag(TagOptions {
                        text: "成功".into(),
                        color: Some(if app.dark { 0xff49aa19 } else { 0xff389e0d }),
                        background: Some(if app.dark { 0xff162312 } else { 0xfff6ffed }),
                        ..Default::default()
                    }),
                    ui::tag(TagOptions {
                        text: "警告".into(),
                        color: Some(if app.dark { 0xffd89614 } else { 0xffd46b08 }),
                        background: Some(if app.dark { 0xff2b1d11 } else { 0xfffff7e6 }),
                        ..Default::default()
                    }),
                    ui::avatar(AvatarOptions {
                        text: "Z".into(),
                        ..Default::default()
                    }),
                    ui::avatar(AvatarOptions {
                        text: "UI".into(),
                        size: 40.0,
                        background: Some(theme.color_primary),
                        ..Default::default()
                    }),
                ],
            ),
            row(
                12.0,
                vec![
                    ui::progress(ProgressOptions {
                        key: Some("progress".into()),
                        percent: app.progress,
                        width: 220.0,
                        ..Default::default()
                    }),
                    ui::text(
                        percent_text,
                        TextOptions {
                            size: 12.0,
                            ..Default::default()
                        },
                    ),
                    ui::button(ButtonOptions {
                        key: Some("progress-more".into()),
                        text: "+20%".into(),
                        size: ButtonSize::Small,
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.more_progress())),
                        ..Default::default()
                    }),
                ],
            ),
            row(
                10.0,
                DISPLAY_ICONS
                    .iter()
                    .map(|&name| {
                        ui::icon(IconOptions {
                            name,
                            ..Default::default()
                        })
                    })
                    .collect(),
            ),
            row(
                12.0,
                vec![
                    ui::image(ImageOptions {
                        key: Some("image".into()),
                        src: "examples/assets/zant.png".into(),
                        width: Some(120.0),
                        height: Some(68.0),
                        tooltip: Some("图片提示".into()),
                        ..Default::default()
                    }),
                    ui::tooltip(TooltipOptions {
                        text: "这是一个提示".into(),
                        child: ui::button(ButtonOptions {
                            key: Some("tooltip-btn".into()),
                            text: "悬停查看提示".into(),
                            ..Default::default()
                        }),
                    }),
                ],
            ),
        ],
    );

    // ---- feedback ---------------------------------------------------------
    let feedback = section(
        "反馈 Alert / Dropdown / Modal / Message",
        vec![
            ui::alert(AlertOptions {
                r#type: AlertType::Info,
                title: "信息".into(),
                description: "普通提示信息".into(),
                ..Default::default()
            }),
            ui::alert(AlertOptions {
                r#type: AlertType::Success,
                title: "成功".into(),
                description: "操作已完成".into(),
                ..Default::default()
            }),
            ui::alert(AlertOptions {
                r#type: AlertType::Warning,
                title: "警告".into(),
                ..Default::default()
            }),
            ui::alert(AlertOptions {
                r#type: AlertType::ErrorState,
                title: "错误".into(),
                description: "出现问题".into(),
                ..Default::default()
            }),
            ui::row(BoxOptions {
                gap: 8.0,
                children: vec![
                    ui::button(ButtonOptions {
                        key: Some("menu-btn".into()),
                        text: "下拉菜单".into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.open_menu())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("modal-btn".into()),
                        text: "对话框".into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.open_modal())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("message-btn".into()),
                        text: "全局提示".into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.show_message())),
                        ..Default::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some("dark-btn".into()),
                        text: if app.dark { "浅色" } else { "深色" }.into(),
                        on_click: Some(ui::action(|s: &mut ComponentsApp| s.toggle_dark())),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
        ],
    );

    // ---- interaction ------------------------------------------------------
    let track_w: f32 = 240.0;
    let interaction = section(
        "交互 PointerArea / 虚拟列表 / 可选择文字",
        vec![
            row(
                12.0,
                vec![
                    ui::pointer_area(PointerAreaOptions {
                        r#box: BoxOptions {
                            key: Some("track".into()),
                            width: Some(track_w),
                            height: Some(28.0),
                            justify: flex::Justify::Center,
                            children: vec![
                                ui::styled(
                                    ui::column(BoxOptions::default()),
                                    Style {
                                        height: Some(4.0),
                                        width_percent: Some(100.0),
                                        radius: Some(2.0),
                                        background: Some(theme.color_fill_secondary),
                                        ..Default::default()
                                    },
                                ),
                                ui::styled(
                                    ui::column(BoxOptions::default()),
                                    Style {
                                        absolute: Some(true),
                                        left: Some(app.knob * (track_w - 16.0)),
                                        top: Some(6.0),
                                        width: Some(16.0),
                                        height: Some(16.0),
                                        radius: Some(8.0),
                                        background: Some(theme.color_primary),
                                        shadow_color: Some(0x3300_0000),
                                        shadow_blur: Some(4.0),
                                        ..Default::default()
                                    },
                                ),
                            ],
                            ..Default::default()
                        },
                        cursor: Some(if app.knob_drag.is_some() {
                            Cursor::Grabbing
                        } else {
                            Cursor::Grab
                        }),
                        on_pointer: ui::PointerAction::new(|cx, info| {
                            cx.state::<ComponentsApp>().on_track(info)
                        }),
                        ..Default::default()
                    }),
                    ui::text(
                        format!("{}%", (app.knob * 100.0).round() as u32),
                        TextOptions {
                            size: 12.0,
                            color: Some(theme.color_text_secondary),
                            ..Default::default()
                        },
                    ),
                ],
            ),
            ui::column(BoxOptions {
                key: Some("list-box".into()),
                height: Some(160.0),
                radius: 8.0,
                border_color: Some(theme.color_border_secondary),
                border_width: 1.0,
                clip: true,
                align_items: flex::Align::Stretch,
                children: vec![ui::list(
                    &app.rows,
                    ListOptions {
                        key: "vlist".into(),
                        count: 10_000,
                        item: Rc::new(list_row),
                        item_height: Some(32.0),
                        on_scroll: Some(ui::action(|s: &mut ComponentsApp| s.list_scrolled())),
                        ..Default::default()
                    },
                )],
                ..Default::default()
            }),
            ui::column(BoxOptions {
                key: Some("select-text".into()),
                selectable: true,
                gap: 4.0,
                align_items: flex::Align::Stretch,
                children: vec![
                    ui::paragraph(
                        "这段文字可以拖动选择：双击选词，三击选整段，⌘C / Ctrl+C 复制。",
                        Default::default(),
                    ),
                    ui::paragraph(
                        "Text in a selectable container can be dragged across nodes.",
                        zant::ui::ParagraphOptions {
                            size: 13.0,
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            }),
        ],
    );

    // ---- documents --------------------------------------------------------
    let documents = section(
        "文档 Markdown / PdfViewer",
        vec![
            ui::button(ButtonOptions {
                key: Some("widget-append".into()),
                text: "追加 Markdown 文本".into(),
                on_click: Some(ui::action(|s: &mut ComponentsApp| s.append_widget())),
                ..Default::default()
            }),
            markdown(
                0x2001,
                MarkdownOptions {
                    source: if app.widget_chunk {
                        "**Markdown** · streamed chunk"
                    } else {
                        "**Markdown**"
                    }
                    .into(),
                    streaming: true,
                    caret: Caret::Block,
                    font_size: 14.0,
                    ..Default::default()
                },
            ),
            // Zig `ui.pdfViewer(0x2002, ...)`: the framework widget
            // (`crate::components::widgets`, exported as `ui::pdf_viewer`).
            pdf_viewer(
                0x2002,
                PdfViewerOptions {
                    src: Str::copy(&app.pdf_src),
                    height: Some(240.0),
                    on_change: Some(ui::action(|s: &mut ComponentsApp| s.pdf_changed())),
                    ..Default::default()
                },
            ),
        ],
    );

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
