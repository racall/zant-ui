//! Sections of `zant-components`: data display and feedback.

use super::*;

/// The display section.
pub(super) fn display(app: &mut ComponentsApp, theme: Theme) -> ui::Node {
    let percent_text = format!("{}%", app.progress as u32);
    section(
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
                        src: "crates/zant-examples/assets/zant.png".into(),
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
    )
}

/// The feedback section.
pub(super) fn feedback(app: &mut ComponentsApp) -> ui::Node {
    section(
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
    )
}
