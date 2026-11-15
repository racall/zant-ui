//! Multi-window support: the window registry, the shared application state
//! and the hooks of windows opened with `desktop::open_window` (opening /
//! closing / invalidating windows and the wake fan-out).
//!
//! The application state lives in one [`SharedApp`]
//! (`Rc<RefCell<Box<dyn App>>>`) that all runtimes clone, and the registry
//! holds weak references so a closed window drops out
//! ([`register`] / [`unregister`], plus the `Drop` of [`Runtime`]).
//!
//! Everything happens on the UI thread: `Runtime` is `!Send`, the registry is
//! a `thread_local` and window registrations must happen there.
#![forbid(unsafe_code)]

use std::cell::{Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

use crate::core::events::KeyEvent;
use crate::ui::app::App;
use crate::ui::element::Node;

use super::Runtime;

// ------------------------------------------------------------------ app state

/// The application state shared by all windows of the app.
///
/// Every `Runtime` and every window hook shares the boxed [`App`]. A later window
/// created with `desktop::open_window` renders the same state, and actions in
/// any window can change it.
#[derive(Clone)]
pub struct SharedApp {
    inner: Rc<RefCell<Box<dyn App>>>,
}

impl SharedApp {
    /// Boxes `app` as the state of the app's windows (`ui::app::run`).
    pub fn new(app: impl App) -> SharedApp {
        SharedApp {
            inner: Rc::new(RefCell::new(Box::new(app))),
        }
    }

    /// Immutable access to the state (theme hooks, tests).
    pub fn borrow(&self) -> Ref<'_, Box<dyn App>> {
        self.inner.borrow()
    }

    /// Mutable access to the state (render, event hooks).
    ///
    /// The borrow must be released before user code calls back into the
    /// runtime.
    pub fn borrow_mut(&self) -> RefMut<'_, Box<dyn App>> {
        self.inner.borrow_mut()
    }

    /// True when both handles are the same state (the main window of this app).
    pub fn ptr_eq(&self, other: &SharedApp) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

// -------------------------------------------------------------- window hooks

/// Builds a window's tree: it must return a container.
pub type RenderHook = Rc<dyn Fn() -> Node>;
/// A window's key handler: return true to consume the key.
pub type KeyHook = Rc<dyn Fn(&KeyEvent) -> bool>;
/// A window's focus handler (gained / lost the keyboard): return true when
/// state changed.
pub type FocusHook = Rc<dyn Fn(bool) -> bool>;
/// A window's close-request handler: return false to veto the close.
pub type CloseHook = Rc<dyn Fn() -> bool>;

/// Hooks of a window opened with `desktop::open_window`; `render` replaces the app's `App::render` for that
/// window, the other hooks answer its events.
///
/// The hooks close over whatever state the window renders (for a TSX
/// `createWindow` panel that is the panel component's local state); the app
/// state is shared implicitly.
#[derive(Clone)]
pub struct WindowHooks {
    /// Builds this window's tree. It must return a container, like
    /// `App::render`.
    pub render: RenderHook,
    /// Keys before the focused widget (Esc to hide a panel).
    pub on_key: Option<KeyHook>,
    /// The window gained (`true`) / lost (`false`) the keyboard; a panel
    /// typically hides when it loses focus.
    pub on_focus: Option<FocusHook>,
    /// A close was requested (system menu, Alt+F4).
    pub on_close_request: Option<CloseHook>,
}

impl WindowHooks {
    /// Hooks with only a render function, the required one.
    pub fn new(render: impl Fn() -> Node + 'static) -> WindowHooks {
        WindowHooks {
            render: Rc::new(render),
            on_key: None,
            on_focus: None,
            on_close_request: None,
        }
    }
}

// ------------------------------------------------------------------ registry

thread_local! {
    /// Windows of the app on the UI thread, main window first. Weak: closing
    /// a window unregisters it.
    static OPEN_RUNTIMES: RefCell<Vec<Weak<RefCell<Runtime>>>> = const { RefCell::new(Vec::new()) };
    /// Windows whose native close must run after the current event callback
    /// returns (a window may not destroy itself from inside its callback);
    /// stored as the opaque owner address of the runtime.
    static PENDING_CLOSES: RefCell<Vec<*const ()>> = const { RefCell::new(Vec::new()) };
}

/// The `ZANT_FRAME_STATS` labels of the windows opened after the main one
/// (`:2`, `:3`, … per window).
const FRAME_STATS_LABELS: [&str; 8] = [":2", ":3", ":4", ":5", ":6", ":7", ":8", ":9"];

/// Registers an open window.
pub(crate) fn register(core: &Rc<RefCell<Runtime>>) {
    OPEN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        // a re-registration replaces the old entry instead of duplicating it
        let address = Rc::as_ptr(core).cast::<()>();
        runtimes.retain(|weak| match weak.upgrade() {
            Some(existing) => !std::ptr::eq(Rc::as_ptr(&existing).cast::<()>(), address),
            None => false,
        });
        runtimes.push(Rc::downgrade(core));
    });
}

/// Removes an open window from the registry.
pub(crate) fn unregister(core: &Rc<RefCell<Runtime>>) {
    let address = Rc::as_ptr(core).cast::<()>();
    OPEN_RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().retain(|weak| match weak.upgrade() {
            Some(existing) => !std::ptr::eq(Rc::as_ptr(&existing).cast::<()>(), address),
            None => false,
        });
    });
}

/// `Runtime`'s drop: an unregistered-but-still-alive window leaves a dead weak.
fn unregister_ptr(pointer: *const Runtime) {
    OPEN_RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().retain(|weak| match weak.upgrade() {
            Some(existing) => !std::ptr::eq(existing.as_ptr().cast_const(), pointer),
            None => false,
        });
    });
}

/// Number of open windows (dead weaks are pruned).
pub fn open_count() -> usize {
    OPEN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.retain(|weak| weak.strong_count() > 0);
        runtimes.len()
    })
}

/// The frame stats label of the next window opened (`":2"` for the first one
/// after the main window).
pub(crate) fn next_frame_stats_label() -> &'static str {
    let index = open_count()
        .saturating_sub(1)
        .min(FRAME_STATS_LABELS.len() - 1);
    FRAME_STATS_LABELS[index]
}

/// Rebuilds every open window: they may render
/// the same application state, so a change in one window changes all of them.
pub fn invalidate_all() {
    OPEN_RUNTIMES.with(|runtimes| {
        for weak in runtimes.borrow().iter() {
            let Some(core) = weak.upgrade() else {
                continue;
            };
            // A window that is dispatching / painting is already borrowed and
            // invalidates itself; skipping it also avoids a `RefCell` panic.
            if let Ok(mut runtime) = core.try_borrow_mut() {
                runtime.invalidate();
            }
        }
    });
}

// --------------------------------------------------------------- closing

/// Asks the UI thread to close the window whose runtime lives at `owner` after
/// the current event callback returns (deferred: a native window must not be
/// destroyed from inside its own callback).
pub(crate) fn request_close(owner: *const ()) {
    PENDING_CLOSES.with(|pending| pending.borrow_mut().push(owner));
    // The wake callback of the main window runs on a fresh event-loop turn;
    // `process_pending_closes` runs there (see `runtime::host`).
    crate::platform::platform::post_wake();
}

/// Closes a window opened with `desktop::open_window`: it leaves the registry
/// and its native window is destroyed. The
/// `Runtime` itself is released when the last `desktop::Window` handle drops.
pub fn close_window(core: &Rc<RefCell<Runtime>>) {
    unregister(core);
    if let Ok(mut runtime) = core.try_borrow_mut() {
        runtime.close_native_window();
    }
}

/// Runs the deferred closes queued by [`request_close`] (`runtime::host` calls
/// it once the runtime borrow is released).
pub(crate) fn process_pending_closes() {
    let pending = PENDING_CLOSES.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    if pending.is_empty() {
        return;
    }
    let mut retry = Vec::new();
    for owner in pending {
        let core = OPEN_RUNTIMES.with(|runtimes| {
            runtimes.borrow().iter().find_map(|weak| {
                let core = weak.upgrade()?;
                let matches = core
                    .try_borrow()
                    .map(|runtime| runtime.owner_ptr() == owner)
                    .unwrap_or(false);
                matches.then_some(core)
            })
        });
        let Some(core) = core else {
            continue;
        };
        if core.try_borrow().is_ok() {
            close_window(&core);
        } else {
            retry.push(owner);
        }
    }
    if !retry.is_empty() {
        PENDING_CLOSES.with(|pending| pending.borrow_mut().extend(retry));
    }
}

/// Forgets every window (the app is ending).
#[cfg(test)]
pub(crate) fn clear() {
    OPEN_RUNTIMES.with(|runtimes| runtimes.borrow_mut().clear());
    PENDING_CLOSES.with(|pending| pending.borrow_mut().clear());
}

/// Releases every app runtime, including cycles through app-owned window
/// handles. The host's adopted window is detached, never destroyed.
pub(crate) fn release_app() {
    struct Released;
    impl App for Released {
        fn render(&mut self) -> Node {
            Node::default()
        }
    }

    // Hold all runtimes alive until every cycle is broken. Never drop app
    // values under the registry or runtime borrow (destructors may re-enter).
    let runtimes = OPEN_RUNTIMES.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    let runtimes: Vec<_> = runtimes
        .into_iter()
        .filter_map(|weak| weak.upgrade())
        .collect();
    for core in &runtimes {
        let old = std::mem::replace(
            &mut *core.borrow_mut(),
            Runtime::init(Released, crate::ui::RunOptions::default()),
        );
        drop(old);
    }
    PENDING_CLOSES.with(|pending| pending.borrow_mut().clear());
}

#[cfg(test)]
pub(crate) fn assert_app_released() {
    OPEN_RUNTIMES.with(|runtimes| assert!(runtimes.borrow().is_empty()));
    PENDING_CLOSES.with(|pending| assert!(pending.borrow().is_empty()));
}

impl Runtime {
    /// Creates a runtime for an extra window: it shares `app` with the other
    /// windows and renders through `hooks`.
    pub fn init_window(app: SharedApp, hooks: WindowHooks, config: super::Config) -> Runtime {
        Runtime::with_shared(app, Some(hooks), config)
    }

    /// Creates a runtime that shares an existing application state but renders
    /// through `App::render` (tests; `desktop::open_window` uses `init_window`).
    #[cfg(test)]
    pub fn init_shared(app: SharedApp, config: super::Config) -> Runtime {
        Runtime::with_shared(app, None, config)
    }

    /// The state handle shared with the app's other windows.
    pub fn shared_app(&self) -> SharedApp {
        self.app.clone()
    }

    /// Destroys this runtime's native window: dropping the last `Rc` calls
    /// `zant_window_destroy` (the `desktop::Window` handles only hold a weak
    /// reference).
    pub(crate) fn close_native_window(&mut self) {
        self.window = None;
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unregister_ptr(self as *const Runtime);
        // A per-build `ui::key_bounds` hook may still point at this window's
        // snapshot when the runtime is dropped between frames.
        crate::ui::set_key_bounds_hook(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::Event;
    use crate::core::node::NodeData;
    use crate::ui::RunOptions;
    use crate::ui::builders::{column, text};
    use crate::ui::element::Node;
    use crate::ui::options::{BoxOptions, TextOptions};

    #[derive(Default)]
    struct Counter {
        count: u32,
    }

    impl App for Counter {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![text(format!("{}", self.count), TextOptions::default())],
                ..Default::default()
            })
        }
    }

    fn shared_runtime(app: SharedApp) -> Rc<RefCell<Runtime>> {
        Rc::new(RefCell::new(Runtime::init_shared(
            app,
            RunOptions::default(),
        )))
    }

    #[test]
    fn windows_share_the_application_state() {
        let app = SharedApp::new(Counter::default());
        let first = shared_runtime(app.clone());
        let second = shared_runtime(app.clone());
        first.borrow_mut().app_state_mut::<Counter>().count = 7;
        assert_eq!(second.borrow().app_state_mut::<Counter>().count, 7);
        // a rebuild of the second window renders the shared state
        second.borrow_mut().rebuild_for_test();
        let rendered = {
            let second = second.borrow();
            second.tree.nodes.iter().find_map(|node| match &node.data {
                NodeData::Text(data) => Some(data.content.to_string()),
                _ => None,
            })
        };
        assert_eq!(rendered.as_deref(), Some("7"), "the shared count renders");
        assert!(
            !SharedApp::new(Counter::default()).ptr_eq(&app),
            "a fresh state is a different app"
        );
    }

    #[test]
    fn a_dispatch_in_one_window_invalidates_the_others() {
        let app = SharedApp::new(Counter::default());
        let first = shared_runtime(app.clone());
        let second = shared_runtime(app.clone());
        register(&first);
        register(&second);
        assert_eq!(open_count(), 2);
        // `window_close_request` asks the app and marks the window for a rebuild
        first.borrow_mut().dispatch(Event::WindowCloseRequest);
        assert!(first.borrow().needs_build);
        assert!(second.borrow().needs_build, "the other window rebuilds too");
        unregister(&first);
        unregister(&second);
        assert_eq!(open_count(), 0);
    }

    #[test]
    fn invalidate_all_rebuilds_every_open_window() {
        let app = SharedApp::new(Counter::default());
        let first = shared_runtime(app.clone());
        let second = shared_runtime(app.clone());
        register(&first);
        register(&second);
        first.borrow_mut().needs_build = false;
        second.borrow_mut().needs_build = false;
        invalidate_all();
        assert!(first.borrow().needs_build);
        assert!(second.borrow().needs_build);
        clear();
    }

    #[test]
    fn the_registry_prunes_closed_windows() {
        let app = SharedApp::new(Counter::default());
        let core = shared_runtime(app);
        register(&core);
        assert_eq!(open_count(), 1);
        drop(core);
        assert_eq!(open_count(), 0, "the weak reference was pruned");
    }

    #[test]
    fn frame_stats_labels_follow_the_open_windows() {
        assert_eq!(next_frame_stats_label(), ":2");
        let app = SharedApp::new(Counter::default());
        let first = shared_runtime(app.clone());
        let second = shared_runtime(app.clone());
        register(&first);
        assert_eq!(next_frame_stats_label(), ":2");
        register(&second);
        assert_eq!(next_frame_stats_label(), ":3");
        clear();
    }

    #[test]
    fn requested_closes_run_after_the_callback() {
        let app = SharedApp::new(Counter::default());
        let core = shared_runtime(app);
        register(&core);
        let owner = core.borrow().owner_ptr();
        request_close(owner);
        assert_eq!(open_count(), 1, "not closed while the callback runs");
        process_pending_closes();
        assert_eq!(open_count(), 0, "closed after the event");
        assert!(core.borrow().window().is_none());
    }

    #[test]
    fn a_window_without_hooks_renders_the_app() {
        let app = SharedApp::new(Counter::default());
        let core = shared_runtime(app);
        core.borrow_mut().rebuild_for_test();
        // the default Counter renders one text node under the root
        let root = core.borrow().tree.root;
        assert_eq!(core.borrow().tree.get(root).children.len(), 1);
    }

    #[test]
    fn window_hooks_drive_the_child_window() {
        struct Panel {
            shows: u32,
        }
        let app = SharedApp::new(Counter::default());
        let panel = Rc::new(RefCell::new(Panel { shows: 0 }));
        let shown = Rc::clone(&panel);
        let hooks = WindowHooks {
            on_key: Some(Rc::new(|event: &KeyEvent| {
                event.key == crate::core::events::Key::Escape
            })),
            on_focus: Some(Rc::new(|focused: bool| focused)),
            on_close_request: Some(Rc::new(|| false)),
            ..WindowHooks::new(move || {
                shown.borrow_mut().shows += 1;
                column(BoxOptions {
                    children: crate::nodes![text("panel", TextOptions::default())],
                    ..Default::default()
                })
            })
        };
        let mut runtime = Runtime::init_window(app, hooks, RunOptions::default());
        runtime.rebuild_for_test();
        assert_eq!(panel.borrow().shows, 1, "the hook rendered the window");
        // the key hook sees Esc before the focused widget
        let event = KeyEvent {
            key: crate::core::events::Key::Escape,
            modifiers: Default::default(),
            repeat: false,
        };
        assert_eq!(runtime.dispatch(Event::KeyDown(event)), 1);
        assert!(runtime.needs_build);
        // the close hook can veto
        assert_eq!(runtime.dispatch(Event::WindowCloseRequest), 1);
        // a child window never asks the app's quit hook
        assert_eq!(runtime.dispatch(Event::AppQuitRequest), 0);
        // a window close only schedules the deferred close
        assert_eq!(runtime.dispatch(Event::WindowClose), 0);
    }

    #[test]
    fn the_theme_hook_of_the_app_reaches_extra_windows() {
        struct Dark;
        impl App for Dark {
            fn render(&mut self) -> Node {
                column(BoxOptions::default())
            }
            fn theme(&self) -> Option<crate::theme::tokens::Theme> {
                Some(crate::theme::tokens::ant_dark())
            }
        }
        let app = SharedApp::new(Dark);
        let hooks = WindowHooks::new(|| column(BoxOptions::default()));
        let mut runtime = Runtime::init_window(app, hooks, RunOptions::default());
        runtime.rebuild_for_test();
        assert_eq!(runtime.theme, crate::theme::tokens::ant_dark());
    }
}
