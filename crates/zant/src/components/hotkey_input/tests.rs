//! Unit tests.

use super::*;
use crate::core::events::{KeyEvent, Modifiers};
use crate::core::keys;
use crate::core::node::NodeKind;
use crate::theme::tokens::ant_light;
use crate::ui::actions::{Action, PointerAction, PointerInfo, action};
use crate::ui::commands::{CommandKind, take_commands};
use std::cell::Cell;
use std::rc::Rc;

fn modifiers(control: bool, alt: bool, shift: bool, meta: bool) -> Modifiers {
    Modifiers {
        control,
        alt,
        shift,
        meta,
        ..Default::default()
    }
}

fn key_event(key: Key, modifiers: Modifiers) -> KeyEvent {
    KeyEvent {
        key,
        modifiers,
        repeat: false,
    }
}

#[test]
fn shortcut_normalizes_keys_and_labels_them_per_platform() {
    let event = key_event(Key::Character('V'), modifiers(false, false, false, true));
    let shortcut = Shortcut::from_key_event(event).expect("cmd+V is a shortcut");
    assert_eq!(shortcut.key, Key::Character('v'), "letters are normalised");
    assert!(shortcut.is_valid());
    assert!(shortcut.eql(&Shortcut {
        key: Key::Character('v'),
        modifiers: modifiers(false, false, false, true),
    }));
    assert!(!shortcut.eql(&Shortcut {
        key: Key::Character('v'),
        modifiers: modifiers(false, false, false, false),
    }));
    let expected = if keys::IS_MACOS { "⌘V" } else { "Win+V" };
    assert_eq!(shortcut.label(), expected);

    let chord = Shortcut {
        key: Key::Character('v'),
        modifiers: modifiers(true, true, false, false),
    };
    let expected = if keys::IS_MACOS {
        "⌃⌥V"
    } else {
        "Ctrl+Alt+V"
    };
    assert_eq!(chord.label(), expected);

    // caps lock is dropped and does not take part in the comparison
    let mut mods = modifiers(false, false, false, true);
    mods.caps_lock = true;
    let with_caps = Shortcut::from_key_event(key_event(Key::Character('V'), mods))
        .expect("cmd+V with caps lock");
    assert!(!with_caps.modifiers.caps_lock);
    assert!(with_caps.eql(&shortcut));
}

#[test]
fn shortcut_validity_needs_a_supported_key_and_a_modifier() {
    let plain = Shortcut {
        key: Key::Character('v'),
        modifiers: Modifiers::default(),
    };
    assert!(!plain.is_valid(), "no ctrl / alt / meta");
    let shift_only = Shortcut {
        key: Key::Character('v'),
        modifiers: modifiers(false, false, true, false),
    };
    assert!(!shift_only.is_valid(), "shift does not count");
    assert!(
        Shortcut::from_key_event(key_event(Key::Character('v'), Modifiers::default())).is_none()
    );
    for key in [
        Key::Character('a'),
        Key::Character('0'),
        Key::Character(' '),
        Key::Character('/'),
        Key::Function(5),
        Key::Enter,
        Key::Tab,
        Key::Escape,
        Key::Left,
    ] {
        assert!(
            Shortcut {
                key,
                modifiers: modifiers(false, false, false, true),
            }
            .is_valid(),
            "{key:?} is a platform key"
        );
    }
    for key in [
        Key::Function(0),
        Key::Function(13),
        Key::Home,
        Key::End,
        Key::Backspace,
        Key::Delete,
        Key::PageUp,
        Key::PageDown,
        Key::Character('€'),
    ] {
        assert!(
            !Shortcut {
                key,
                modifiers: modifiers(false, false, false, true),
            }
            .is_valid(),
            "{key:?} is not a platform key"
        );
    }
}

#[test]
fn shortcut_labels_use_the_key_display_names() {
    assert_eq!(
        Shortcut {
            key: Key::Character(' '),
            modifiers: modifiers(false, false, false, true)
        }
        .label(),
        if keys::IS_MACOS {
            "⌘Space"
        } else {
            "Win+Space"
        }
    );
    assert_eq!(
        Shortcut {
            key: Key::Function(7),
            modifiers: modifiers(false, false, false, true)
        }
        .label(),
        if keys::IS_MACOS { "⌘F7" } else { "Win+F7" }
    );
    assert_eq!(
        Shortcut {
            key: Key::Escape,
            modifiers: modifiers(true, false, false, false)
        }
        .label(),
        if keys::IS_MACOS { "⌃Esc" } else { "Ctrl+Esc" }
    );
    assert_eq!(
        Shortcut {
            key: Key::Backspace,
            modifiers: modifiers(false, false, false, true)
        }
        .label(),
        if keys::IS_MACOS {
            "⌘backspace"
        } else {
            "Win+backspace"
        }
    );
}

#[test]
fn hotkey_input_option_defaults() {
    let defaults = HotkeyInputOptions::default();
    assert_eq!(defaults.key.as_str(), "hotkey-input");
    assert!(defaults.value.is_none());
    assert!(!defaults.recording);
    assert!(defaults.width.is_none());
    assert_eq!(defaults.height, 32.0);
    assert_eq!(defaults.placeholder, "Click to set a shortcut");
    assert_eq!(defaults.recording_text, "Press a shortcut…");
    assert!(defaults.on_start.is_none());
    assert!(defaults.on_record.is_none());
    assert!(defaults.on_cancel.is_none());
    assert!(defaults.on_clear.is_none());

    // the defaults follow the app locale; explicit texts win
    crate::i18n::builtin::testing::use_locale("zh-CN");
    let chinese = HotkeyInputOptions::default();
    assert_eq!(chinese.placeholder, "点击设置快捷键");
    assert_eq!(chinese.recording_text, "请按下快捷键…");
    let explicit = HotkeyInputOptions {
        placeholder: "Set".into(),
        ..Default::default()
    };
    assert_eq!(explicit.placeholder, "Set");
    assert_eq!(explicit.recording_text, "请按下快捷键…");
}

#[test]
fn hotkey_input_shows_placeholder_caps_or_the_recording_hint() {
    let theme = ant_light();
    // no shortcut: the placeholder in the disabled colour
    let placeholder = hotkey_input(HotkeyInputOptions::default());
    let element = placeholder.element();
    assert_eq!(element.kind, NodeKind::Container);
    assert_eq!(element.key.as_deref(), Some("hotkey-input"));
    assert_eq!(element.layout.height, Some(32.0));
    assert_eq!(element.visual.radius, 6.0);
    assert_eq!(element.layout.padding.left, 8.0);
    assert_eq!(element.layout.padding.right, 4.0);
    assert_eq!(element.visual.background, Some(theme.color_bg_container));
    assert_eq!(element.visual.border_color, Some(theme.color_border));
    assert_eq!(
        element.hover_visual.expect("hover look").border_color,
        Some(theme.color_primary_hover)
    );
    assert_eq!(element.cursor, Some(Cursor::PointingHand));
    assert!(element.focusable);
    let label = element.children[0].element();
    assert_eq!(label.content, "Click to set a shortcut");
    assert_eq!(label.text_style.color, theme.color_text_disabled);
    assert_eq!(label.text_style.font_size, 13.0);

    // a recorded shortcut: one cap per key
    let recorded = hotkey_input(HotkeyInputOptions {
        value: Some(Shortcut {
            key: Key::Character('v'),
            modifiers: modifiers(false, true, false, true),
        }),
        ..Default::default()
    });
    let caps = recorded.element().children[0].element();
    assert_eq!(caps.layout.gap, 4.0);
    assert_eq!(caps.layout.align_items, flex::Align::Center);
    assert_eq!(caps.children.len(), 3, "one cap per key");
    let first_cap = caps.children[0].element();
    assert_eq!(first_cap.layout.padding, inset_xy(6.0, 1.0));
    assert_eq!(first_cap.visual.radius, 4.0);
    assert_eq!(first_cap.visual.background, Some(theme.color_fill_tertiary));
    assert_eq!(
        first_cap.visual.border_color,
        Some(theme.color_border_secondary)
    );
    let cap_label = first_cap.children[0].element();
    assert_eq!(cap_label.text_style.font_size, 12.0);
    assert_eq!(cap_label.text_style.weight, Weight::Medium);

    // recording: the hint in the primary colour and the active look
    let recording = hotkey_input(HotkeyInputOptions {
        recording: true,
        ..Default::default()
    });
    let element = recording.element();
    assert_eq!(element.visual.background, Some(theme.color_primary_bg));
    assert_eq!(element.visual.border_color, Some(theme.color_primary));
    assert!(
        element.hover_visual.is_none(),
        "recording: no hover border override"
    );
    let hint = element.children[0].element();
    assert_eq!(hint.content, "Press a shortcut…");
    assert_eq!(hint.text_style.color, theme.color_primary);
}

#[test]
fn hotkey_input_shows_a_clear_button_only_with_a_value() {
    let clear = action(|s: &mut u32| *s += 1);
    let cleared = hotkey_input(HotkeyInputOptions {
        value: Some(Shortcut {
            key: Key::Character('v'),
            modifiers: modifiers(false, false, false, true),
        }),
        on_clear: Some(clear.clone()),
        ..Default::default()
    });
    let children = &cleared.element().children;
    let button = children.last().expect("clear button");
    let element = button.element();
    assert_eq!(element.key.as_deref(), Some("hotkey-input-clear"));
    assert_eq!(element.icon, Icon::Close);
    assert_eq!(element.icon_size, 14.0);
    assert_eq!(element.layout.width, Some(22.0));
    assert_eq!(element.layout.height, Some(22.0));
    let tooltip = element.tooltip.as_deref();
    assert_eq!(tooltip, Some("Clear"));
    assert!(element.on_click.is_some());

    // no clear button while recording or without a value
    let recording = hotkey_input(HotkeyInputOptions {
        value: Some(Shortcut {
            key: Key::Character('v'),
            modifiers: modifiers(false, false, false, true),
        }),
        recording: true,
        on_clear: Some(clear),
        ..Default::default()
    });
    assert!(
        !recording
            .element()
            .children
            .iter()
            .any(|child| child.element().key.as_deref() == Some("hotkey-input-clear"))
    );
}

fn handlers(node: &Node) -> (PointerAction, KeyAction) {
    let element = node.element();
    (
        element.pointer.clone().expect("pointer area").action,
        element.key_action.clone().expect("key action"),
    )
}

#[test]
fn hotkey_input_pointer_starts_recording_and_focuses_the_field() {
    let _ = take_commands();
    let starts = Rc::new(Cell::new(0));
    let counter = Rc::clone(&starts);
    let node = hotkey_input(HotkeyInputOptions {
        on_start: Some(Action::new(move |_| counter.set(counter.get() + 1))),
        ..Default::default()
    });
    let (pointer, _) = handlers(&node);
    let info = PointerInfo {
        phase: PointerPhase::Up,
        x: 5.0,
        y: 5.0,
        width: 200.0,
        height: 32.0,
        ..Default::default()
    };
    let mut state = 0u32;
    let mut cx = ActionCx::new(&mut state);
    assert!(pointer.call(&mut cx, info));
    assert_eq!(starts.get(), 1);
    let commands = take_commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].kind, CommandKind::Focus);
    assert_eq!(commands[0].key(), "hotkey-input");
    // a press outside the field, a non-up phase, or recording: no start
    assert!(!pointer.call(&mut cx, PointerInfo { x: 300.0, ..info }));
    assert!(!pointer.call(
        &mut cx,
        PointerInfo {
            phase: PointerPhase::Down,
            ..info
        }
    ));
    assert_eq!(starts.get(), 1);

    let other = Rc::clone(&starts);
    let recording = hotkey_input(HotkeyInputOptions {
        recording: true,
        on_start: Some(Action::new(move |_| other.set(other.get() + 1))),
        ..Default::default()
    });
    let (pointer, _) = handlers(&recording);
    assert!(!pointer.call(&mut cx, info));
    assert_eq!(starts.get(), 1);
}

#[test]
fn hotkey_input_key_starts_recording_on_enter_or_space_when_idle() {
    let starts = Rc::new(Cell::new(0));
    let counter = Rc::clone(&starts);
    let node = hotkey_input(HotkeyInputOptions {
        on_start: Some(Action::new(move |_| counter.set(counter.get() + 1))),
        ..Default::default()
    });
    let (_, on_key) = handlers(&node);
    let mut state = 0u32;
    let mut cx = ActionCx::new(&mut state);
    assert!(on_key.call(&mut cx, key_event(Key::Enter, Modifiers::default())));
    assert_eq!(starts.get(), 1);
    assert!(on_key.call(
        &mut cx,
        key_event(Key::Character(' '), Modifiers::default())
    ));
    assert_eq!(starts.get(), 2);
    assert!(!on_key.call(
        &mut cx,
        key_event(Key::Character('a'), Modifiers::default())
    ));
    assert_eq!(starts.get(), 2, "other keys are not consumed");
}

#[test]
fn hotkey_input_key_records_cancels_and_clears_while_recording() {
    let cancels = Rc::new(Cell::new(0));
    let cancels_c = Rc::clone(&cancels);
    let clears = Rc::new(Cell::new(0));
    let clears_c = Rc::clone(&clears);
    let recorded = Rc::new(std::cell::RefCell::new(None));
    let recorded_c = Rc::clone(&recorded);
    let node = hotkey_input(HotkeyInputOptions {
        recording: true,
        on_cancel: Some(Action::new(move |_| cancels_c.set(cancels_c.get() + 1))),
        on_clear: Some(Action::new(move |_| clears_c.set(clears_c.get() + 1))),
        on_record: Some(HotkeyAction::new(move |_, shortcut| {
            *recorded_c.borrow_mut() = Some(shortcut);
        })),
        ..Default::default()
    });
    let (_, on_key) = handlers(&node);
    let mut state = 0u32;
    let mut cx = ActionCx::new(&mut state);
    // plain Esc cancels, and is consumed
    assert!(on_key.call(&mut cx, key_event(Key::Escape, Modifiers::default())));
    assert_eq!(cancels.get(), 1);
    // plain Backspace / Delete clear
    assert!(on_key.call(&mut cx, key_event(Key::Backspace, Modifiers::default())));
    assert!(on_key.call(&mut cx, key_event(Key::Delete, Modifiers::default())));
    assert_eq!(clears.get(), 2);
    // Ctrl+Esc falls through to the recorder
    assert!(on_key.call(
        &mut cx,
        key_event(Key::Escape, modifiers(true, false, false, false))
    ));
    assert_eq!(cancels.get(), 1, "only the plain Esc cancels");
    let shortcut = (*recorded.borrow()).expect("Ctrl+Esc was recorded");
    assert_eq!(shortcut.key, Key::Escape);
    assert!(shortcut.modifiers.control);
    // a combination is recorded with its normalised key and consumed
    *recorded.borrow_mut() = None;
    assert!(on_key.call(
        &mut cx,
        key_event(Key::Character('V'), modifiers(false, true, false, true))
    ));
    let shortcut = (*recorded.borrow()).expect("Alt+Cmd+V was recorded");
    assert_eq!(shortcut.key, Key::Character('v'));
    assert!(shortcut.modifiers.alt && shortcut.modifiers.meta);
    // a plain key while recording is consumed but records nothing
    *recorded.borrow_mut() = None;
    assert!(on_key.call(
        &mut cx,
        key_event(Key::Character('x'), Modifiers::default())
    ));
    assert!(recorded.borrow().is_none());
}
