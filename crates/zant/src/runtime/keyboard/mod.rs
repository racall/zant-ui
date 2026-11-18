//! Keyboard and text input.
//!
//! Keys go to: the app's `on_key` hook (in `dispatch`), a focused pointer
//! area's `on_key`, the selected text (copy / clear / select all), a focused
//! input's `on_key` hook, Tab / Escape handling, the focused button / toggle /
//! select, and finally the focused input's editing keys (including the macOS
//! and Windows navigation differences).
#![forbid(unsafe_code)]

use super::{Runtime, action_cx, ime};

mod dispatch;
mod editing;
mod text;

#[cfg(test)]
mod keyboard_tests;

/// A control character that must not be typed.
pub(crate) fn has_control_chars(text: &str) -> bool {
    text.bytes().any(|byte| {
        (byte < 0x20 && byte != b'\t' && byte != b'\n' && byte != b'\r') || byte == 0x7f
    })
}

#[cfg(test)]
mod tests {
    use super::has_control_chars;

    #[test]
    fn control_characters_are_detected() {
        assert!(!has_control_chars("hello"));
        assert!(!has_control_chars("a\tb\nc\rd"));
        assert!(!has_control_chars("héllo 中文"));
        assert!(has_control_chars("\u{1}"));
        assert!(has_control_chars("a\u{7f}b"));
        assert!(has_control_chars("\u{0}"));
    }
}
