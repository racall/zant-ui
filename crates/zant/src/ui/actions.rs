//! Declarative callbacks (`ui::action`, `ui::text_action`, …).
//!
//! The callback types stored in nodes plus the `ui::action` /
//! `ui::text_action` / `ui::bool_action` / `ui::index_action` /
//! `ui::range_action` / `ui::pointer_action` / `ui::key_action` /
//! `ui::input_key_action` adapters that build them from a state method.
//!
//! A callback is a cheap `Rc` of a closure. The runtime always **clones the
//! callback out of the node and releases its borrow before calling it**, so
//! closures may rebuild the UI or touch the tree.
#![forbid(unsafe_code)]

use std::any::Any;
use std::rc::Rc;

use crate::core::events::{KeyEvent, Modifiers, PointerButton};

/// The runtime context a callback runs in: the root application state plus,
/// later, whatever the dispatcher needs.
///
/// Tests construct it from a `&mut dyn Any` root. `pub(crate)` fields
/// (window, dispatcher, …) and extra constructors may be added without
/// changing `new` / `state` / `Action::call`.
pub struct ActionCx<'a> {
    root: &'a mut dyn Any,
}

impl<'a> ActionCx<'a> {
    /// Builds the context around the current window's root state.
    pub fn new(root: &'a mut dyn Any) -> ActionCx<'a> {
        ActionCx { root }
    }

    /// The root application state; panics when the callback expects another
    /// type (the state type is only known at run time, through `dyn Any`).
    pub fn state<S: 'static>(&mut self) -> &mut S {
        match self.root.downcast_mut::<S>() {
            Some(state) => state,
            None => panic!(
                "Zant UI: action expects the root state to be `{}`, got another type",
                std::any::type_name::<S>()
            ),
        }
    }
}

thread_local! {
    /// An app callback ran since the last build: state it changed is not
    /// rendered yet (`runtime` renders it before the next discrete event).
    static RAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Records that an app callback is running.
pub(crate) fn note_ran() {
    RAN.with(|ran| ran.set(true));
}

/// Whether an app callback ran since [`clear_ran`].
pub(crate) fn ran() -> bool {
    RAN.with(|ran| ran.get())
}

pub(crate) fn clear_ran() {
    RAN.with(|ran| ran.set(false));
}

/// `ui::action`: a callback that only needs the root state.
#[derive(Clone)]
pub struct Action(Rc<ActionFn>);

type ActionFn = dyn Fn(&mut ActionCx<'_>);

impl Action {
    /// A callback that reads the whole [`ActionCx`] (React-style closures).
    pub fn new(callback: impl Fn(&mut ActionCx<'_>) + 'static) -> Action {
        Action(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>) {
        note_ran();
        (self.0)(cx);
    }
}

/// `ui::bool_action`: toggles, checkboxes.
#[derive(Clone)]
pub struct BoolAction(Rc<BoolActionFn>);

type BoolActionFn = dyn Fn(&mut ActionCx<'_>, bool);

impl BoolAction {
    pub fn new(callback: impl Fn(&mut ActionCx<'_>, bool) + 'static) -> BoolAction {
        BoolAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, checked: bool) {
        note_ran();
        (self.0)(cx, checked);
    }
}

/// `ui::text_action`: inputs and rich-text links.
#[derive(Clone)]
pub struct TextAction(Rc<TextActionFn>);

type TextActionFn = dyn Fn(&mut ActionCx<'_>, &str);

impl TextAction {
    pub fn new(callback: impl Fn(&mut ActionCx<'_>, &str) + 'static) -> TextAction {
        TextAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, value: &str) {
        note_ran();
        (self.0)(cx, value);
    }
}

/// `ui::index_action`: one callback for a list of items.
#[derive(Clone)]
pub struct IndexAction(Rc<IndexActionFn>);

type IndexActionFn = dyn Fn(&mut ActionCx<'_>, usize);

impl IndexAction {
    pub fn new(callback: impl Fn(&mut ActionCx<'_>, usize) + 'static) -> IndexAction {
        IndexAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, index: usize) {
        note_ran();
        (self.0)(cx, index);
    }
}

/// Reports a byte range, e.g. an input selection `(anchor, cursor)`.
#[derive(Clone)]
pub struct RangeAction(Rc<RangeActionFn>);

type RangeActionFn = dyn Fn(&mut ActionCx<'_>, usize, usize);

impl RangeAction {
    pub fn new(callback: impl Fn(&mut ActionCx<'_>, usize, usize) + 'static) -> RangeAction {
        RangeAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, start: usize, end: usize) {
        note_ran();
        (self.0)(cx, start, end);
    }
}

/// What happened to a pointer area (`ui::pointer_area`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PointerPhase {
    /// Button pressed inside the area; the area captures the pointer until `up`.
    #[default]
    Down,
    /// Pointer moved while captured (dragging), coordinates may leave the area.
    Drag,
    /// Button released after `down` (also sent when the capture is lost).
    Up,
    /// Pointer moved over the area without a button pressed.
    Hover,
    /// Pointer left the area (after `hover`).
    Leave,
    /// Wheel / trackpad scroll over an area with `capture_scroll`.
    Scroll,
}

/// A pointer event delivered to a pointer area. `x` / `y` are relative to the
/// area's top-left corner (logical px); `window_x` / `window_y` are window
/// coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerInfo {
    pub phase: PointerPhase,
    pub x: f32,
    pub y: f32,
    pub window_x: f32,
    pub window_y: f32,
    /// Size of the area.
    pub width: f32,
    pub height: f32,
    pub button: PointerButton,
    pub modifiers: Modifiers,
    /// 1 for a single click, 2 for a double click, 3 for a triple click
    /// (`down` / `up` only).
    pub click_count: u8,
    /// Scroll delta (`scroll` only): positive `dy` moves content down, like
    /// AppKit's scrollingDeltaY.
    pub dx: f32,
    pub dy: f32,
}

impl Default for PointerInfo {
    fn default() -> Self {
        PointerInfo {
            phase: PointerPhase::Down,
            x: 0.0,
            y: 0.0,
            window_x: 0.0,
            window_y: 0.0,
            width: 0.0,
            height: 0.0,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
            click_count: 1,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

/// `ui::pointer_action`: return true when application state changed and the UI
/// must be rebuilt.
#[derive(Clone)]
pub struct PointerAction(Rc<PointerActionFn>);

type PointerActionFn = dyn Fn(&mut ActionCx<'_>, PointerInfo) -> bool;

impl PointerAction {
    pub fn new(
        callback: impl Fn(&mut ActionCx<'_>, PointerInfo) -> bool + 'static,
    ) -> PointerAction {
        PointerAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, info: PointerInfo) -> bool {
        note_ran();
        (self.0)(cx, info)
    }
}

/// `ui::key_action`: key handler of a focused pointer area; true consumes the key.
#[derive(Clone)]
pub struct KeyAction(Rc<KeyActionFn>);

type KeyActionFn = dyn Fn(&mut ActionCx<'_>, KeyEvent) -> bool;

impl KeyAction {
    pub fn new(callback: impl Fn(&mut ActionCx<'_>, KeyEvent) -> bool + 'static) -> KeyAction {
        KeyAction(Rc::new(callback))
    }

    pub fn call(&self, cx: &mut ActionCx<'_>, key: KeyEvent) -> bool {
        note_ran();
        (self.0)(cx, key)
    }
}

/// `ui::input_key_action`: key hook of an input. Receives the input's current
/// text and selection `(anchor, cursor)`; true consumes the key.
#[derive(Clone)]
pub struct InputKeyAction(Rc<InputKeyActionFn>);

type InputKeyActionFn = dyn Fn(&mut ActionCx<'_>, KeyEvent, &str, usize, usize) -> bool;

impl InputKeyAction {
    pub fn new(
        callback: impl Fn(&mut ActionCx<'_>, KeyEvent, &str, usize, usize) -> bool + 'static,
    ) -> InputKeyAction {
        InputKeyAction(Rc::new(callback))
    }

    pub fn call(
        &self,
        cx: &mut ActionCx<'_>,
        key: KeyEvent,
        text: &str,
        anchor: usize,
        cursor: usize,
    ) -> bool {
        note_ran();
        (self.0)(cx, key, text, anchor, cursor)
    }
}

/// `ui::action::<State>(State::save)`.
pub fn action<S: 'static>(callback: impl Fn(&mut S) + 'static) -> Action {
    Action::new(move |cx| callback(cx.state::<S>()))
}

/// `ui::bool_action::<State>(State::set_checked)`.
pub fn bool_action<S: 'static>(callback: impl Fn(&mut S, bool) + 'static) -> BoolAction {
    BoolAction::new(move |cx, checked| callback(cx.state::<S>(), checked))
}

/// `ui::text_action::<State>(State::set_text)`.
pub fn text_action<S: 'static>(callback: impl Fn(&mut S, &str) + 'static) -> TextAction {
    TextAction::new(move |cx, value| callback(cx.state::<S>(), value))
}

/// `ui::index_action::<State>(State::select)`.
pub fn index_action<S: 'static>(callback: impl Fn(&mut S, usize) + 'static) -> IndexAction {
    IndexAction::new(move |cx, index| callback(cx.state::<S>(), index))
}

/// `ui::range_action::<State>(State::set_selection)`.
pub fn range_action<S: 'static>(callback: impl Fn(&mut S, usize, usize) + 'static) -> RangeAction {
    RangeAction::new(move |cx, start, end| callback(cx.state::<S>(), start, end))
}

// Call helpers, taking the optional field by value so the tree is not
// borrowed while the callback runs.

pub fn call(callback: Option<Action>, cx: &mut ActionCx<'_>) {
    if let Some(callback) = callback {
        callback.call(cx);
    }
}

pub fn call_bool(callback: Option<BoolAction>, cx: &mut ActionCx<'_>, checked: bool) {
    if let Some(callback) = callback {
        callback.call(cx, checked);
    }
}

pub fn call_text(callback: Option<TextAction>, cx: &mut ActionCx<'_>, value: &str) {
    if let Some(callback) = callback {
        callback.call(cx, value);
    }
}

pub fn call_index(callback: Option<IndexAction>, cx: &mut ActionCx<'_>, index: usize) {
    if let Some(callback) = callback {
        callback.call(cx, index);
    }
}

pub fn call_range(callback: Option<RangeAction>, cx: &mut ActionCx<'_>, start: usize, end: usize) {
    if let Some(callback) = callback {
        callback.call(cx, start, end);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::core::events::{Key, Modifiers};

    #[derive(Default)]
    struct Counter {
        count: u32,
        text: String,
        flag: bool,
        range: (usize, usize),
    }

    fn key_event() -> KeyEvent {
        KeyEvent {
            key: Key::Enter,
            modifiers: Modifiers::default(),
            repeat: false,
        }
    }

    #[test]
    fn action_with_state_mutates_the_root() {
        let mut state = Counter::default();
        let increment = action(|s: &mut Counter| s.count += 1);
        {
            let mut cx = ActionCx::new(&mut state);
            increment.call(&mut cx);
            increment.call(&mut cx);
        }
        assert_eq!(state.count, 2);
    }

    #[test]
    fn action_without_state_uses_the_closure() {
        let calls = Rc::new(Cell::new(0));
        let cloned = Rc::clone(&calls);
        let action = Action::new(move |_| cloned.set(cloned.get() + 1));
        let mut state = Counter::default();
        let mut cx = ActionCx::new(&mut state);
        action.call(&mut cx);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn cloned_actions_share_the_closure() {
        let calls = Rc::new(Cell::new(0));
        let cloned = Rc::clone(&calls);
        let action = Action::new(move |_| cloned.set(cloned.get() + 1));
        let copy = action.clone();
        let mut state = Counter::default();
        let mut cx = ActionCx::new(&mut state);
        action.call(&mut cx);
        copy.call(&mut cx);
        assert_eq!(calls.get(), 2);
    }

    #[test]
    #[should_panic(expected = "action expects the root state")]
    fn action_with_the_wrong_state_panics() {
        let mut state = Counter::default();
        let wrong = action(|s: &mut String| s.push('x'));
        let mut cx = ActionCx::new(&mut state);
        wrong.call(&mut cx);
    }

    #[test]
    fn typed_action_helpers_match_their_payloads() {
        let mut state = Counter::default();
        {
            let mut cx = ActionCx::new(&mut state);
            bool_action(|s: &mut Counter, checked| s.flag = checked).call(&mut cx, true);
            text_action(|s: &mut Counter, value| s.text.push_str(value)).call(&mut cx, "hi");
            index_action(|s: &mut Counter, index| s.count = index as u32).call(&mut cx, 7);
            range_action(|s: &mut Counter, start, end| s.range = (start, end)).call(&mut cx, 3, 5);
        }
        assert!(state.flag);
        assert_eq!(state.text, "hi");
        assert_eq!(state.count, 7);
        assert_eq!(state.range, (3, 5));
    }

    #[test]
    fn pointer_and_key_actions_report_changes() {
        let mut state = Counter::default();
        let mut cx = ActionCx::new(&mut state);
        let pointer = PointerAction::new(|_, info| info.phase == PointerPhase::Up);
        let info = PointerInfo {
            phase: PointerPhase::Up,
            ..Default::default()
        };
        assert!(pointer.call(&mut cx, info));
        assert!(!pointer.call(&mut cx, PointerInfo::default()));
        let key = KeyAction::new(|_, event| event.key == Key::Enter);
        assert!(key.call(&mut cx, key_event()));
        let input = InputKeyAction::new(|_, event, text, anchor, cursor| {
            event.key == Key::Enter && text == "abc" && anchor == 0 && cursor == 3
        });
        assert!(input.call(&mut cx, key_event(), "abc", 0, 3));
    }

    #[test]
    fn pointer_info_has_the_expected_defaults() {
        let info = PointerInfo::default();
        assert_eq!(info.phase, PointerPhase::Down);
        assert_eq!(info.button, PointerButton::Left);
        assert_eq!(info.click_count, 1);
        assert_eq!(info.modifiers, Modifiers::default());
    }

    #[test]
    fn option_helpers_ignore_missing_callbacks() {
        let mut state = Counter::default();
        {
            let mut cx = ActionCx::new(&mut state);
            call(None, &mut cx);
            call_bool(None, &mut cx, true);
            call_text(None, &mut cx, "x");
            call_index(None, &mut cx, 0);
            call_range(None, &mut cx, 0, 0);
            call(Some(action(|s: &mut Counter| s.count = 1)), &mut cx);
        }
        assert_eq!(state.count, 1);
    }
}
