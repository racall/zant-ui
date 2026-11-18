//! `createWindow(Panel, { … })` in TSX: another window of the app rendering a
//! component without props.
//!
//! The window opens the first time it is used (`show`, `toggle`, …) and keeps
//! its component's state, like any instance, in its own runtime. Hooks inside
//! the component (`useKeyDown`, `useCloseRequest`, `useWindowFocus`) become
//! this window's.
//!
//! The window hooks are closures that capture the component's shared
//! `local_state`, and the generated `impl tsx::window::Component for Panel` rebuilds
//! the component from it:
//!
//! ```ignore
//! impl tsx::window::Component for Panel {
//!     type Local = PanelLocal;
//!     fn with_local(local: std::rc::Rc<std::cell::RefCell<PanelLocal>>) -> Panel {
//!         Panel { _s: local }
//!     }
//!     fn render(&mut self) -> zant::ui::Node { … }
//! }
//! ```
//!
//! `Window` is `!Send` (UI-thread only), like every other UI value.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use crate::desktop;
use crate::theme::tokens;
use crate::ui::Node;

use super::app::{KeyEvent, current_theme, key_event};

/// A TSX component that can live in its own window: a component without props
/// whose state is its `Local`.
pub trait Component: Sized + 'static {
    /// The component's `local_state` type.
    type Local: Default + 'static;

    /// Builds the component around its shared local state (the codegen sets
    /// `_s`).
    fn with_local(local: Rc<RefCell<Self::Local>>) -> Self;

    /// Builds this window's tree (must return a container).
    fn render(&mut self) -> Node;

    /// `useKeyDown`: true when the component handled the key.
    fn on_key(&mut self, _event: &KeyEvent) -> bool {
        false
    }

    /// `useWindowFocus`.
    fn on_focus(&mut self, _focused: bool) {}

    /// `useCloseRequest`: false keeps the window open.
    fn on_close_request(&mut self) -> bool {
        true
    }
}

/// Another window of the app rendering `C`.
///
/// Every method takes `&self` and releases its own borrows before it calls
/// the native window: hiding / showing a window makes the system send focus
/// events at once, and the component's `useWindowFocus` may use this window
/// again (`if (!focused) panel.hide()`), so the calls must be re-entrant.
pub struct Window<C: Component> {
    options: RefCell<desktop::OpenOptions>,
    /// `setMaxSize` before the window opened (the open options have no
    /// maximum): applied when it opens.
    max_size: Cell<Option<(f32, f32)>>,
    handle: RefCell<Option<desktop::Window>>,
    local: Rc<RefCell<C::Local>>,
}

/// The `thread_local!` slot behind a TSX `createWindow` constant: the window
/// is created on first use and shared, so a call made while another call on
/// the same window is running (a focus event during `hide`) sees it too.
pub struct Slot<C: Component> {
    window: OnceCell<Rc<Window<C>>>,
}

impl<C: Component> Slot<C> {
    pub const fn new() -> Slot<C> {
        Slot {
            window: OnceCell::new(),
        }
    }

    /// The window, created with `options()` the first time.
    pub fn get(&self, options: impl FnOnce() -> desktop::OpenOptions) -> Rc<Window<C>> {
        Rc::clone(self.window.get_or_init(|| Rc::new(Window::new(options()))))
    }
}

impl<C: Component> Default for Slot<C> {
    fn default() -> Self {
        Slot::new()
    }
}

/// Runs a window hook unless the component's state is in use further up the
/// stack (a hook of this window re-entered by a native call): skipping it
/// beats a double borrow inside a native callback.
fn with_free_local<C: Component, R>(
    local: &Rc<RefCell<C::Local>>,
    fallback: R,
    hook: impl FnOnce(C) -> R,
) -> R {
    if local.try_borrow_mut().is_err() {
        return fallback;
    }
    hook(C::with_local(Rc::clone(local)))
}

impl<C: Component> Window<C> {
    /// A window that opens on first use with `options`.
    pub fn new(options: desktop::OpenOptions) -> Window<C> {
        Window {
            options: RefCell::new(options),
            max_size: Cell::new(None),
            handle: RefCell::new(None),
            local: Rc::new(RefCell::new(C::Local::default())),
        }
    }

    /// The native window when it is open (no borrow is held afterwards).
    fn handle(&self) -> Option<desktop::Window> {
        self.handle.borrow().clone()
    }

    /// The window, opened (hidden) on first use; `None` if it cannot open.
    fn window(&self) -> Option<desktop::Window> {
        if let Some(handle) = self.handle() {
            return Some(handle);
        }
        let mut options = self.options.borrow().clone();
        options.window.hidden = true;
        let render_local = Rc::clone(&self.local);
        let key_local = Rc::clone(&self.local);
        let focus_local = Rc::clone(&self.local);
        let close_local = Rc::clone(&self.local);
        let hooks = desktop::WindowHooks {
            render: Rc::new(move || C::with_local(Rc::clone(&render_local)).render()),
            on_key: Some(Rc::new(move |event| {
                with_free_local::<C, _>(&key_local, false, |mut c| c.on_key(&key_event(event)))
            })),
            on_focus: Some(Rc::new(move |focused| {
                with_free_local::<C, _>(&focus_local, false, |mut c| {
                    c.on_focus(focused);
                    true
                })
            })),
            on_close_request: Some(Rc::new(move || {
                // busy: keep the window; the request can be repeated
                with_free_local::<C, _>(&close_local, false, |mut c| c.on_close_request())
            })),
        };
        match desktop::open_window(hooks, options.clone()) {
            Ok(window) => {
                if let Some((width, height)) = self.max_size.get() {
                    window.set_max_size(width, height);
                }
                *self.handle.borrow_mut() = Some(window.clone());
                Some(window)
            }
            Err(error) => {
                eprintln!("zant: window \"{}\": {error}", options.title);
                None
            }
        }
    }

    /// Shows the window (opening it on first use).
    pub fn show(&self) {
        if let Some(window) = self.window() {
            window.show();
        }
    }

    /// Hides the window when it is open.
    pub fn hide(&self) {
        if let Some(window) = self.handle() {
            window.hide();
        }
    }

    /// Shows the window and gives it the keyboard.
    pub fn activate(&self) {
        if let Some(window) = self.window() {
            window.activate();
        }
    }

    /// Hidden → shown and focused; shown → hidden.
    pub fn toggle(&self) {
        if self.visible() {
            self.hide();
        } else {
            self.activate();
        }
    }

    pub fn visible(&self) -> bool {
        self.handle().is_some_and(|window| window.is_visible())
    }

    /// Shown and the app's key window (has the keyboard).
    pub fn active(&self) -> bool {
        self.handle()
            .is_some_and(|window| window.is_visible() && window.is_active())
    }

    pub fn minimized(&self) -> bool {
        self.handle().is_some_and(|window| window.is_minimized())
    }

    /// Moves it to a spot of the screen under the pointer.
    pub fn place(&self, place: desktop::Place, margin: f32) {
        if let Some(window) = self.window() {
            window.place(place, margin);
        }
    }

    /// Focuses the node with `key` after the window's next layout.
    pub fn focus(&self, key: &str) {
        if let Some(window) = self.window() {
            window.focus(key);
        }
    }

    /// `setTitle`: the system title; an unopened window opens with it.
    pub fn set_title(&self, title: &str) {
        self.options.borrow_mut().title = title.to_owned();
        if let Some(window) = self.handle() {
            window.set_title(title);
        }
    }

    /// `setSize`: the content size in points.
    pub fn set_size(&self, width: f64, height: f64) {
        {
            let mut options = self.options.borrow_mut();
            options.window.width = width as f32;
            options.window.height = height as f32;
        }
        if let Some(window) = self.handle() {
            window.set_size(width as f32, height as f32);
        }
    }

    pub fn set_min_size(&self, width: f64, height: f64) {
        {
            let mut options = self.options.borrow_mut();
            options.window.min_width = width as f32;
            options.window.min_height = height as f32;
        }
        if let Some(window) = self.handle() {
            window.set_min_size(width as f32, height as f32);
        }
    }

    /// `setMaxSize`: 0 in a dimension leaves it unlimited.
    pub fn set_max_size(&self, width: f64, height: f64) {
        self.max_size.set(Some((width as f32, height as f32)));
        if let Some(window) = self.handle() {
            window.set_max_size(width as f32, height as f32);
        }
    }

    pub fn set_always_on_top(&self, on: bool) {
        self.options.borrow_mut().window.always_on_top = on;
        if let Some(window) = self.handle() {
            window.set_always_on_top(on);
        }
    }

    /// `setFullscreen`: opens the window when needed.
    pub fn set_fullscreen(&self, on: bool) {
        if let Some(window) = self.window() {
            window.set_fullscreen(on);
        }
    }

    pub fn fullscreen(&self) -> bool {
        self.handle().is_some_and(|window| window.is_fullscreen())
    }

    /// The theme windows of the app use now.
    pub fn theme(&self) -> tokens::Theme {
        current_theme()
    }

    /// The component's shared state (`_s`).
    pub fn local(&self) -> &Rc<RefCell<C::Local>> {
        &self.local
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::builders::text;
    use crate::ui::options::TextOptions;

    struct Panel {
        local: Rc<RefCell<u32>>,
        keys: Vec<String>,
    }

    impl Component for Panel {
        type Local = u32;

        fn with_local(local: Rc<RefCell<u32>>) -> Panel {
            Panel {
                local,
                keys: Vec::new(),
            }
        }

        fn render(&mut self) -> Node {
            *self.local.borrow_mut() += 1;
            text(
                "panel",
                TextOptions {
                    key: Some("panel".into()),
                    ..Default::default()
                },
            )
        }

        fn on_key(&mut self, event: &KeyEvent) -> bool {
            self.keys.push(event.key.to_string());
            event.key.as_str() == "Escape"
        }
    }

    thread_local! {
        static SLOT: Slot<Panel> = const { Slot::new() };
    }

    fn panel_with<R>(apply: impl FnOnce(&Window<Panel>) -> R) -> R {
        let window = SLOT.with(|slot| slot.get(desktop::OpenOptions::default));
        apply(&window)
    }

    #[test]
    fn a_window_used_again_inside_its_own_call_does_not_panic() {
        // what a focus hook does while `hide` runs: use the same window again
        panel_with(|outer| {
            outer.hide();
            panel_with(|inner| {
                assert!(std::ptr::eq(outer, inner));
                inner.hide();
                inner.set_title("Again");
                assert!(!inner.visible());
            });
        });
        // a hook of a component whose state is in use is skipped, not a panic
        let local = Rc::new(RefCell::new(0u32));
        let held = local.borrow_mut();
        assert!(!with_free_local::<Panel, _>(&local, false, |_| true));
        drop(held);
        assert!(with_free_local::<Panel, _>(&local, false, |_| true));
    }

    #[test]
    fn a_window_opens_lazily_and_keeps_its_local_state() {
        let window = Window::<Panel>::new(desktop::OpenOptions {
            title: String::from("Panel"),
            ..Default::default()
        });
        assert!(!window.visible());
        assert!(!window.active());
        assert!(!window.minimized());
        // without a running app the native window cannot open; the handle stays
        // empty and the calls are no-ops
        window.hide();
        window.toggle();
        assert!(!window.visible());
        // the component's local state is shared with the render closures
        let local = Rc::clone(window.local());
        let mut panel = Panel::with_local(local);
        panel.render();
        assert_eq!(*window.local().borrow(), 1);
        assert!(panel.on_key(&KeyEvent {
            key: "Escape".into(),
            ..Default::default()
        }));
        assert!(!panel.on_key(&KeyEvent {
            key: "f".into(),
            ..Default::default()
        }));
        assert_eq!(panel.keys, vec!["Escape", "f"]);
        assert!(panel.on_close_request());
        panel.on_focus(true);
        assert_eq!(window.theme(), current_theme());
        assert_eq!(window.local().borrow_mut().to_string(), "1");
        let _ = crate::ui::builders::text("x", TextOptions::default());
    }
}
