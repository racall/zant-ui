//! The shortcut recorder.
//!
//! A settings field for a global hotkey: shows the current combination as key
//! caps; click it, press a new combination, done. Recording state lives in the
//! app (`recording`), like every Zant control.
//!
//! ```ignore
//! ui::hotkey_input(HotkeyInputOptions {
//!     value: self.hotkey,
//!     recording: self.recording,
//!     on_start: Some(ui::action(|s: &mut State| s.start_recording())),
//!     on_record: Some(ui::hotkey_action(|s: &mut State, shortcut| s.set_hotkey(shortcut))),
//!     on_cancel: Some(ui::action(|s: &mut State| s.stop_recording())),
//!     on_clear: Some(ui::action(|s: &mut State| s.clear_hotkey())),
//!     ..Default::default()
//! })
//! ```
//!
//! While recording: a combination with Ctrl / Alt (⌥) / ⌘ (Win) records it,
//! Esc cancels, Backspace / Delete clears. Unregister the app's own hotkey in
//! `on_start`: a registered hotkey never reaches the window.
//!
//! The recorder only needs `from_key_event` / `is_valid` / `eql` / `label`,
//! so this module carries its own [`Shortcut`] with that subset (same
//! normalisation and labels as `crate::desktop::Shortcut`, which adds
//! `format` / `parse`).
//!
//! TODO: use `crate::desktop::Shortcut` here and re-export it instead of
//! keeping two `Shortcut` types.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::core::events::{self, Key, KeyEvent, Modifiers};
use crate::core::geometry::Edges;
use crate::core::keys;
use crate::core::node::Icon;
use crate::layout::flex;
use crate::platform::platform::Cursor;
use crate::renderer::text::Weight;
use crate::ui::actions::{Action, ActionCx, KeyAction, PointerAction, PointerPhase};
use crate::ui::builders::{icon_button, pointer_area, row, text};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, IconButtonOptions, PointerAreaOptions, TextOptions};
use crate::ui::styled::inset_xy;

/// A key combination, e.g. ⌥⌘V / Ctrl+Alt+V (see the module note about
/// `crate::desktop::Shortcut`).
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// Usable as a system-wide hotkey: a supported key plus at least one of
    /// Ctrl / Alt (⌥) / ⌘ (Win).
    pub fn is_valid(&self) -> bool {
        let m = self.modifiers;
        (m.control || m.alt || m.meta) && platform_key_supported(self.key)
    }

    /// From a key press in a shortcut recorder; `None` for modifier-only or
    /// unsupported keys.
    pub fn from_key_event(event: KeyEvent) -> Option<Shortcut> {
        let mut modifiers = event.modifiers;
        modifiers.caps_lock = false;
        let shortcut = Shortcut {
            key: normalized(event.key),
            modifiers,
        };
        if shortcut.is_valid() {
            Some(shortcut)
        } else {
            None
        }
    }

    /// Compares the normalised key and the four
    /// modifiers (`caps_lock` is ignored).
    pub fn eql(&self, other: &Shortcut) -> bool {
        normalized(self.key) == normalized(other.key)
            && self.modifiers.shift == other.modifiers.shift
            && self.modifiers.control == other.modifiers.control
            && self.modifiers.alt == other.modifiers.alt
            && self.modifiers.meta == other.modifiers.meta
    }

    /// Display label for this platform: "⌥⌘V" on macOS, "Ctrl+Alt+V" on Windows.
    pub fn label(&self) -> String {
        let m = self.modifiers;
        let mut out = String::new();
        if keys::IS_MACOS {
            if m.control {
                out.push('⌃');
            }
            if m.alt {
                out.push('⌥');
            }
            if m.shift {
                out.push('⇧');
            }
            if m.meta {
                out.push('⌘');
            }
        } else {
            if m.control {
                out.push_str("Ctrl+");
            }
            if m.alt {
                out.push_str("Alt+");
            }
            if m.shift {
                out.push_str("Shift+");
            }
            if m.meta {
                out.push_str("Win+");
            }
        }
        out.push_str(&key_name(self.key));
        out
    }
}

/// Uppercase letters become lowercase.
fn normalized(key: Key) -> Key {
    match key {
        Key::Character(c) if c.is_ascii_uppercase() => Key::Character(c.to_ascii_lowercase()),
        other => other,
    }
}

/// The display name of a key.
fn key_name(key: Key) -> String {
    match key {
        Key::Character(' ') => "Space".to_owned(),
        Key::Character(c) => c.to_ascii_uppercase().to_string(),
        Key::Function(n) => format!("F{n}"),
        Key::Enter => if keys::IS_MACOS { "↵" } else { "Enter" }.to_owned(),
        Key::Tab => if keys::IS_MACOS { "⇥" } else { "Tab" }.to_owned(),
        Key::Escape => "Esc".to_owned(),
        Key::Left => "←".to_owned(),
        Key::Right => "→".to_owned(),
        Key::Up => "↑".to_owned(),
        Key::Down => "↓".to_owned(),
        // The remaining keys use their lowercase snake_case names.
        Key::Home => "home".to_owned(),
        Key::End => "end".to_owned(),
        Key::Backspace => "backspace".to_owned(),
        Key::Delete => "delete".to_owned(),
        Key::PageUp => "page_up".to_owned(),
        Key::PageDown => "page_down".to_owned(),
    }
}

/// Whether the native hotkey API supports the key.
fn platform_key_supported(key: Key) -> bool {
    match key {
        Key::Character(c) => {
            c.is_ascii_lowercase()
                || c.is_ascii_uppercase()
                || c.is_ascii_digit()
                || c == ' '
                || matches!(
                    c,
                    '`' | '-' | '=' | '[' | ']' | ';' | '\'' | ',' | '.' | '/' | '\\'
                )
        }
        Key::Function(n) => (1..=12).contains(&n),
        Key::Enter | Key::Tab | Key::Escape | Key::Left | Key::Right | Key::Up | Key::Down => true,
        Key::Home | Key::End | Key::Backspace | Key::Delete | Key::PageUp | Key::PageDown => false,
    }
}

/// Receives the recorded [`Shortcut`].
#[derive(Clone)]
pub struct HotkeyAction(Rc<HotkeyActionFn>);

type HotkeyActionFn = dyn Fn(&mut ActionCx<'_>, Shortcut);

impl HotkeyAction {
    pub fn new(callback: impl Fn(&mut ActionCx<'_>, Shortcut) + 'static) -> HotkeyAction {
        HotkeyAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, shortcut: Shortcut) {
        (self.0)(cx, shortcut);
    }
}

/// `ui.hotkey_action(state, State.set_hotkey)`.
pub fn hotkey_action<S: 'static>(callback: impl Fn(&mut S, Shortcut) + 'static) -> HotkeyAction {
    HotkeyAction::new(move |cx, shortcut| callback(cx.state::<S>(), shortcut))
}

/// Options of [`hotkey_input`] (`HotkeyInputOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct HotkeyInputOptions {
    pub key: Str,
    pub value: Option<Shortcut>,
    pub recording: bool,
    pub width: Option<f32>,
    pub height: f32,
    pub placeholder: Str,
    pub recording_text: Str,
    /// Clicked (or Enter / Space while focused): start recording.
    pub on_start: Option<Action>,
    pub on_record: Option<HotkeyAction>,
    /// Esc while recording.
    pub on_cancel: Option<Action>,
    /// The clear button, or Backspace / Delete while recording.
    pub on_clear: Option<Action>,
}

impl Default for HotkeyInputOptions {
    fn default() -> Self {
        HotkeyInputOptions {
            key: Str::Static("hotkey-input"),
            value: None,
            recording: false,
            width: None,
            height: 32.0,
            placeholder: Str::Static("点击设置快捷键"),
            recording_text: Str::Static("按下组合键…"),
            on_start: None,
            on_record: None,
            on_cancel: None,
            on_clear: None,
        }
    }
}

/// One small key-cap pill.
fn key_cap(label: &str, theme: &crate::theme::tokens::Theme) -> Node {
    row(BoxOptions {
        padding: inset_xy(6.0, 1.0),
        radius: 4.0,
        background: Some(theme.color_fill_tertiary),
        border_color: Some(theme.color_border_secondary),
        border_width: 1.0,
        children: vec![text(
            Str::copy(label),
            TextOptions {
                size: 12.0,
                weight: Weight::Medium,
                color: Some(theme.color_text),
                ..Default::default()
            },
        )],
        ..Default::default()
    })
}

/// The shortcut recorder field.
pub fn hotkey_input(o: HotkeyInputOptions) -> Node {
    let theme = current_theme();
    let mut children: Vec<Node> = Vec::new();
    if o.recording {
        children.push(text(
            o.recording_text.clone(),
            TextOptions {
                size: 13.0,
                color: Some(theme.color_primary),
                ..Default::default()
            },
        ));
    } else if let Some(shortcut) = o.value {
        // one cap per key: "⌃⌘V" -> ⌃ ⌘ V, "Ctrl+Alt+V" -> Ctrl Alt V
        let label = shortcut.label();
        let mut cap_nodes: Vec<Node> = Vec::new();
        if keys::IS_MACOS {
            for codepoint in label.chars() {
                let cap = codepoint.to_string();
                cap_nodes.push(key_cap(&cap, &theme));
            }
        } else {
            for part in label.split('+') {
                cap_nodes.push(key_cap(part, &theme));
            }
        }
        children.push(row(BoxOptions {
            gap: 4.0,
            align_items: flex::Align::Center,
            children: cap_nodes,
            ..Default::default()
        }));
    } else {
        children.push(text(
            o.placeholder.clone(),
            TextOptions {
                size: 13.0,
                color: Some(theme.color_text_disabled),
                ..Default::default()
            },
        ));
    }
    children.push(row(BoxOptions {
        grow: 1.0,
        ..Default::default()
    }));
    if o.value.is_some()
        && !o.recording
        && let Some(clear) = &o.on_clear
    {
        children.push(icon_button(IconButtonOptions {
            key: Some(Str::from(format!("{}-clear", o.key.as_str()))),
            icon: Icon::Close,
            size: 14.0,
            r#box: 22.0,
            // antd's clear icon: quiet at rest, darker on hover, no fill
            color: Some(theme.color_text_disabled),
            hover_color: Some(theme.color_text_secondary),
            tooltip: Some(Str::Static("清除")),
            on_click: Some(clear.clone()),
            ..Default::default()
        }));
    }

    // The pointer / key closures each capture their own clones of the callbacks.
    let focus_key = o.key.clone();
    let pointer_on_start = o.on_start.clone();
    let recording = o.recording;
    let on_pointer = PointerAction::new(move |cx, info| {
        if info.phase != PointerPhase::Up || info.button != events::PointerButton::Left {
            return false;
        }
        if info.x < 0.0 || info.y < 0.0 || info.x > info.width || info.y > info.height {
            return false;
        }
        if recording {
            return false;
        }
        crate::ui::focus(&focus_key);
        if let Some(action) = &pointer_on_start {
            action.call(cx);
        }
        true
    });
    let key_on_start = o.on_start.clone();
    let key_on_cancel = o.on_cancel.clone();
    let key_on_clear = o.on_clear.clone();
    let key_on_record = o.on_record.clone();
    let on_key = KeyAction::new(move |cx, key_event| {
        if !recording {
            let start = match key_event.key {
                Key::Enter => true,
                Key::Character(c) => c == ' ',
                _ => false,
            };
            if start && let Some(action) = &key_on_start {
                action.call(cx);
            }
            return start;
        }
        let plain = !(key_event.modifiers.control
            || key_event.modifiers.alt
            || key_event.modifiers.meta
            || key_event.modifiers.shift);
        match key_event.key {
            Key::Escape => {
                if plain {
                    if let Some(action) = &key_on_cancel {
                        action.call(cx);
                    }
                    return true;
                }
            }
            Key::Backspace | Key::Delete => {
                if plain {
                    if let Some(action) = &key_on_clear {
                        action.call(cx);
                    }
                    return true;
                }
            }
            _ => {}
        }
        if let Some(shortcut) = Shortcut::from_key_event(key_event)
            && let Some(action) = &key_on_record
        {
            action.call(cx, shortcut);
        }
        // while recording every key belongs to the recorder
        true
    });

    pointer_area(PointerAreaOptions {
        r#box: BoxOptions {
            key: Some(o.key),
            width: o.width,
            height: Some(o.height),
            padding: Edges {
                left: 8.0,
                right: 4.0,
                ..Edges::default()
            },
            radius: 6.0,
            align_items: flex::Align::Center,
            flex_direction: Some(flex::Direction::Row),
            background: Some(if o.recording {
                theme.color_primary_bg
            } else {
                theme.color_bg_container
            }),
            border_color: Some(if o.recording {
                theme.color_primary
            } else {
                theme.color_border
            }),
            border_width: 1.0,
            hover_border_color: if o.recording {
                None
            } else {
                Some(theme.color_primary_hover)
            },
            children,
            ..Default::default()
        },
        cursor: Some(Cursor::PointingHand),
        focusable: true,
        on_pointer,
        on_key: Some(on_key),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            Shortcut::from_key_event(key_event(Key::Character('v'), Modifiers::default()))
                .is_none()
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
        assert_eq!(defaults.placeholder, "点击设置快捷键");
        assert_eq!(defaults.recording_text, "按下组合键…");
        assert!(defaults.on_start.is_none());
        assert!(defaults.on_record.is_none());
        assert!(defaults.on_cancel.is_none());
        assert!(defaults.on_clear.is_none());
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
        assert_eq!(label.content, "点击设置快捷键");
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
        assert_eq!(hint.content, "按下组合键…");
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
        assert_eq!(tooltip, Some("清除"));
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
}
