//! Application entry point: the `App` trait, `RunOptions` and `run()`.
//!
//! Port of `src/launch.zig` plus the `Config` of `src/ui_runtime.zig:25-48`.
//! Zig detected the optional hooks (`theme`, `onKey`, `onStart`, …) with
//! `@hasDecl` on the state type; Rust's `App` trait replaces that with default
//! methods (plan.md §4).
//!
//! ```ignore
//! struct Counter { count: u32 }
//! impl zant::ui::App for Counter {
//!     fn render(&mut self) -> zant::ui::Node {
//!         zant::ui::column(Default::default()) // …
//!     }
//! }
//! zant::ui::run(Counter { count: 0 }, zant::ui::RunOptions::default())?;
//! ```
//!
//! The trait requires `Any` so callbacks (`ui::action(|s: &mut Counter| …)`) can
//! resolve the application state through `ActionCx::state`; concrete `App`
//! values are `'static` by definition, so no `impl` ever writes it out.
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::Error;
use crate::core::events::KeyEvent;
use crate::motion::motion;
use crate::platform::platform::{AppEvent, Window, WindowOptions};
use crate::theme::tokens::Theme;

use super::element::Node;

/// What `launch.zig`'s `Options` carried: the window title, its platform
/// options and the initial theme.
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Window title (Zig `[:0]const u8 = "Zant UI"`).
    pub title: String,
    /// Platform window options passed to `Window::create`.
    pub window: WindowOptions,
    /// Theme the first frame builds with; `App::theme` may override it per
    /// frame (dark mode).
    pub theme: Theme,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            title: String::from("Zant UI"),
            window: WindowOptions::default(),
            theme: Theme::default(),
        }
    }
}

impl RunOptions {
    /// Options with `title` and the platform / theme defaults.
    pub fn new(title: impl Into<String>) -> RunOptions {
        RunOptions {
            title: title.into(),
            ..RunOptions::default()
        }
    }
}

/// The application: state plus a `render` function and optional hooks.
///
/// Every hook has the same default as Zig's missing declaration: no dark-mode
/// theme provider, no key shortcut handler, the window may close, nothing runs
/// on start / wake / exit.
pub trait App: std::any::Any {
    /// Builds the declarative tree of this frame. Called before every rebuild;
    /// it must return a container (`column` / `row` / `card`).
    fn render(&mut self) -> Node;

    /// Theme of the next frame (e.g. return `Some(ant_dark())`); `None` keeps
    /// the runtime's current theme (`RunOptions::theme` initially).
    fn theme(&self) -> Option<Theme> {
        None
    }

    /// Application shortcuts (⌘S, Esc, …), seen before the focused widget.
    /// Return true to consume the key.
    fn on_key(&mut self, _event: &KeyEvent) -> bool {
        false
    }

    /// Asked before the window closes (close button, ⌘W). Return false to keep
    /// it open, e.g. to ask about unsaved changes.
    fn on_close_request(&mut self) -> bool {
        true
    }

    /// Asked when the user quits the app (⌘Q, Dock menu); defaults to
    /// `on_close_request` (Zig `Config.quit_fn` defaults to `close_fn`).
    fn on_quit_request(&mut self) -> bool {
        self.on_close_request()
    }

    /// Once, after the window exists and before the first frame: open panels,
    /// set the tray icon, start background work.
    fn on_start(&mut self) {}

    /// On the UI thread after `ui::post_wake()` from a background thread: take
    /// the results it published (all windows rebuild afterwards).
    fn on_wake(&mut self) {}

    /// Tray / status bar clicks, global hotkeys, Dock clicks (desktop events,
    /// WP-2.8). Return true when state changed.
    fn on_app_event(&mut self, _event: &AppEvent) -> bool {
        false
    }

    /// The app is about to end (quit, main window closed): stop background
    /// work and save. Wiring the exit hook belongs to WP-2.8 (`desktop`).
    fn on_exit(&mut self) {}
}

/// Runs the app: creates the window, dispatches events, rebuilds and paints
/// until the native event loop ends.
///
/// Port of `launch.zig:run` + `ui_runtime.Runtime.run`: window creation, the
/// `ZANT_*` debug environment, `on_start`, an optional snapshot and the frame
/// loop all live here.
pub fn run(app: impl App, options: RunOptions) -> Result<(), Error> {
    if !crate::platform::platform::SUPPORTED {
        return Err(Error::Message(String::from("unsupported platform")));
    }
    // motion off for Reduce Motion, ZANT_NO_MOTION=1 and snapshots (decided
    // here, not in Runtime::init, so tests keep control of it)
    let no_motion = env_flag("ZANT_NO_MOTION");
    let snapshot = crate::platform::platform::getenv("ZANT_SNAPSHOT");
    motion::set_enabled(
        !(no_motion || snapshot.is_some() || crate::platform::platform::reduce_motion()),
    );

    let core = Rc::new(RefCell::new(crate::runtime::Runtime::init(
        app,
        options.clone(),
    )));
    let window = match Window::create(
        &options.title,
        options.window,
        crate::runtime::host::RuntimeHost::boxed(Rc::clone(&core)),
    ) {
        Ok(window) => Rc::new(window),
        Err(_) => return Err(Error::Message(String::from("window creation failed"))),
    };
    core.borrow_mut().attach_window(Rc::clone(&window));
    eprintln!("Zant UI app — renderer: {}", window.renderer_name());
    if let Some(path) = crate::platform::platform::getenv("ZANT_LAYOUT_FILE") {
        core.borrow_mut().layout_file = Some(path);
    }
    if env_flag("ZANT_FRAME_STATS") {
        core.borrow_mut().enable_frame_stats();
    }
    core.borrow_mut().app.on_start();
    if let Some(path) = snapshot {
        window.schedule_snapshot(&path, 600);
    }
    window.run();
    // Break the Rc cycle Runtime -> Window -> Host -> Rc<RefCell<Runtime>> so
    // both are freed when this function returns.
    core.borrow_mut().detach_window();
    Ok(())
}

/// `ZANT_*` switch with Zig's rule: the variable must start with `1`
/// (`v.len > 0 and v[0] == '1'`).
pub(crate) fn env_flag(name: &str) -> bool {
    env_flag_value(crate::platform::platform::getenv(name).as_deref())
}

/// The value rule of [`env_flag`], split out so it is testable without
/// mutating the process environment.
pub(crate) fn env_flag_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.starts_with('1'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::builders::{column, text};
    use crate::ui::options::{BoxOptions, TextOptions};

    struct Smoke;

    impl App for Smoke {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![text("hello", TextOptions::default())],
                ..Default::default()
            })
        }
    }

    #[test]
    fn app_defaults_match_the_zig_hooks() {
        struct Bare;
        impl App for Bare {
            fn render(&mut self) -> Node {
                Node::default()
            }
        }
        let mut app = Bare;
        assert!(app.theme().is_none());
        assert!(!app.on_key(&KeyEvent {
            key: crate::core::events::Key::Enter,
            modifiers: Default::default(),
            repeat: false
        }));
        assert!(app.on_close_request());
        assert!(app.on_quit_request(), "quit defaults to close");
        app.on_start();
        app.on_wake();
        app.on_exit();
        // a concrete hook may override the default
        struct Veto;
        impl App for Veto {
            fn render(&mut self) -> Node {
                Node::default()
            }
            fn on_close_request(&mut self) -> bool {
                false
            }
        }
        let mut veto = Veto;
        assert!(!veto.on_quit_request(), "quit still defers to close");
    }

    #[test]
    fn run_options_defaults_match_launch_zig() {
        let options = RunOptions::default();
        assert_eq!(options.title, "Zant UI");
        assert_eq!(options.window.width, 560.0);
        assert_eq!(options.window.height, 420.0);
        assert_eq!(options.theme, Theme::default());
        let named = RunOptions::new("Demo");
        assert_eq!(named.title, "Demo");
    }

    #[test]
    fn an_app_can_be_boxed_and_rendered() {
        let mut boxed: Box<dyn App> = Box::new(Smoke);
        assert_eq!(boxed.render().element().children.len(), 1);
    }

    #[test]
    fn env_flags_require_a_leading_one() {
        assert!(!env_flag_value(None));
        assert!(env_flag_value(Some("1")));
        assert!(env_flag_value(Some("1abc")));
        assert!(!env_flag_value(Some("0")));
        assert!(!env_flag_value(Some("")));
        assert!(!env_flag_value(Some("true")));
    }
}
