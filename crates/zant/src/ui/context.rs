//! The build context: the thread-local state of `src/ui.zig:94-470`.
//!
//! Zig had ~18 separate `threadlocal var`s (theme, frame clock, display scale,
//! queued effects / portals, pending document flag, …). Rust keeps them in one
//! [`BuildCx`] behind a single `RefCell`.
//!
//! **Hard rule (plan §4): release the `CX` borrow before calling user code**
//! (`render`, a memo build closure, any `Action`). Those call `ui::*`
//! functions, which borrow `CX` again, and a nested `borrow_mut()` panics.
//! [`with_cx`] borrows only for the duration of the closure it is given; keep
//! those closures to plain field reads / writes. [`run_effects`] is the
//! reference shape: take the queue out, drop the borrow, then call callbacks.
#![forbid(unsafe_code)]

use std::cell::RefCell;

use crate::Str;
use crate::platform::platform::{self, TitlebarInsets, WindowStyle};
use crate::theme::tokens::Theme;

use super::actions::{Action, ActionCx};
use super::element::Node;

/// All build-context state of one thread (each window builds on one thread).
pub struct BuildCx {
    theme: Theme,
    frame_active: bool,
    now_ms: i64,
    display_scale: f32,
    window_style: WindowStyle,
    pending_wake_ms: Option<u32>,
    pending_edited: Option<bool>,
    effects: Vec<Action>,
    portals: Vec<Node>,
    rebuild_requested: bool,
}

impl Default for BuildCx {
    /// Zig module-level defaults: light theme, scale 1, native window style.
    fn default() -> Self {
        BuildCx {
            theme: Theme::default(),
            frame_active: false,
            now_ms: 0,
            display_scale: 1.0,
            window_style: WindowStyle::Native,
            pending_wake_ms: None,
            pending_edited: None,
            effects: Vec::new(),
            portals: Vec::new(),
            rebuild_requested: false,
        }
    }
}

thread_local! {
    /// The one build context of the current thread; see the module docs for
    /// the "never call user code while borrowed" rule.
    static CX: RefCell<BuildCx> = RefCell::new(BuildCx::default());
}

/// Borrows the build context for a short, non-reentrant closure.
///
/// Panics when called from inside another `with_cx` closure (a nested
/// `borrow_mut()`); use it for field reads / writes only, never around user
/// code.
pub fn with_cx<R>(f: impl FnOnce(&mut BuildCx) -> R) -> R {
    CX.with(|cx| f(&mut cx.borrow_mut()))
}

/// Theme used by `ui::*` builders for their default colours; the runtime sets
/// it before every build from the active theme (light / dark).
pub fn set_build_theme(theme: Theme) {
    with_cx(|build| build.theme = theme);
}

/// Active theme while building a frame (`ui::currentTheme`).
pub fn current_theme() -> Theme {
    with_cx(|build| build.theme)
}

/// Pixels per logical px of the window's display (2 on Retina, 1.5 at 150% on
/// Windows). Use it to pick the resolution of rendered images.
pub fn display_scale() -> f32 {
    with_cx(|build| build.display_scale)
}

/// Set by the runtime before every build; values <= 0 are ignored (Zig).
pub fn set_display_scale(scale: f32) {
    with_cx(|build| build.display_scale = if scale > 0.0 { scale } else { 1.0 });
}

/// Timestamp (ms, monotonic) of the current frame or event (`ui::nowMs`).
pub fn now_ms() -> i64 {
    with_cx(|build| build.now_ms)
}

/// Sets the timestamp [`now_ms`] reports (the runtime samples it per frame).
pub fn set_now(ms: i64) {
    with_cx(|build| build.now_ms = ms);
}

/// Queues `callback` to run after this build has been reconciled (React's
/// `useEffect`). An effect that changes state calls [`request_rebuild`].
///
/// The context of a Rust closure is a capture, so nothing has to be copied.
pub fn effect(callback: Action) {
    with_cx(|build| build.effects.push(callback));
}

/// From an effect: its state changes need another build.
pub fn request_rebuild() {
    with_cx(|build| build.rebuild_requested = true);
}

/// Runs the effects queued by the last build; true when one of them asked for
/// another build.
///
/// The queue is moved out (and the `CX` borrow released) before any callback
/// runs, so effects may call `ui::*` / actions freely.
pub fn run_effects(action_cx: &mut ActionCx<'_>) -> bool {
    let queued = with_cx(|build| {
        let queued = std::mem::take(&mut build.effects);
        build.rebuild_requested = false;
        queued
    });
    for callback in &queued {
        callback.call(action_cx);
    }
    with_cx(|build| {
        let requested = build.rebuild_requested;
        build.rebuild_requested = false;
        requested
    })
}

/// Renders `node` as the last child of the window root instead of here (a
/// React portal); the storage half of WP-2.4's `ui::portal`.
pub fn push_portal(node: Node) {
    with_cx(|build| build.portals.push(node));
}

/// The nodes [`push_portal`] moved out of this build, in build order (the
/// runtime appends them to the root).
pub fn take_portals() -> Vec<Node> {
    with_cx(|build| std::mem::take(&mut build.portals))
}

/// Requests a rebuild after `ms` milliseconds (call it from `render`; e.g. to
/// hide a toast). Valid for the build that requested it.
pub fn wake_after(ms: u32) {
    with_cx(|build| {
        build.pending_wake_ms = Some(match build.pending_wake_ms {
            Some(current) => current.min(ms),
            None => ms,
        });
    });
}

/// The wake-up requested by the last build (runtime).
pub fn take_wake() -> Option<u32> {
    with_cx(|build| build.pending_wake_ms.take())
}

/// Declares whether the document has unsaved changes (call it from `render`).
pub fn document_edited(edited: bool) {
    with_cx(|build| build.pending_edited = Some(edited));
}

/// The value declared by the last build, if any (runtime).
pub fn take_document_edited() -> Option<bool> {
    with_cx(|build| build.pending_edited.take())
}

/// Window style of the app being built (set by the runtime before render).
pub fn set_build_window_style(style: WindowStyle) {
    with_cx(|build| build.window_style = style);
}

/// Space an app-drawn title bar (`.hidden_titlebar`) must keep free for the
/// system's window controls: the traffic lights on the left on macOS, the
/// caption buttons on the right on Windows.
pub fn titlebar_insets() -> TitlebarInsets {
    platform::titlebar_insets(with_cx(|build| build.window_style))
}

/// Starts a build: clears the per-frame queues (Zig `beginFrame` resets its
/// frame arena; Rust nodes own their data, so there is nothing to free).
pub fn begin_frame() {
    with_cx(|build| {
        build.effects.clear();
        build.portals.clear();
        build.pending_wake_ms = None;
        build.pending_edited = None;
        build.frame_active = true;
    });
}

pub fn end_frame() {
    with_cx(|build| build.frame_active = false);
}

pub fn is_frame_active() -> bool {
    with_cx(|build| build.frame_active)
}

/// Copies a runtime `&str` for storage in a node (Zig `ui.frameString` copied
/// into the frame arena; Rust strings are shared `Str`s).
pub fn frame_string(value: &str) -> Str {
    Str::copy(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reset() {
        with_cx(|build| *build = BuildCx::default());
    }

    #[test]
    fn theme_round_trips_and_defaults_to_light() {
        reset();
        assert_eq!(current_theme(), Theme::default());
        let dark = crate::theme::tokens::ant_dark();
        set_build_theme(dark);
        assert_eq!(current_theme(), dark);
        reset();
    }

    #[test]
    fn frame_clock_and_display_scale() {
        reset();
        assert_eq!(now_ms(), 0);
        set_now(1234);
        assert_eq!(now_ms(), 1234);
        assert_eq!(display_scale(), 1.0);
        set_display_scale(2.0);
        assert_eq!(display_scale(), 2.0);
        set_display_scale(0.0);
        assert_eq!(display_scale(), 1.0, "non-positive scales fall back to 1");
        reset();
    }

    #[test]
    fn effects_run_after_the_borrow_is_released() {
        reset();
        let seen = std::rc::Rc::new(std::cell::Cell::new(0));
        let captured = std::rc::Rc::clone(&seen);
        effect(Action::new(move |_| {
            // Re-entering the build context from an effect must not panic.
            let _ = current_theme();
            captured.set(captured.get() + 1);
            request_rebuild();
        }));
        effect(Action::new(|_| {}));
        let mut state = 0u32;
        let mut cx = ActionCx::new(&mut state);
        assert!(run_effects(&mut cx), "the first effect asked for a rebuild");
        assert_eq!(seen.get(), 1);
        // the queue is empty now
        assert!(!run_effects(&mut cx));
        assert_eq!(seen.get(), 1);
        reset();
    }

    #[test]
    fn portals_are_queued_and_taken() {
        reset();
        assert!(take_portals().is_empty());
        push_portal(Node::default());
        push_portal(Node::default());
        assert_eq!(take_portals().len(), 2);
        assert!(take_portals().is_empty());
        reset();
    }

    #[test]
    fn begin_frame_clears_the_per_frame_state() {
        reset();
        effect(Action::new(|_| {}));
        push_portal(Node::default());
        wake_after(50);
        document_edited(true);
        assert!(!is_frame_active());
        begin_frame();
        assert!(is_frame_active());
        assert!(take_portals().is_empty());
        assert_eq!(take_wake(), None);
        assert_eq!(take_document_edited(), None);
        let mut state = 0u32;
        let mut cx = ActionCx::new(&mut state);
        assert!(!run_effects(&mut cx), "queued effects were dropped");
        end_frame();
        assert!(!is_frame_active());
        reset();
    }

    #[test]
    fn wake_after_keeps_the_soonest_deadline() {
        reset();
        assert_eq!(take_wake(), None);
        wake_after(500);
        wake_after(200);
        wake_after(700);
        assert_eq!(take_wake(), Some(200));
        assert_eq!(take_wake(), None);
        reset();
    }

    #[test]
    fn document_edited_reports_the_last_declaration() {
        reset();
        assert_eq!(take_document_edited(), None);
        document_edited(true);
        document_edited(false);
        assert_eq!(take_document_edited(), Some(false));
        assert_eq!(take_document_edited(), None);
        reset();
    }

    #[test]
    fn titlebar_insets_follow_the_window_style() {
        reset();
        assert_eq!(titlebar_insets(), TitlebarInsets::default());
        set_build_window_style(WindowStyle::HiddenTitlebar);
        let insets = titlebar_insets();
        if cfg!(windows) {
            assert_eq!(insets.left, 0.0);
            assert_eq!(insets.right, platform::CAPTION_BUTTON_WIDTH * 3.0);
        } else {
            assert_eq!(insets.left, 84.0);
            assert_eq!(insets.right, 0.0);
        }
        reset();
    }

    #[test]
    fn frame_string_copies_runtime_text() {
        let value = String::from("hello");
        let copied = frame_string(&value);
        assert_eq!(copied, "hello");
    }
}
