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
//! The value is [`Shortcut`] (`desktop::Shortcut`): its `from_key_event`
//! reads the recorded press and `key_caps` gives the caps shown.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::core::events::{self, Key};
use crate::core::geometry::Edges;
use crate::core::node::Icon;
use crate::flex;
use crate::i18n::builtin;
use crate::platform::platform::Cursor;
use crate::renderer::text::Weight;
use crate::ui::actions::{Action, ActionCx, KeyAction, PointerAction, PointerPhase};
use crate::ui::builders::{icon_button, pointer_area, row, text};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, IconButtonOptions, PointerAreaOptions, TextOptions};
use crate::ui::styled::inset_xy;

/// A key combination, e.g. ⌥⌘V / Ctrl+Alt+V: the one shortcut type of the
/// framework (`desktop::Shortcut`, which also registers system-wide hotkeys).
pub use crate::desktop::Shortcut;

mod options;
#[cfg(test)]
mod tests;

pub use self::options::*;

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
        // one cap per key: ⌃ ⌘ V / Ctrl Alt V
        let cap_nodes: Vec<Node> = shortcut
            .key_caps()
            .iter()
            .map(|cap| key_cap(cap, &theme))
            .collect();
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
            tooltip: Some(builtin::HOTKEY_CLEAR.current()),
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
