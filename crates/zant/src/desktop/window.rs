//! Windows of the app: the main window and the extra windows opened with
//! [`open_window`].
//!
//! Every window renders the same application state (see
//! [`crate::runtime::window::SharedApp`]): an action in one window asks all of
//! them to rebuild. The runtime lives in an `Rc<RefCell<…>>` and this handle
//! holds only a weak
//! reference to the native window, so the plain window operations never borrow
//! the runtime (an app hook that hides its window from `on_close_request` must
//! not panic on a `RefCell` already borrowed by the dispatcher).
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::platform::platform::{self, PlatformError, Window as NativeWindow, WindowOptions};
use crate::runtime::Runtime;
use crate::runtime::host::RuntimeHost;
use crate::runtime::window as runtime_window;
use crate::runtime::window::SharedApp;
use crate::theme::tokens::Theme;
use crate::ui::RunOptions;
use crate::ui::commands::CommandKind;

/// Where [`Window::place`] puts a window, in the work area of the screen under
/// the pointer.
pub use crate::platform::platform::Place;

/// `true` on macOS.
pub const IS_MACOS: bool = platform::IS_MACOS;

/// Hooks of a window opened with [`open_window`]; `render` is required.
pub use crate::runtime::window::WindowHooks;

/// Options of a window opened with [`open_window`].
#[derive(Clone, Debug)]
pub struct OpenOptions {
    /// Window title (default "Zant UI").
    pub title: String,
    /// Platform window options (style, size, hidden, …).
    pub window: WindowOptions,
    /// Theme of the window before its first `App::theme()` hook runs.
    pub theme: Theme,
}

impl Default for OpenOptions {
    fn default() -> Self {
        OpenOptions {
            title: String::from("Zant UI"),
            window: WindowOptions::default(),
            theme: Theme::default(),
        }
    }
}

impl OpenOptions {
    /// The `RunOptions` of the window's runtime (title / window / theme).
    pub(crate) fn run_options(&self) -> RunOptions {
        RunOptions {
            title: self.title.clone(),
            window: self.window,
            theme: self.theme,
        }
    }
}

/// The main window's runtime and native window; `None` before
/// `ui::app::run` created it and after it ended.
struct MainWindow {
    core: Rc<RefCell<Runtime>>,
    app: SharedApp,
    native: Option<Weak<NativeWindow>>,
}

thread_local! {
    /// The main window on the UI thread.
    static MAIN: RefCell<Option<MainWindow>> = const { RefCell::new(None) };
}

/// Registers the window `ui::app::run` created; `None` when the app ends.
///
/// The app state handle is read here once: `main_window()` and
/// `open_window()` must work from inside a callback whose window is currently
/// borrowed, so they never borrow the runtime.
pub fn set_main_runtime(runtime: Option<Rc<RefCell<Runtime>>>) {
    let next = runtime.map(|core| {
        let (app, native) = {
            let runtime = core.borrow();
            (runtime.shared_app(), runtime.window().map(Rc::downgrade))
        };
        MainWindow { core, app, native }
    });
    let old = MAIN.with(|main| main.replace(next));
    drop(old);
}

#[cfg(test)]
pub(crate) fn assert_app_released() {
    MAIN.with(|main| assert!(main.borrow().is_none()));
}

/// The app's main window, `None` before `ui::app::run` created it.
pub fn main_window() -> Option<Window> {
    MAIN.with(|main| {
        let main = main.borrow();
        let state = main.as_ref()?;
        let native = state.native.as_ref()?.upgrade()?;
        Some(Window {
            core: Rc::clone(&state.core),
            native: Rc::downgrade(&native),
        })
    })
}

/// One window of the app: the main window ([`main_window`]) or a window
/// opened with [`open_window`].
pub struct Window {
    core: Rc<RefCell<Runtime>>,
    /// Weak: the runtime owns the last strong reference, so closing a window
    /// destroys its native window even while the app keeps this handle.
    native: Weak<NativeWindow>,
}

impl Window {
    fn native(&self) -> Option<Rc<NativeWindow>> {
        self.native.upgrade()
    }

    /// The command owner pointer of this window's runtime (the same opaque
    /// address `Runtime::owner_ptr` hands out).
    fn owner(&self) -> *const () {
        RefCell::as_ptr(&self.core) as *const ()
    }

    /// Asks for a rebuild of this window; a window owned by the dispatcher
    /// (already borrowed) just requests a redraw.
    fn invalidate(&self) {
        if let Ok(mut runtime) = self.core.try_borrow_mut() {
            runtime.invalidate();
        } else if let Some(native) = self.native() {
            native.request_redraw(0);
        }
    }

    fn is_main(&self) -> bool {
        MAIN.with(|main| {
            main.borrow()
                .as_ref()
                .is_some_and(|state| Rc::ptr_eq(&state.core, &self.core))
        })
    }

    /// Shows the window without taking the keyboard.
    pub fn show(&self) {
        if let Some(native) = self.native() {
            native.set_visible(true);
        }
    }

    /// Hides the window; it keeps its state and can be shown again.
    pub fn hide(&self) {
        if let Some(native) = self.native() {
            native.set_visible(false);
        }
    }

    /// Shows the window, brings it forward and gives it the keyboard (a
    /// `.panel` on macOS does this without bringing the app forward).
    pub fn activate(&self) {
        self.invalidate();
        if let Some(native) = self.native() {
            native.activate();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.native().is_some_and(|native| native.is_visible())
    }

    /// Minimized to the Dock / taskbar (still open, unlike hidden).
    pub fn is_minimized(&self) -> bool {
        self.native().is_some_and(|native| native.is_minimized())
    }

    /// Has the keyboard focus.
    pub fn is_active(&self) -> bool {
        self.native().is_some_and(|native| native.is_active())
    }

    /// Moves the window to a spot of the work area of the screen under the
    /// pointer, `margin` points from its edges.
    pub fn place(&self, place: Place, margin: f32) {
        if let Some(native) = self.native() {
            native.place(place, margin);
        }
    }

    /// Focuses the node with `key` in this window after its next layout
    /// (`ui::focus` from an app event — a hotkey, the tray — has no window).
    pub fn focus(&self, key: &str) {
        crate::ui::queue_command_for(Some(self.owner()), CommandKind::Focus, key);
        self.invalidate();
    }

    /// Closes a window opened with [`open_window`] (not the main window: quit
    /// instead).
    ///
    /// The native close is requested now but the runtime and its native window
    /// are freed by [`crate::runtime::window::process_pending_closes`] on the
    /// next event-loop turn (a window may not destroy itself from inside its
    /// own event callback).
    pub fn close(&self) {
        if self.is_main() {
            return;
        }
        match self.native() {
            Some(native) => native.close(),
            None => runtime_window::request_close(self.owner()),
        }
    }

    /// The runtime of this window (tests need to inspect the tree / state).
    pub fn runtime(&self) -> Rc<RefCell<Runtime>> {
        Rc::clone(&self.core)
    }
}

impl Clone for Window {
    /// A second handle to the same window.
    fn clone(&self) -> Window {
        Window {
            core: Rc::clone(&self.core),
            native: self.native.clone(),
        }
    }
}

impl std::fmt::Debug for Window {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("desktop::Window")
            .field("visible", &self.is_visible())
            .finish()
    }
}

/// Opens another window of the running app that renders `hooks` with the same
/// application state. Call it on the UI thread, e.g. from an action or the
/// first `render`; with `.window.hidden = true` it opens hidden.
pub fn open_window(hooks: WindowHooks, options: OpenOptions) -> Result<Window, PlatformError> {
    let app = MAIN
        .with(|main| main.borrow().as_ref().map(|state| state.app.clone()))
        .ok_or(PlatformError::UnsupportedPlatform)?;
    let mut runtime = Runtime::init_window(app, hooks, options.run_options());
    // The ZANT_FRAME_STATS labels of extra windows are
    // ":2", ":3", … (the main window enabled the layout statistics)
    if crate::ui::app::env_flag("ZANT_FRAME_STATS") {
        let label = runtime_window::next_frame_stats_label();
        runtime.frame_stats = Some(crate::runtime::debug::FrameStats::for_window(label));
    }
    let core = Rc::new(RefCell::new(runtime));
    let native = Rc::new(NativeWindow::create(
        &options.title,
        options.window,
        RuntimeHost::boxed(Rc::clone(&core)),
    )?);
    core.borrow_mut().attach_window(Rc::clone(&native));
    runtime_window::register(&core);
    core.borrow_mut().invalidate();
    Ok(Window {
        core,
        native: Rc::downgrade(&native),
    })
}

/// Rebuilds all windows (after changing state outside an action, e.g. in a
/// timer or after a background result; `ui::post_wake` does it from threads).
pub fn invalidate_all() {
    runtime_window::invalidate_all();
}

/// Ends the app now (all windows), without asking `on_close_request` again.
pub fn quit() {
    platform::quit();
}

/// The app's only running copy per user: true for the first process with
/// `app_id` (the bundle id); a later launch brings the running one forward (it
/// gets `AppEvent::Reopen`) and gets false — return from `main` then. Call it
/// before `ui::app::run`.
///
/// ```ignore
/// if !zant::desktop::single_instance(cfg.bundle_id) {
///     return Ok(());
/// }
/// ```
pub fn single_instance(app_id: &str) -> bool {
    platform::single_instance(app_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::App;
    use crate::ui::builders::{column, text};
    use crate::ui::element::Node;
    use crate::ui::options::{BoxOptions, TextOptions};

    struct Bare;

    impl App for Bare {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![text("main", TextOptions::default())],
                ..Default::default()
            })
        }
    }

    fn headless_runtime() -> Rc<RefCell<Runtime>> {
        Rc::new(RefCell::new(Runtime::init(Bare, RunOptions::default())))
    }

    #[test]
    fn there_is_no_main_window_before_run_starts_it() {
        set_main_runtime(None);
        assert!(main_window().is_none());
    }

    #[test]
    fn the_main_window_needs_its_native_window() {
        let core = headless_runtime();
        set_main_runtime(Some(Rc::clone(&core)));
        // the runtime has no native window yet (headless), so there is no main
        // window handle, as before `run`
        assert!(main_window().is_none());
        set_main_runtime(None);
        assert!(main_window().is_none());
    }

    #[test]
    fn open_options_have_the_documented_defaults() {
        let options = OpenOptions::default();
        assert_eq!(options.title, "Zant UI");
        assert_eq!(options.window.width, 560.0);
        assert_eq!(options.theme, Theme::default());
        let run = options.run_options();
        assert_eq!(run.title, "Zant UI");
        assert_eq!(run.window.height, 420.0);
        assert_eq!(run.theme, Theme::default());
    }

    #[test]
    fn opening_a_window_without_a_main_window_fails_cleanly() {
        set_main_runtime(None);
        let hooks = WindowHooks::new(|| column(BoxOptions::default()));
        let error = open_window(hooks, OpenOptions::default()).expect_err("no app window");
        assert!(matches!(error, PlatformError::UnsupportedPlatform));
    }

    #[test]
    fn quit_and_single_instance_delegate_to_the_platform() {
        let _guard = crate::platform::platform::native_test_lock();
        // no native state: the platform helpers must stay quiet
        let _ = single_instance("com.example.zant");
    }
}
