//! Clickable rows, the hotkey recorder, wrapped rows, images, segmented widths and buttons.

use super::*;

pub(super) struct Rows {
    pub(super) picked: Option<usize>,
    pub(super) deleted: usize,
    pub(super) recording: bool,
    pub(super) hotkey: Option<HotkeyShortcut>,
}

impl App for Rows {
    fn render(&mut self) -> Node {
        let mut children = Vec::new();
        for i in 0..2usize {
            children.push(clickable(ClickableOptions {
                r#box: BoxOptions {
                    key: Some(if i == 0 { "row-0" } else { "row-1" }.into()),
                    height: Some(40.0),
                    flex_direction: Some(zant::flex::Direction::Row),
                    children: zant::nodes![
                        row(BoxOptions {
                            grow: 1.0,
                            ..Default::default()
                        }),
                        icon_button(IconButtonOptions {
                            key: Some(if i == 0 { "del-0" } else { "del-1" }.into()),
                            icon: Icon::Trash,
                            on_click: Some(action(|state: &mut Rows| state.deleted += 1)),
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                },
                index_action: Some(index_action(|state: &mut Rows, index: usize| {
                    state.picked = Some(index)
                })),
                index: i,
                ..Default::default()
            }));
        }
        children.push(hotkey_input(HotkeyInputOptions {
            key: "hk".into(),
            value: self.hotkey,
            recording: self.recording,
            on_start: Some(action(|state: &mut Rows| state.recording = true)),
            on_record: Some(HotkeyAction::new(|cx, shortcut| {
                let state = cx.state::<Rows>();
                state.hotkey = Some(shortcut);
                state.recording = false;
            })),
            ..Default::default()
        }));
        column(BoxOptions {
            align_items: zant::flex::Align::Stretch,
            children,
            ..Default::default()
        })
    }
}

pub(super) fn click(runtime: &mut Runtime, id: NodeId) {
    let b = runtime.tree.get(id).bounds;
    let p = zant::core::events::PointerEvent {
        x: b.x + b.width * 0.5,
        y: b.y + b.height * 0.5,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerMove(p));
    runtime.dispatch(Event::PointerDown(p));
    runtime.dispatch(Event::PointerUp(p));
}

#[test]
fn clickable_rows_and_the_hotkey_recorder() {
    motion::set_enabled(false);
    let mut runtime = Runtime::init(
        Rows {
            picked: None,
            deleted: 0,
            recording: false,
            hotkey: None,
        },
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 300.0,
                height: 200.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(300, 200).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let row1 = runtime.tree.id_of_key("row-1").expect("row-1");
    click(&mut runtime, row1);
    assert_eq!(runtime.app_state_mut::<Rows>().picked, Some(1));
    // a button inside the row takes its own click
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let del = runtime.tree.id_of_key("del-0").expect("del-0");
    click(&mut runtime, del);
    assert_eq!(runtime.app_state_mut::<Rows>().deleted, 1);
    assert_eq!(runtime.app_state_mut::<Rows>().picked, Some(1));
    // click the recorder, press ⌃⌥K
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    let hk = runtime.tree.id_of_key("hk").expect("hotkey input");
    click(&mut runtime, hk);
    assert!(runtime.app_state_mut::<Rows>().recording);
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Character('k'),
        modifiers: Modifiers {
            control: true,
            alt: true,
            ..Default::default()
        },
        repeat: false,
    }));
    assert!(!runtime.app_state_mut::<Rows>().recording);
    let hotkey = runtime.app_state_mut::<Rows>().hotkey.expect("hotkey");
    assert!(hotkey.eql(&HotkeyShortcut {
        key: Key::Character('k'),
        modifiers: Modifiers {
            control: true,
            alt: true,
            ..Default::default()
        },
    }));
    frame(&mut runtime, &mut renderer, 300.0, 200.0);
    motion::set_enabled(true);
}

pub(super) struct WrappedRow;

impl App for WrappedRow {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            align_items: zant::flex::Align::Stretch,
            children: zant::nodes![row(BoxOptions {
                key: Some("row".into()),
                padding: zant::ui::inset_xy(16.0, 12.0),
                gap: 16.0,
                align_items: zant::flex::Align::Center,
                children: zant::nodes![
                    column(BoxOptions {
                        key: Some("text-col".into()),
                        grow: 1.0,
                        shrink: 1.0,
                        basis: Some(0.0),
                        gap: 3.0,
                        align_items: zant::flex::Align::Stretch,
                        children: zant::nodes![
                            zant::ui::text(
                                "自动同步",
                                TextOptions {
                                    size: 14.0,
                                    ..Default::default()
                                }
                            ),
                            zant::ui::text(
                                "同一局域网内打开 ClipHop 的设备会互相发现，并同步最新复制的一条文字；历史记录不会同步。",
                                TextOptions {
                                    size: 12.0,
                                    wrap: true,
                                    ..Default::default()
                                },
                            ),
                        ],
                        ..Default::default()
                    }),
                    zant::ui::r#switch(ToggleOptions {
                        key: Some("sw".into()),
                        checked: true,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn a_switch_after_a_growing_wrapped_text_column_stays_inside_the_row() {
    motion::set_enabled(false);
    let mut runtime = Runtime::init(
        WrappedRow,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 392.0,
                height: 200.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(392, 200).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 392.0, 200.0, 1.0);
        renderer.end();
    }
    let row_bounds = runtime.tree.bounds_of_key("row").expect("row");
    let col = runtime.tree.bounds_of_key("text-col").expect("text-col");
    let sw = runtime.tree.bounds_of_key("sw").expect("sw");
    assert!((sw.width - 40.0).abs() <= 0.5);
    assert!(sw.x + sw.width <= row_bounds.x + row_bounds.width - 16.0 + 0.5);
    // the text column takes the rest of the row
    assert!((row_bounds.width - 32.0 - 16.0 - 40.0 - col.width).abs() <= 0.5);
    motion::set_enabled(true);
}

#[test]
fn image_thumbnail_and_png_round_trip() {
    // 40×20 opaque red, encoded through Skia, thumbnailed to 8 px wide
    let mut pixels = [0u8; 40 * 20 * 4];
    for i in 0..40 * 20 {
        pixels[i * 4..][..4].copy_from_slice(if NATIVE_ORDER == PixelOrder::Rgba {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        });
    }
    let img = Image::from_pixels(&pixels, 40, 20, 40 * 4).expect("image");
    let png_bytes = img.encode_png().expect("png encode");
    assert!(png_bytes.starts_with(b"\x89PNG"));
    let thumb = Image::thumbnail(&png_bytes, 8).expect("thumbnail");
    assert_eq!(thumb.width, 8);
    assert_eq!(thumb.height, 4);
    // never enlarged
    let same = Image::thumbnail(&png_bytes, 100).expect("thumbnail");
    assert_eq!(same.width, 40);
}

pub(super) struct Seg;

impl App for Seg {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            key: Some("col".into()),
            align_items: zant::flex::Align::Stretch,
            gap: 8.0,
            children: zant::nodes![
                segmented(SegmentedOptions {
                    key: Some("inline".into()),
                    options: vec![
                        SegmentedOption {
                            label: "Small".into(),
                            ..Default::default()
                        },
                        SegmentedOption {
                            label: "Large".into(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                segmented(SegmentedOptions {
                    key: Some("block".into()),
                    block: true,
                    options: vec![
                        SegmentedOption {
                            label: "Small".into(),
                            ..Default::default()
                        },
                        SegmentedOption {
                            label: "Large".into(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn segmented_is_as_wide_as_its_options_unless_block() {
    motion::set_enabled(false);
    let mut runtime = Runtime::init(
        Seg,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 500.0,
                height: 200.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(500, 200).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 500.0, 200.0, 1.0);
        renderer.end();
    }
    let inline_track = runtime.tree.bounds_of_key("inline").expect("inline");
    let block_track = runtime.tree.bounds_of_key("block").expect("block");
    // the root column fills the 500px window
    assert!(inline_track.width < 200.0);
    assert_eq!(block_track.width, 500.0);
    motion::set_enabled(true);
}

pub(super) struct Buttons;

impl App for Buttons {
    fn render(&mut self) -> Node {
        row(BoxOptions {
            gap: 8.0,
            children: zant::nodes![
                button(ButtonOptions {
                    key: Some("plain".into()),
                    text: "Save".into(),
                    ..Default::default()
                }),
                button(ButtonOptions {
                    key: Some("icon".into()),
                    text: "Save".into(),
                    icon: Icon::Check,
                    ..Default::default()
                }),
                button(ButtonOptions {
                    key: Some("only".into()),
                    icon: Icon::Check,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn a_button_with_an_icon_and_a_label_is_wider_by_the_icon_and_its_gap() {
    motion::set_enabled(false);
    let mut runtime = Runtime::init(
        Buttons,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 500.0,
                height: 120.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(500, 120).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 500.0, 120.0, 1.0);
        renderer.end();
    }
    let plain = runtime.tree.bounds_of_key("plain").expect("plain");
    let icon = runtime.tree.bounds_of_key("icon").expect("icon");
    // a 16px icon (font size 14 + 2) and the 8px gap
    assert!((plain.width + 24.0 - icon.width).abs() <= 0.5);
    assert!(runtime.tree.bounds_of_key("only").expect("only").width < plain.width);
    motion::set_enabled(true);
}
