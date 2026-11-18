//! Modals, messages, focus scopes and dialog focus.

use super::*;

pub(super) struct Help {
    pub(super) open: bool,
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

pub(super) fn frame(runtime: &mut Runtime, renderer: &mut Renderer, w: f32, h: f32) {
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

pub(super) struct DialogApp {
    pub(super) open: bool,
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

/// A dialog with an `autoFocus` field (or none): opening it moves the
/// keyboard into it, closing it gives the focus back to the page.
pub(super) struct FocusDialog {
    pub(super) open: bool,
    pub(super) auto_focus: bool,
}

impl App for FocusDialog {
    fn render(&mut self) -> Node {
        let behind = button(ButtonOptions {
            key: Some("behind".into()),
            text: "Open".into(),
            ..Default::default()
        });
        let mut children = vec![behind];
        if self.open {
            children.push(modal(ModalOptions {
                key: "dlg".into(),
                title: "T".into(),
                content: zant::ui::input(zant::ui::InputOptions {
                    key: Some("dlg-input".into()),
                    auto_focus: self.auto_focus,
                    ..Default::default()
                }),
                ..Default::default()
            }));
        }
        column(BoxOptions {
            grow: 1.0,
            children,
            ..Default::default()
        })
    }
}

pub(super) fn focused_key(runtime: &Runtime) -> Option<String> {
    runtime
        .tree
        .focused
        .and_then(|id| runtime.tree.get(id).key.as_ref().map(|key| key.to_string()))
}

#[test]
fn a_dialog_takes_the_focus_and_gives_it_back_when_it_closes() {
    motion::set_enabled(false);
    for auto_focus in [true, false] {
        let mut runtime = Runtime::init(
            FocusDialog {
                open: false,
                auto_focus,
            },
            RunOptions::default(),
        );
        let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");
        frame(&mut runtime, &mut renderer, 400.0, 300.0);
        runtime.dispatch(Event::KeyDown(KeyEvent {
            key: Key::Tab,
            modifiers: Modifiers::default(),
            repeat: false,
        }));
        assert_eq!(focused_key(&runtime).as_deref(), Some("behind"));

        runtime.app_state_mut::<FocusDialog>().open = true;
        runtime.invalidate();
        frame(&mut runtime, &mut renderer, 400.0, 300.0);
        if auto_focus {
            assert_eq!(focused_key(&runtime).as_deref(), Some("dlg-input"));
        } else {
            // the page behind no longer has the keyboard; Tab enters the dialog
            assert_eq!(runtime.tree.focused, None);
        }

        runtime.app_state_mut::<FocusDialog>().open = false;
        runtime.invalidate();
        frame(&mut runtime, &mut renderer, 400.0, 300.0);
        assert_eq!(
            focused_key(&runtime).as_deref(),
            Some("behind"),
            "auto_focus: {auto_focus}"
        );
    }
    motion::set_enabled(true);
}
