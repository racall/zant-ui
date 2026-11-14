//! System-wide hotkeys: the [`Shortcut`] value with its display / settings
//! text forms and the registration calls (port of `src/desktop.zig`'s
//! "hotkeys" section).
#![forbid(unsafe_code)]

use crate::core::events::{Key, KeyEvent, Modifiers};
use crate::platform::platform;

/// A key combination, e.g. ⌥⌘V / Ctrl+Alt+V.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Shortcut {
    /// A shortcut from its parts (`Shortcut.parse` / the recorder build one).
    pub fn new(key: Key, modifiers: Modifiers) -> Shortcut {
        Shortcut { key, modifiers }
    }

    /// Usable as a system-wide hotkey: a supported key plus at least one of
    /// Ctrl / Alt (⌥) / ⌘ (Win).
    pub fn is_valid(&self) -> bool {
        let modifiers = self.modifiers;
        (modifiers.control || modifiers.alt || modifiers.meta) && platform_key(self.key).is_some()
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
        shortcut.is_valid().then_some(shortcut)
    }

    /// True when both name the same combination (the keys compare with Shift /
    /// Ctrl / Alt / Cmd and ignore Caps Lock, like Zig `Shortcut.eql`).
    pub fn eql(&self, other: &Shortcut) -> bool {
        normalized(self.key) == normalized(other.key)
            && self.modifiers.shift == other.modifiers.shift
            && self.modifiers.control == other.modifiers.control
            && self.modifiers.alt == other.modifiers.alt
            && self.modifiers.meta == other.modifiers.meta
    }

    /// Display label for this platform: "⌥⌘V" on macOS, "Ctrl+Alt+V" on
    /// Windows (Zig `Shortcut.label` writes into a caller buffer).
    pub fn label(&self) -> String {
        let mut out = String::new();
        let modifiers = self.modifiers;
        if platform::IS_MACOS {
            if modifiers.control {
                out.push('⌃');
            }
            if modifiers.alt {
                out.push('⌥');
            }
            if modifiers.shift {
                out.push('⇧');
            }
            if modifiers.meta {
                out.push('⌘');
            }
        } else {
            if modifiers.control {
                out.push_str("Ctrl+");
            }
            if modifiers.alt {
                out.push_str("Alt+");
            }
            if modifiers.shift {
                out.push_str("Shift+");
            }
            if modifiers.meta {
                out.push_str("Win+");
            }
        }
        write_key_name(&mut out, self.key);
        out
    }

    /// Text form for settings files: "ctrl+alt+shift+meta+v" (any subset).
    pub fn format(&self) -> String {
        let mut out = String::new();
        let modifiers = self.modifiers;
        if modifiers.control {
            out.push_str("ctrl+");
        }
        if modifiers.alt {
            out.push_str("alt+");
        }
        if modifiers.shift {
            out.push_str("shift+");
        }
        if modifiers.meta {
            out.push_str("meta+");
        }
        match self.key {
            Key::Character(' ') => out.push_str("space"),
            Key::Character(character) => out.push(character),
            Key::Function(number) => out.push_str(&format!("f{number}")),
            other => out.push_str(key_tag_name(other)),
        }
        out
    }

    /// Inverse of [`Shortcut::format`]; `None` when malformed.
    pub fn parse(text: &str) -> Option<Shortcut> {
        let mut shortcut = Shortcut {
            key: Key::Character('\0'),
            modifiers: Modifiers::default(),
        };
        let mut key_part: Option<&str> = None;
        for part in text.split('+') {
            if key_part.is_some() {
                return None; // modifiers come first
            }
            match part {
                "ctrl" => shortcut.modifiers.control = true,
                "alt" => shortcut.modifiers.alt = true,
                "shift" => shortcut.modifiers.shift = true,
                "meta" => shortcut.modifiers.meta = true,
                other => key_part = Some(other),
            }
        }
        let key_part = key_part?;
        if key_part == "space" {
            shortcut.key = Key::Character(' ');
        } else if key_part.len() >= 2
            && key_part.starts_with('f')
            && key_part.as_bytes()[1].is_ascii_digit()
        {
            let number: u8 = key_part[1..].parse().ok()?;
            if !(1..=12).contains(&number) {
                return None;
            }
            shortcut.key = Key::Function(number);
        } else if key_part.chars().count() == 1 {
            shortcut.key = Key::Character(key_part.chars().next()?);
        } else {
            shortcut.key = match key_part {
                "left" => Key::Left,
                "right" => Key::Right,
                "up" => Key::Up,
                "down" => Key::Down,
                "enter" => Key::Enter,
                "tab" => Key::Tab,
                "escape" => Key::Escape,
                // unknown names keep the default character key and fail
                // `is_valid`, like Zig's inline for
                _ => Key::Character('\0'),
            };
        }
        shortcut.is_valid().then_some(shortcut)
    }
}

/// Zig `normalized`: character keys compare case-insensitively.
fn normalized(key: Key) -> Key {
    match key {
        Key::Character(character) if character.is_ascii_uppercase() => {
            Key::Character(character.to_ascii_lowercase())
        }
        other => other,
    }
}

/// Zig `@tagName` of the non-character keys, as used by `Shortcut.format`.
fn key_tag_name(key: Key) -> &'static str {
    match key {
        Key::Character(_) => "character",
        Key::Left => "left",
        Key::Right => "right",
        Key::Up => "up",
        Key::Down => "down",
        Key::Home => "home",
        Key::End => "end",
        Key::Backspace => "backspace",
        Key::Delete => "delete",
        Key::Escape => "escape",
        Key::Tab => "tab",
        Key::Enter => "enter",
        Key::PageUp => "page_up",
        Key::PageDown => "page_down",
        Key::Function(_) => "function",
    }
}

/// Writes the display name of `key` (Zig `writeKeyName`).
fn write_key_name(out: &mut String, key: Key) {
    match key {
        Key::Character(character) => {
            if character == ' ' {
                out.push_str("Space");
            } else {
                out.push(character.to_ascii_uppercase());
            }
        }
        Key::Function(number) => out.push_str(&format!("F{number}")),
        Key::Enter => out.push_str(if platform::IS_MACOS { "↵" } else { "Enter" }),
        Key::Tab => out.push_str(if platform::IS_MACOS { "⇥" } else { "Tab" }),
        Key::Escape => out.push_str("Esc"),
        Key::Left => out.push('←'),
        Key::Right => out.push('→'),
        Key::Up => out.push('↑'),
        Key::Down => out.push('↓'),
        other => out.push_str(key_tag_name(other)),
    }
}

/// Zig `platformKey`: the platform key code for a supported key.
fn platform_key(key: Key) -> Option<i32> {
    use crate::platform::platform::key as platform_key_code;
    Some(match key {
        Key::Character(character) => match character {
            'a'..='z' | '0'..='9' => character as i32,
            'A'..='Z' => (character as i32) + 32,
            ' ' => platform_key_code::SPACE,
            '`' | '-' | '=' | '[' | ']' | ';' | '\'' | ',' | '.' | '/' | '\\' => character as i32,
            _ => return None,
        },
        Key::Function(number) if (1..=12).contains(&number) => {
            platform_key_code::F1 + i32::from(number) - 1
        }
        Key::Function(_) => return None,
        Key::Enter => platform_key_code::ENTER,
        Key::Tab => platform_key_code::TAB,
        Key::Escape => platform_key_code::ESCAPE,
        Key::Left => platform_key_code::LEFT,
        Key::Right => platform_key_code::RIGHT,
        Key::Up => platform_key_code::UP,
        Key::Down => platform_key_code::DOWN,
        _ => return None,
    })
}

/// Zig `platformModifiers`: the `ZANT_MOD_*` bits of a combination.
fn platform_modifiers(modifiers: Modifiers) -> i32 {
    let mut out = 0;
    if modifiers.shift {
        out |= platform::modifiers::SHIFT;
    }
    if modifiers.control {
        out |= platform::modifiers::CONTROL;
    }
    if modifiers.alt {
        out |= platform::modifiers::ALT;
    }
    if modifiers.meta {
        out |= platform::modifiers::META;
    }
    out
}

/// Registers a system-wide hotkey (works while other apps are in front);
/// presses arrive as [`crate::desktop::AppEvent::Hotkey`]. Re-registering an id
/// replaces it. False when another app already uses the combination or it is
/// invalid.
pub fn register_hotkey(id: u32, shortcut: Shortcut) -> bool {
    let Some(key) = platform_key(shortcut.key) else {
        return false;
    };
    if !shortcut.is_valid() {
        return false;
    }
    let id = i32::try_from(id).expect("hotkey id fits in a C int");
    platform::register_hotkey(id, key, platform_modifiers(shortcut.modifiers))
}

/// Removes the hotkey registered with `id` (Zig `unregisterHotkey`).
pub fn unregister_hotkey(id: u32) {
    let id = i32::try_from(id).expect("hotkey id fits in a C int");
    platform::unregister_hotkey(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_text_form_and_labels() {
        // Zig `test "shortcut text form and labels"`
        let shortcut = Shortcut::parse("ctrl+alt+v").expect("parsed");
        assert!(shortcut.modifiers.control && shortcut.modifiers.alt && !shortcut.modifiers.shift);
        assert_eq!(shortcut.format(), "ctrl+alt+v");
        let expected = if platform::IS_MACOS {
            "⌃⌥V"
        } else {
            "Ctrl+Alt+V"
        };
        assert_eq!(shortcut.label(), expected);
        let function = Shortcut::parse("meta+shift+f5").expect("parsed");
        assert_eq!(function.format(), "shift+meta+f5");
        assert_eq!(Shortcut::parse("v"), None, "needs ctrl / alt / meta");
        assert_eq!(Shortcut::parse("shift+v"), None);
        assert_eq!(Shortcut::parse("ctrl+"), None);
        let recorded = Shortcut::from_key_event(KeyEvent {
            key: Key::Character('V'),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            repeat: false,
        })
        .expect("recorded");
        assert!(recorded.eql(&Shortcut::new(
            Key::Character('v'),
            Modifiers {
                meta: true,
                ..Default::default()
            }
        )));
    }

    #[test]
    fn unsupported_keys_are_not_valid_shortcuts() {
        let modifiers = Modifiers {
            control: true,
            ..Default::default()
        };
        assert!(!Shortcut::new(Key::Home, modifiers).is_valid());
        assert!(!Shortcut::new(Key::PageUp, modifiers).is_valid());
        assert!(!Shortcut::new(Key::Function(13), modifiers).is_valid());
        assert!(!Shortcut::new(Key::Character('é'), modifiers).is_valid());
        assert!(Shortcut::new(Key::Function(12), modifiers).is_valid());
        assert!(Shortcut::new(Key::Character('-'), modifiers).is_valid());
        assert_eq!(
            Shortcut::from_key_event(KeyEvent {
                key: Key::Escape,
                modifiers: Modifiers {
                    control: true,
                    ..Default::default()
                },
                repeat: false,
            })
            .map(|shortcut| shortcut.format()),
            Some(String::from("ctrl+escape"))
        );
    }

    #[test]
    fn space_and_settings_text_round_trip() {
        let shortcut = Shortcut::parse("ctrl+shift+space").expect("space");
        assert_eq!(shortcut.key, Key::Character(' '));
        assert_eq!(shortcut.format(), "ctrl+shift+space");
        assert_eq!(
            shortcut.label(),
            if platform::IS_MACOS {
                "⌃⇧Space"
            } else {
                "Ctrl+Shift+Space"
            }
        );
        for text in ["ctrl+enter", "alt+tab", "meta+left", "ctrl+`"] {
            let parsed = Shortcut::parse(text).expect(text);
            assert_eq!(parsed.format(), text);
        }
    }

    #[test]
    fn malformed_text_forms_are_rejected() {
        assert_eq!(Shortcut::parse(""), None);
        assert_eq!(Shortcut::parse("ctrl+alt"), None, "no key");
        assert_eq!(Shortcut::parse("f0"), None);
        assert_eq!(Shortcut::parse("f13"), None);
        assert_eq!(
            Shortcut::parse("ctrl+f"),
            Some(Shortcut::new(
                Key::Character('f'),
                Modifiers {
                    control: true,
                    ..Default::default()
                }
            ))
        );
        assert_eq!(Shortcut::parse("home"), None, "unsupported named key");
    }

    #[test]
    fn registering_an_invalid_shortcut_fails_without_touching_the_system() {
        assert!(!register_hotkey(
            1,
            Shortcut::new(Key::Character('v'), Modifiers::default())
        ));
        unregister_hotkey(1);
    }
}
