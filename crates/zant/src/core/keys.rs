//! Port of `src/core/keys.zig` — WP-1.1d.
//!
//! Platform-aware keyboard shortcuts. Apps describe a shortcut once and get the
//! platform's modifier and label:
//!
//! ```ignore
//! if keys::primary(event.modifiers) && ... { }   // ⌘ on macOS, Ctrl on Windows
//! node.options.keys = keys::label("mod+shift+s"); // "⇧⌘S" / "Ctrl+Shift+S"
//! let chord = keys::pick("alt+mod+f", "mod+h");   // macOS vs Windows chord
//! ```
//!
//! Spec syntax: chords separated by spaces, keys in a chord joined by `+`.
//! Modifiers: `mod` (the primary modifier), `ctrl`, `alt`, `shift`. Keys:
//! letters, digits, punctuation, `enter`, `esc`, `tab`, `space`, `backspace`,
//! `delete`, `home`, `end`, `left`, `right`, `up`, `down`, `f1`..`f12`.
//!
//! Zig computes labels at comptime; the Rust port computes the same strings at
//! runtime (there is no comptime string concatenation), with the same
//! per-platform tables and order.

use crate::core::events::Modifiers;

pub const IS_MACOS: bool = cfg!(target_os = "macos");

/// The platform's command modifier is held: ⌘ on macOS, Ctrl elsewhere.
pub fn primary(m: Modifiers) -> bool {
    if IS_MACOS { m.meta } else { m.control }
}

/// `mac` on macOS, `other` elsewhere (Zig: `pick`).
pub fn pick<T>(mac: T, other: T) -> T {
    if IS_MACOS { mac } else { other }
}

/// Display label of a shortcut spec for the current platform.
pub fn label(spec: &str) -> String {
    label_for(spec, IS_MACOS)
}

fn key_name(key: &str, mac: bool) -> String {
    match key {
        "enter" => return if mac { "↵" } else { "Enter" }.to_owned(),
        "esc" => return "Esc".to_owned(),
        "tab" => return if mac { "⇥" } else { "Tab" }.to_owned(),
        "space" => return "Space".to_owned(),
        "backspace" => return if mac { "⌫" } else { "Backspace" }.to_owned(),
        "delete" => return if mac { "⌦" } else { "Delete" }.to_owned(),
        "home" => return if mac { "↖" } else { "Home" }.to_owned(),
        "end" => return if mac { "↘" } else { "End" }.to_owned(),
        "left" => return "←".to_owned(),
        "right" => return "→".to_owned(),
        "up" => return "↑".to_owned(),
        "down" => return "↓".to_owned(),
        _ => {}
    }
    let bytes = key.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_lowercase() {
        return ((bytes[0] - 32) as char).to_string();
    }
    if bytes.len() >= 2 && bytes[0] == b'f' && bytes[1].is_ascii_digit() {
        return format!("F{}", &key[1..]);
    }
    key.to_owned()
}

fn chord_label(chord: &str, mac: bool) -> String {
    let mut has_mod = false;
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut key = "";
    for part in chord.split('+') {
        // "mod++" (the plus key): an empty part after a '+' is the key
        if part.is_empty() {
            key = "+";
        } else if part == "mod" {
            has_mod = true;
        } else if part == "ctrl" {
            has_ctrl = true;
        } else if part == "alt" {
            has_alt = true;
        } else if part == "shift" {
            has_shift = true;
        } else {
            key = part;
        }
    }
    let name = key_name(key, mac);
    if mac {
        // Apple order: ⌃ ⌥ ⇧ ⌘, no separators
        let mut out = String::new();
        if has_ctrl {
            out.push('⌃');
        }
        if has_alt {
            out.push('⌥');
        }
        if has_shift {
            out.push('⇧');
        }
        if has_mod {
            out.push('⌘');
        }
        out.push_str(&name);
        out
    } else {
        // Windows order: Ctrl+Alt+Shift+Key
        let mut out = String::new();
        if has_mod || has_ctrl {
            out.push_str("Ctrl+");
        }
        if has_alt {
            out.push_str("Alt+");
        }
        if has_shift {
            out.push_str("Shift+");
        }
        out.push_str(&name);
        out
    }
}

fn label_for(spec: &str, mac: bool) -> String {
    let mut out = String::new();
    for chord in spec.split(' ').filter(|part| !part.is_empty()) {
        if !out.is_empty() {
            out.push_str("  ");
        }
        out.push_str(&chord_label(chord, mac));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_labels_follow_each_platforms_convention() {
        assert_eq!(label_for("mod+shift+s", true), "⇧⌘S");
        assert_eq!(label_for("mod+shift+s", false), "Ctrl+Shift+S");
        assert_eq!(label_for("alt+mod+f", true), "⌥⌘F");
        assert_eq!(label_for("alt+mod+f", false), "Ctrl+Alt+F");
        assert_eq!(label_for("mod++ mod+- mod+0", true), "⌘+  ⌘-  ⌘0");
        assert_eq!(
            label_for("mod++ mod+- mod+0", false),
            "Ctrl++  Ctrl+-  Ctrl+0"
        );
        assert_eq!(label_for("enter shift+enter", true), "↵  ⇧↵");
        assert_eq!(label_for("enter shift+enter", false), "Enter  Shift+Enter");
        assert_eq!(label_for("mod+\\", true), "⌘\\");
        assert_eq!(label_for("esc", false), "Esc");
        // the default label for the current platform matches `label`
        assert_eq!(label("mod+shift+s"), label_for("mod+shift+s", IS_MACOS));
    }

    #[test]
    fn primary_modifier() {
        let m = if IS_MACOS {
            Modifiers {
                meta: true,
                ..Default::default()
            }
        } else {
            Modifiers {
                control: true,
                ..Default::default()
            }
        };
        assert!(primary(m));
        assert!(!primary(Modifiers {
            shift: true,
            ..Default::default()
        }));
    }

    #[test]
    fn pick_returns_the_platform_value() {
        assert_eq!(pick("mac", "other"), if IS_MACOS { "mac" } else { "other" });
    }
}
