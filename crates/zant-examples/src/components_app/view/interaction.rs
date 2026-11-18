//! Sections of `zant-components`: interaction (pointer areas, drag, hotkeys) and documents.

use super::*;

/// The interaction section.
pub(super) fn interaction(app: &mut ComponentsApp, theme: Theme) -> ui::Node {
    let track_w: f32 = 240.0;
    section(
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
    )
}

/// The documents section.
pub(super) fn documents(app: &mut ComponentsApp) -> ui::Node {
    section(
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
            // The framework's PDF reader widget (`ui::pdf_viewer`).
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
    )
}
