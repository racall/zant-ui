//! Sections of `zant-components`: buttons, form controls and navigation.

use super::*;

/// The buttons section.
pub(super) fn buttons() -> ui::Node {
    section(
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
            row(8.0, crate::components_app::buttons::danger_buttons()),
            row(8.0, crate::components_app::buttons::loading_buttons()),
            crate::components_app::buttons::block_button(),
        ],
    )
}

/// The form section.
pub(super) fn form(app: &mut ComponentsApp) -> ui::Node {
    section(
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
    )
}

/// The navigation section.
pub(super) fn nav(app: &mut ComponentsApp) -> ui::Node {
    section(
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
    )
}
