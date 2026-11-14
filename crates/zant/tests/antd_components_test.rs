//! Port of `tests/antd_components_test.zig` — the antd composite components:
//! icons, dropdown menu, segmented, dismiss layer, modal, shortcut list,
//! message, focus scope, clickable rows, hotkey recorder, image thumbnails and
//! button icon metrics.

use zant::components::hotkey_input::{
    HotkeyAction, HotkeyInputOptions, Shortcut as HotkeyShortcut, hotkey_input,
};
use zant::components::menu::{MenuItem, MenuOptions, dropdown_menu};
use zant::components::message::{MessageKind, MessageOptions, message};
use zant::components::modal::{ModalOptions, modal};
use zant::components::segmented::{SegmentedOption, SegmentedOptions, segmented};
use zant::components::shortcut_list::shortcut_list;
use zant::components::shortcut_list::{
    Shortcut as ShortcutItem, ShortcutGroup, ShortcutListOptions,
};
use zant::core::events::{Event, Key, KeyEvent, Modifiers};
use zant::core::node::{ButtonType, FlexStyle, NodeId};
use zant::core::tree::Tree;
use zant::image::{Image, NATIVE_ORDER, PixelOrder};
use zant::motion::motion::{self, Presence};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::theme::tokens::ant_light;
use zant::ui::actions::{action, index_action};
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{
    BoxOptions, ButtonOptions, ClickableOptions, DismissLayerOptions, IconButtonOptions,
    IconOptions, TextOptions, ToggleOptions,
};
use zant::ui::{Icon, button, clickable, column, dismiss_layer, icon, icon_button, row};

#[test]
fn icons_build_for_every_name() {
    for name in Icon::ALL {
        let node = icon(IconOptions {
            name,
            size: 18.0,
            ..Default::default()
        });
        let drawing = node.element().drawing.as_ref().expect("icon drawing");
        assert_eq!(drawing.width, 18.0);
    }
}

struct Clicks {
    clicks: usize,
}

impl App for Clicks {
    fn render(&mut self) -> Node {
        column(BoxOptions::default())
    }
}

#[test]
fn dropdown_menu_segmented_and_dismiss_layer() {
    let _state = Clicks { clicks: 0 };
    let menu = dropdown_menu(MenuOptions {
        items: vec![
            MenuItem {
                label: "New".into(),
                icon: Icon::File,
                extra: "⌘N".into(),
                on_click: Some(action(|state: &mut Clicks| state.clicks += 1)),
                ..Default::default()
            },
            MenuItem::DIVIDER,
            MenuItem::group("View"),
            MenuItem {
                label: "Disabled".into(),
                disabled: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    assert_eq!(menu.element().children.len(), 4);
    let seg = segmented(SegmentedOptions {
        options: vec![
            SegmentedOption {
                label: "A".into(),
                ..Default::default()
            },
            SegmentedOption {
                label: "B".into(),
                ..Default::default()
            },
        ],
        selected: 1,
        ..Default::default()
    });
    // inline (antd default): a wrapper row around the track; the track holds
    // the sliding thumb + 2 options
    assert_eq!(seg.element().children.len(), 1);
    assert_eq!(seg.element().children[0].element().children.len(), 3);
    let block = segmented(SegmentedOptions {
        options: vec![
            SegmentedOption {
                label: "A".into(),
                ..Default::default()
            },
            SegmentedOption {
                label: "B".into(),
                ..Default::default()
            },
        ],
        block: true,
        ..Default::default()
    });
    assert_eq!(block.element().children.len(), 3);
    let layer = dismiss_layer(DismissLayerOptions {
        on_dismiss: Some(action(|state: &mut Clicks| state.clicks += 1)),
        ..Default::default()
    });
    assert!(layer.element().layout.absolute);
    assert_eq!(layer.element().button_type, ButtonType::Plain);
}

#[test]
fn block_pointer_containers_swallow_clicks_on_empty_areas() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let backdrop = tree.add_container(
        Some(root),
        FlexStyle {
            width: Some(200.0),
            height: Some(100.0),
            ..Default::default()
        },
    );
    tree.get_mut(backdrop).visual.block_pointer = true;
    let plain = tree.add_container(
        Some(root),
        FlexStyle {
            width: Some(200.0),
            height: Some(100.0),
            ..Default::default()
        },
    );
    tree.layout(zant::core::geometry::Size {
        width: 300.0,
        height: 300.0,
    });
    assert_eq!(tree.hit_test(10.0, 10.0), Some(backdrop));
    assert_eq!(tree.hit_test(10.0, tree.get(plain).bounds.y + 10.0), None);
}

struct Dim;

impl App for Dim {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            grow: 1.0,
            background: Some(0xffffffff),
            children: zant::nodes![dismiss_layer(DismissLayerOptions {
                background: Some(0xff000000),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn dismiss_layer_paints_its_dimming_background() {
    let mut runtime = Runtime::init(
        Dim,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 100.0,
                height: 100.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(100, 100).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 100.0, 100.0, 1.0);
        renderer.end();
    }
    assert_eq!(renderer.pixel_at(50, 50), Some(0xff000000));
}

struct Help {
    open: bool,
}

impl App for Help {
    fn render(&mut self) -> Node {
        if !self.open {
            return column(BoxOptions::default());
        }
        column(BoxOptions {
            grow: 1.0,
            children: zant::nodes![modal(ModalOptions {
                key: "help".into(),
                title: "Keys".into(),
                width: Some(300.0),
                on_close: Some(action(|state: &mut Help| state.open = false)),
                content: shortcut_list(ShortcutListOptions {
                    key: "keys".into(),
                    column_width: 120.0,
                    groups: vec![
                        ShortcutGroup {
                            title: "A".into(),
                            items: vec![
                                ShortcutItem {
                                    label: "one".into(),
                                    keys: "⌘1".into(),
                                },
                                ShortcutItem {
                                    label: "two".into(),
                                    keys: "⌘2".into(),
                                },
                            ],
                        },
                        ShortcutGroup {
                            title: "B".into(),
                            items: vec![ShortcutItem {
                                label: "three".into(),
                                keys: "⌘3".into(),
                            }],
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

fn frame(runtime: &mut Runtime, renderer: &mut Renderer, w: f32, h: f32) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, w, h, 1.0);
    renderer.end();
}

#[test]
fn modal_mask_closes_card_swallows_clicks_shortcut_list_balances_columns() {
    motion::set_enabled(false);
    let mut runtime = Runtime::init(
        Help { open: true },
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 600.0,
                height: 400.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(600, 400).expect("raster renderer");
    frame(&mut runtime, &mut renderer, 600.0, 400.0);
    // two groups → two columns
    let list_id = runtime.tree.id_of_key("keys").expect("shortcut list");
    assert_eq!(runtime.tree.get(list_id).children.len(), 2);
    let card_id = runtime.tree.id_of_key("help-card").expect("modal card");
    let c = runtime.tree.get(card_id).bounds;
    assert!((c.width - 300.0).abs() <= 0.5);
    let inside = zant::core::events::PointerEvent {
        x: c.x + 10.0,
        y: c.y + c.height - 5.0,
        ..Default::default()
    };
    // clicking inside the card does nothing; the mask closes
    runtime.dispatch(Event::PointerDown(inside));
    runtime.dispatch(Event::PointerUp(inside));
    assert!(runtime.app_state_mut::<Help>().open);
    let mask = zant::core::events::PointerEvent {
        x: 5.0,
        y: 5.0,
        ..Default::default()
    };
    runtime.dispatch(Event::PointerDown(mask));
    runtime.dispatch(Event::PointerUp(mask));
    assert!(!runtime.app_state_mut::<Help>().open);
    motion::set_enabled(true);
}

#[test]
fn message_is_a_pass_through_pill_with_a_drop_in_presence() {
    let node = message(MessageOptions {
        key: "msg".into(),
        text: "Saved".into(),
        kind: MessageKind::Err,
        top: 60.0,
        ..Default::default()
    });
    let e = node.element();
    assert!(e.layout.absolute);
    assert_eq!(e.layout.top, Some(60.0));
    assert_eq!(e.motion.presence, Presence::MoveUp);
    // the full-width row itself never catches clicks
    assert!(!e.visual.block_pointer);
}

struct DialogApp {
    open: bool,
}

impl App for DialogApp {
    fn render(&mut self) -> Node {
        let behind = row(BoxOptions {
            gap: 8.0,
            children: zant::nodes![
                button(ButtonOptions {
                    key: Some("behind-a".into()),
                    text: "A".into(),
                    ..Default::default()
                }),
                button(ButtonOptions {
                    key: Some("behind-b".into()),
                    text: "B".into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });
        if !self.open {
            return column(BoxOptions {
                grow: 1.0,
                children: zant::nodes![behind],
                ..Default::default()
            });
        }
        column(BoxOptions {
            grow: 1.0,
            children: zant::nodes![
                behind,
                modal(ModalOptions {
                    key: "dlg".into(),
                    title: "T".into(),
                    content: zant::ui::input(zant::ui::InputOptions {
                        key: Some("dlg-input".into()),
                        ..Default::default()
                    }),
                    footer: Some(zant::ui::button(ButtonOptions {
                        key: Some("dlg-ok".into()),
                        text: "OK".into(),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn modal_is_a_focus_scope_tab_cycles_inside_the_dialog_only() {
    motion::set_enabled(false);
    let mut runtime = Runtime::init(
        DialogApp { open: true },
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 600.0,
                height: 400.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    runtime.rebuild_for_test();
    let mut seen: [String; 8] = std::array::from_fn(|_| String::new());
    for slot in &mut seen {
        runtime.dispatch(Event::KeyDown(KeyEvent {
            key: Key::Tab,
            modifiers: Modifiers::default(),
            repeat: false,
        }));
        let id = runtime.tree.focused.expect("focused node");
        let key = runtime
            .tree
            .get(id)
            .key
            .as_ref()
            .map(|key| key.to_string())
            .unwrap_or_else(|| "-".to_string());
        assert!(!key.starts_with("behind"), "Tab reached {key}");
        *slot = key;
    }
    // Shift+Tab also stays inside
    runtime.dispatch(Event::KeyDown(KeyEvent {
        key: Key::Tab,
        modifiers: Modifiers {
            shift: true,
            ..Default::default()
        },
        repeat: false,
    }));
    let id = runtime.tree.focused.expect("focused node");
    let key = runtime
        .tree
        .get(id)
        .key
        .as_ref()
        .map(|key| key.to_string())
        .unwrap_or_else(|| "-".to_string());
    assert!(!key.starts_with("behind"), "Shift+Tab reached {key}");
    motion::set_enabled(true);
}

struct Rows {
    picked: Option<usize>,
    deleted: usize,
    recording: bool,
    hotkey: Option<HotkeyShortcut>,
}

impl App for Rows {
    fn render(&mut self) -> Node {
        let mut children = Vec::new();
        for i in 0..2usize {
            children.push(clickable(ClickableOptions {
                r#box: BoxOptions {
                    key: Some(if i == 0 { "row-0" } else { "row-1" }.into()),
                    height: Some(40.0),
                    flex_direction: Some(zant::layout::flex::Direction::Row),
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
            align_items: zant::layout::flex::Align::Stretch,
            children,
            ..Default::default()
        })
    }
}

fn click(runtime: &mut Runtime, id: NodeId) {
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

struct WrappedRow;

impl App for WrappedRow {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            align_items: zant::layout::flex::Align::Stretch,
            children: zant::nodes![row(BoxOptions {
                key: Some("row".into()),
                padding: zant::ui::inset_xy(16.0, 12.0),
                gap: 16.0,
                align_items: zant::layout::flex::Align::Center,
                children: zant::nodes![
                    column(BoxOptions {
                        key: Some("text-col".into()),
                        grow: 1.0,
                        shrink: 1.0,
                        basis: Some(0.0),
                        gap: 3.0,
                        align_items: zant::layout::flex::Align::Stretch,
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

struct Seg;

impl App for Seg {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            key: Some("col".into()),
            align_items: zant::layout::flex::Align::Stretch,
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

struct Buttons;

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
