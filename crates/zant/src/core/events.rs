//! Port of `src/core/events.zig` — WP-1.1d.
//!
//! Platform-independent input events: the runtime translates the raw
//! `platform::Event` handed to `HostHandler::event` into these values.
//!
//! Deviation from Zig: `text_input` and `composition` own a `String` because the
//! native event only lends its UTF-8 bytes for the duration of the callback
//! (Zig borrowed them into a `[]const u8`). The runtime processes the event
//! synchronously, so the allocation is short-lived.

use crate::core::keys;
use crate::platform::platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Character(char),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Backspace,
    Delete,
    Escape,
    Tab,
    Enter,
    PageUp,
    PageDown,
    /// F1 … F12 (1 … 12)
    Function(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
    pub caps_lock: bool,
}

impl Modifiers {
    /// The platform's command modifier: ⌘ on macOS, Ctrl on Windows.
    pub fn primary(&self) -> bool {
        keys::primary(*self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    pub x: f32,
    pub y: f32,
    pub button: PointerButton,
    pub modifiers: Modifiers,
}

impl Default for PointerEvent {
    fn default() -> Self {
        PointerEvent {
            x: 0.0,
            y: 0.0,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    pub text: String,
    pub selection_start: usize,
    pub selection_end: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowResize {
    pub width: f32,
    pub height: f32,
    pub dpr: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub modifiers: Modifiers,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    PointerMove(PointerEvent),
    PointerDown(PointerEvent),
    PointerUp(PointerEvent),
    PointerEnter(PointerEvent),
    PointerLeave(PointerEvent),
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
    TextInput(String),
    Composition(Composition),
    WindowResize(WindowResize),
    WindowFocus,
    WindowBlur,
    WindowClose,
    /// The user asked to close the window; the app may veto it.
    WindowCloseRequest,
    /// The user asked to quit the app (⌘Q, the Dock menu, logout); sent to the
    /// main window, the app may veto it.
    AppQuitRequest,
    /// The system switched between light and dark appearance.
    AppearanceChange,
    Scroll(Scroll),
}

pub fn modifiers_from_platform(mods: i32) -> Modifiers {
    Modifiers {
        shift: (mods & platform::modifiers::SHIFT) != 0,
        control: (mods & platform::modifiers::CONTROL) != 0,
        alt: (mods & platform::modifiers::ALT) != 0,
        meta: (mods & platform::modifiers::META) != 0,
        caps_lock: (mods & platform::modifiers::CAPS_LOCK) != 0,
    }
}

/// Maps a `ZANT_KEY_*` / character code to a [`Key`] (Zig: `keyFromPlatform`).
pub fn key_from_platform(code: i32) -> Key {
    key_from_code(code)
}

/// Translates a raw `ZantEvent` into an [`Event`].
pub fn translate(raw: &platform::Event) -> Event {
    let mods = modifiers_from_platform(raw.modifiers);
    let pointer = PointerEvent {
        x: raw.x,
        y: raw.y,
        button: match raw.button {
            platform::pointer_button::RIGHT => PointerButton::Right,
            platform::pointer_button::MIDDLE => PointerButton::Middle,
            _ => PointerButton::Left,
        },
        modifiers: mods,
    };
    match raw.type_ {
        platform::event_type::POINTER_MOVE => Event::PointerMove(pointer),
        platform::event_type::POINTER_DOWN => Event::PointerDown(pointer),
        platform::event_type::POINTER_UP => Event::PointerUp(pointer),
        platform::event_type::POINTER_ENTER => Event::PointerEnter(pointer),
        platform::event_type::POINTER_LEAVE => Event::PointerLeave(pointer),
        platform::event_type::KEY_DOWN => Event::KeyDown(KeyEvent {
            key: key_from_code(raw.key),
            modifiers: mods,
            repeat: raw.is_repeat != 0,
        }),
        platform::event_type::KEY_UP => Event::KeyUp(KeyEvent {
            key: key_from_code(raw.key),
            modifiers: mods,
            repeat: false,
        }),
        platform::event_type::TEXT_INPUT => Event::TextInput(platform::event_text(raw).to_owned()),
        platform::event_type::COMPOSITION => Event::Composition(Composition {
            text: platform::event_text(raw).to_owned(),
            selection_start: raw.selection_start,
            selection_end: raw.selection_end,
        }),
        platform::event_type::WINDOW_RESIZE => Event::WindowResize(WindowResize {
            width: raw.width as f32,
            height: raw.height as f32,
            dpr: raw.dpr,
        }),
        platform::event_type::WINDOW_FOCUS => Event::WindowFocus,
        platform::event_type::WINDOW_BLUR => Event::WindowBlur,
        platform::event_type::WINDOW_CLOSE => Event::WindowClose,
        platform::event_type::APPEARANCE_CHANGE => Event::AppearanceChange,
        platform::event_type::WINDOW_CLOSE_REQUEST => {
            if raw.button == 1 {
                Event::AppQuitRequest
            } else {
                Event::WindowCloseRequest
            }
        }
        platform::event_type::SCROLL => Event::Scroll(Scroll {
            x: raw.x,
            y: raw.y,
            dx: raw.scroll_x,
            dy: raw.scroll_y,
            modifiers: mods,
        }),
        _ => Event::WindowFocus,
    }
}

/// Maps a `ZANT_KEY_*` / character code to a [`Key`].
pub fn key_from_code(code: i32) -> Key {
    use platform::key as pk;
    match code {
        pk::LEFT => Key::Left,
        pk::RIGHT => Key::Right,
        pk::UP => Key::Up,
        pk::DOWN => Key::Down,
        pk::HOME => Key::Home,
        pk::END => Key::End,
        pk::BACKSPACE => Key::Backspace,
        pk::DELETE => Key::Delete,
        pk::ESCAPE => Key::Escape,
        pk::TAB => Key::Tab,
        pk::ENTER => Key::Enter,
        pk::PAGE_UP => Key::PageUp,
        pk::PAGE_DOWN => Key::PageDown,
        f1 if (pk::F1..=pk::F12).contains(&f1) => {
            Key::Function(u8::try_from(f1 - pk::F1 + 1).unwrap_or(1))
        }
        other => Key::Character(if other >= 0 {
            char::from_u32(u32::try_from(other).unwrap_or(0)).unwrap_or('\0')
        } else {
            '\0'
        }),
    }
}

pub fn character_equals(key: Key, c: u8) -> bool {
    match key {
        Key::Character(cp) => cp == char::from(c),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(type_: i32) -> platform::Event {
        platform::Event {
            type_,
            x: 0.0,
            y: 0.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            button: 0,
            key: 0,
            modifiers: 0,
            is_repeat: 0,
            text: std::ptr::null(),
            text_len: 0,
            selection_start: 0,
            selection_end: 0,
            width: 0,
            height: 0,
            dpr: 1.0,
        }
    }

    #[test]
    fn modifiers_map_each_platform_bit() {
        let mods = modifiers_from_platform(
            platform::modifiers::SHIFT
                | platform::modifiers::CONTROL
                | platform::modifiers::CAPS_LOCK,
        );
        assert!(mods.shift);
        assert!(mods.control);
        assert!(!mods.alt);
        assert!(!mods.meta);
        assert!(mods.caps_lock);
    }

    #[test]
    fn keys_map_named_codes_and_characters() {
        assert_eq!(key_from_code(platform::key::LEFT), Key::Left);
        assert_eq!(key_from_code(platform::key::PAGE_DOWN), Key::PageDown);
        assert_eq!(key_from_code(platform::key::F1), Key::Function(1));
        assert_eq!(key_from_code(platform::key::F12), Key::Function(12));
        assert_eq!(key_from_code(i32::from(b'a')), Key::Character('a'));
        assert_eq!(key_from_code(-1), Key::Character('\0'));
        assert!(character_equals(Key::Character('a'), b'a'));
        assert!(!character_equals(Key::Left, b'a'));
    }

    #[test]
    fn pointer_and_scroll_events_keep_their_coordinates() {
        let mut event = raw(platform::event_type::SCROLL);
        event.x = 10.0;
        event.y = 20.0;
        event.scroll_x = 1.5;
        event.scroll_y = -3.0;
        event.modifiers = platform::modifiers::ALT;
        match translate(&event) {
            Event::Scroll(scroll) => {
                assert_eq!(scroll.x, 10.0);
                assert_eq!(scroll.y, 20.0);
                assert_eq!(scroll.dx, 1.5);
                assert_eq!(scroll.dy, -3.0);
                assert!(scroll.modifiers.alt);
            }
            other => panic!("expected a scroll event, got {other:?}"),
        }
    }

    #[test]
    fn close_request_is_a_quit_when_button_is_set() {
        let mut event = raw(platform::event_type::WINDOW_CLOSE_REQUEST);
        assert_eq!(translate(&event), Event::WindowCloseRequest);
        event.button = 1;
        assert_eq!(translate(&event), Event::AppQuitRequest);
    }

    #[test]
    fn primary_modifier_follows_the_platform() {
        let meta = Modifiers {
            meta: true,
            ..Default::default()
        };
        let control = Modifiers {
            control: true,
            ..Default::default()
        };
        if keys::IS_MACOS {
            assert!(meta.primary());
            assert!(!control.primary());
        } else {
            assert!(control.primary());
            assert!(!meta.primary());
        }
        assert!(!Modifiers::default().primary());
    }
}
