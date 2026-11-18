//! `HotkeyInputOptions` and the `HotkeyAction` callback a recorded shortcut is sent to.

use super::*;

/// Receives the recorded [`Shortcut`].
#[derive(Clone)]
pub struct HotkeyAction(Rc<HotkeyActionFn>);

pub(super) type HotkeyActionFn = dyn Fn(&mut ActionCx<'_>, Shortcut);

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
            placeholder: builtin::HOTKEY_PLACEHOLDER.current(),
            recording_text: builtin::HOTKEY_RECORDING.current(),
            on_start: None,
            on_record: None,
            on_cancel: None,
            on_clear: None,
        }
    }
}
