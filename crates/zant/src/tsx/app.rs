//! The entry of a TS + TSX app: the root component, the
//! runtime shell around it and the theme rules.
//!
//! The generated root struct is the runtime's state and a [`Shell`] around it
//! runs the hooks (`on_wake` fires the timers, app events reach the tray /
//! hotkeys, the theme follows the system unless `ZANT_THEME` is set). The
//! root implements the `ui::App` trait:
//!
//! ```ignore
//! // generated src/app.rs
//! pub struct App { count: u32 }
//! impl Default for App { fn default() -> Self { App { count: 0 } } }
//! impl zant::ui::App for App {
//!     fn render(&mut self) -> zant::ui::Node { … }
//!     // optional hooks: on_key / on_close_request / on_wake / on_app_event /
//!     // on_start / on_exit
//! }
//!
//! // generated src/main.rs
//! fn main() -> zant::Result<()> {
//!     zant::tsx::app::run::<crate::app::App>(zant::tsx::app::Config {
//!         title: "Counter".into(),
//!         ..Default::default()
//!     })
//! }
//! ```
//!
//! `useKeyDown` gets the DOM-like [`KeyEvent`]: generated `on_key` converts the
//! runtime event first (`let e = tsx::app::key_event(event);`).
//!
//! `ZANT_THEME=light|dark` fixes the theme (screenshots, tests).

use std::any::Any;
use std::cell::RefCell;

use crate::core::events::{Key, KeyEvent as RuntimeKeyEvent};
use crate::desktop;
use crate::desktop::AppEvent;
use crate::platform::platform::WindowOptions;
use crate::theme::tokens::{self, Theme};
use crate::tsx::timers;
use crate::ui::{self, ActionCx, Node};

/// A key press as TSX sees it (`useKeyDown((e) => …)`): `key` is "Escape",
/// "Enter", "Tab", "Backspace", "Delete", "ArrowUp" … (like the DOM's
/// `KeyboardEvent.key`), "F1" … "F12", or the character typed ("f", "/").
/// `r#mod` is the platform's command key: ⌘ on macOS, Ctrl on Windows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: crate::Str,
    pub r#mod: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub meta: bool,
    pub repeat: bool,
}

/// Converts a runtime key event to the DOM-like TSX shape.
pub fn key_event(event: &RuntimeKeyEvent) -> KeyEvent {
    KeyEvent {
        key: key_name(&event.key),
        r#mod: event.modifiers.primary(),
        shift: event.modifiers.shift,
        alt: event.modifiers.alt,
        ctrl: event.modifiers.control,
        meta: event.modifiers.meta,
        repeat: event.repeat,
    }
}

fn key_name(key: &Key) -> crate::Str {
    use crate::Str;
    match key {
        Key::Character(character) => Str::copy(&character.to_string()),
        Key::Left => Str::Static("ArrowLeft"),
        Key::Right => Str::Static("ArrowRight"),
        Key::Up => Str::Static("ArrowUp"),
        Key::Down => Str::Static("ArrowDown"),
        Key::Home => Str::Static("Home"),
        Key::End => Str::Static("End"),
        Key::Backspace => Str::Static("Backspace"),
        Key::Delete => Str::Static("Delete"),
        Key::Escape => Str::Static("Escape"),
        Key::Tab => Str::Static("Tab"),
        Key::Enter => Str::Static("Enter"),
        Key::PageUp => Str::Static("PageUp"),
        Key::PageDown => Str::Static("PageDown"),
        Key::Function(number) => Str::copy(&format!("F{number}")),
    }
}

thread_local! {
    /// `ZANT_THEME` as read by [`run`], for the app's other windows.
    static FORCED: RefCell<Option<Theme>> = const { RefCell::new(None) };
}

/// Sets the forced theme (tests / extra windows); [`run`] reads `ZANT_THEME`.
pub(crate) fn set_forced_theme(theme: Option<Theme>) {
    FORCED.with(|forced| *forced.borrow_mut() = theme);
}

/// The theme the app uses when it does not override it: the system's light /
/// dark mode.
pub fn system_theme() -> Theme {
    if desktop::system_dark_mode() {
        tokens::ant_dark()
    } else {
        tokens::ant_light()
    }
}

/// The theme every window of the app uses now (forced theme, else the
/// system's).
pub fn current_theme() -> Theme {
    FORCED
        .with(|forced| *forced.borrow())
        .unwrap_or_else(system_theme)
}

/// `ZANT_THEME=light|dark` fixes the theme.
fn forced_theme() -> Option<Theme> {
    let value = crate::platform::platform::getenv("ZANT_THEME")?;
    match value.as_str() {
        "dark" => Some(tokens::ant_dark()),
        "light" => Some(tokens::ant_light()),
        _ => None,
    }
}

/// The runtime's view of a TSX app: the generated root component implements
/// [`crate::ui::App`]; this shell
/// forwards its hooks and runs what has to happen around them:
///
/// - `ui::App::on_wake` fires the due timers with the root state as the
///   callback context (a root timer action is `ui::action(|s: &mut App| …)`)
///   before the app's own wake hook runs;
/// - `ui::App::on_app_event` dispatches tray clicks / menu items and global
///   hotkeys to `tsx::desktop` (their actions also get the root state), then
///   calls the app's handler and asks every window to rebuild
///   (it returns true unconditionally);
/// - `ui::App::on_start` installs the clipboard watcher when
///   [`Config::watch_clipboard`] is set;
/// - the theme follows the system light / dark mode unless `ZANT_THEME` fixes
///   it (or the app overrides it through `ui::App::theme`).
///
/// `useKeyDown` callbacks see the DOM-like [`KeyEvent`]: generated code calls
/// [`key_event`] on the runtime event at the top of its `on_key`.
pub struct Shell<A: crate::ui::App> {
    root: A,
    forced: Option<Theme>,
    watch_clipboard: bool,
}

impl<A: crate::ui::App> Shell<A> {
    pub fn new(root: A) -> Shell<A> {
        Shell {
            root,
            forced: None,
            watch_clipboard: false,
        }
    }

    pub fn with_forced(mut self, forced: Option<Theme>) -> Shell<A> {
        self.forced = forced;
        self
    }

    /// Watches the clipboard while the app runs (`useClipboardChange`;
    /// installed on start when the hook is declared).
    pub fn with_clipboard_watch(mut self, watch: bool) -> Shell<A> {
        self.watch_clipboard = watch;
        self
    }

    /// The root component's state.
    pub fn root(&self) -> &A {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut A {
        &mut self.root
    }

    /// Runs `f` with a callback context over the root state (tray / hotkey /
    /// timer callbacks can reach `cx.state::<A>()`).
    fn with_action_cx(&mut self, f: impl FnOnce(&mut ActionCx<'_>)) {
        let root: &mut dyn Any = &mut self.root;
        let mut cx = ActionCx::new(root);
        f(&mut cx);
    }
}

impl<A: crate::ui::App> crate::ui::App for Shell<A> {
    /// Generated TSX actions target the app's root component, not the shell.
    fn action_root(&mut self) -> Option<&mut dyn Any> {
        Some(&mut self.root)
    }

    fn render(&mut self) -> Node {
        self.root.render()
    }

    fn theme(&self) -> Option<Theme> {
        self.forced
            .or_else(|| self.root.theme())
            .or_else(|| Some(system_theme()))
    }

    fn on_key(&mut self, event: &RuntimeKeyEvent) -> bool {
        self.root.on_key(event)
    }

    fn on_close_request(&mut self) -> bool {
        self.root.on_close_request()
    }

    fn on_quit_request(&mut self) -> bool {
        self.root.on_quit_request()
    }

    fn on_start(&mut self) {
        if self.watch_clipboard {
            desktop::watch_clipboard(true);
        }
        self.root.on_start();
    }

    /// Runs the due timers with the root state as the callback context, then
    /// the app's `useWake` hook.
    fn on_wake(&mut self) {
        self.with_action_cx(|cx| {
            timers::fire_due(cx);
        });
        self.root.on_wake();
    }

    /// Tray / hotkey events go to the hooks first, then the app's handler;
    /// every window rebuilds afterwards (always returns true).
    ///
    /// A Dock / reopen event also shows the main window; the app's `useReopen`
    /// handler, if any, runs too (the two do not conflict).
    fn on_app_event(&mut self, event: &AppEvent) -> bool {
        self.with_action_cx(|cx| {
            super::desktop::handle_app_event(cx, event);
        });
        if *event == AppEvent::Reopen
            && let Some(window) = desktop::main_window()
        {
            window.activate();
        }
        self.root.on_app_event(event);
        true
    }

    fn on_exit(&mut self) {
        self.root.on_exit();
    }
}

/// The app configuration of a generated TSX entry (generated from
/// `app.config.json` by `zant sync`).
#[derive(Clone, Debug)]
pub struct Config {
    pub title: String,
    pub window: WindowOptions,
    /// The app's bundle id (login items, single instance, data dir).
    pub bundle_id: String,
    /// `app.config.json` `single_instance`: a second launch comes forward.
    pub single_instance: bool,
    /// The app uses `useClipboardChange`: watch the clipboard while it runs.
    pub watch_clipboard: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            title: String::from("Zant UI"),
            window: WindowOptions::default(),
            bundle_id: String::new(),
            single_instance: false,
            watch_clipboard: false,
        }
    }
}

/// Runs the app until its last window closes.
pub fn run<A: crate::ui::App + Default>(config: Config) -> crate::Result<()> {
    // already running (maybe hidden in the tray): it comes forward instead
    if config.single_instance && !desktop::single_instance(&config.bundle_id) {
        return Ok(());
    }
    crate::tsx::platform::set_app_id(&config.bundle_id);
    let forced = forced_theme();
    set_forced_theme(forced);
    timers::set_running(true);
    let options = ui::RunOptions {
        title: config.title,
        window: config.window,
        theme: forced.unwrap_or_else(system_theme),
    };
    let shell = Shell::new(A::default())
        .with_forced(forced)
        .with_clipboard_watch(config.watch_clipboard);
    let result = ui::run(shell, options);
    timers::set_running(false);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::App as _;
    use crate::ui::builders::column;
    use crate::ui::options::BoxOptions;

    #[derive(Default)]
    struct Probe {
        renders: u32,
        keys: u32,
        woken: u32,
        clipped: u32,
        exited: u32,
    }

    impl crate::ui::App for Probe {
        fn render(&mut self) -> Node {
            self.renders += 1;
            column(BoxOptions::default())
        }

        fn on_key(&mut self, event: &RuntimeKeyEvent) -> bool {
            self.keys += 1;
            matches!(event.key, Key::Escape)
        }

        fn on_wake(&mut self) {
            self.woken += 1;
        }

        fn on_app_event(&mut self, event: &AppEvent) -> bool {
            if *event == AppEvent::Clipboard {
                self.clipped += 1;
            }
            true
        }

        fn on_exit(&mut self) {
            self.exited += 1;
        }
    }

    #[test]
    fn the_shell_renders_its_root_and_follows_a_forced_theme() {
        set_forced_theme(None);
        let mut shell = Shell::new(Probe::default()).with_forced(Some(tokens::ant_dark()));
        let _ = shell.render();
        assert_eq!(shell.root().renders, 1);
        assert_eq!(
            shell.theme().expect("a theme").color_bg_base,
            tokens::ant_dark().color_bg_base
        );
        // without a forced theme the shell follows the system
        let plain = Shell::new(Probe::default());
        assert_eq!(
            plain.theme().expect("a theme").color_bg_base,
            system_theme().color_bg_base
        );
    }

    #[test]
    fn key_events_read_like_the_doms() {
        let escape = key_event(&RuntimeKeyEvent {
            key: Key::Escape,
            modifiers: Default::default(),
            repeat: false,
        });
        assert_eq!(escape.key.as_str(), "Escape");
        let character = key_event(&RuntimeKeyEvent {
            key: Key::Character('f'),
            modifiers: crate::core::events::Modifiers {
                shift: true,
                ..Default::default()
            },
            repeat: false,
        });
        assert_eq!(character.key.as_str(), "f");
        assert!(character.shift && !character.alt);
        assert_eq!(
            key_event(&RuntimeKeyEvent {
                key: Key::Function(5),
                modifiers: Default::default(),
                repeat: false,
            })
            .key
            .as_str(),
            "F5"
        );
        assert_eq!(
            key_event(&RuntimeKeyEvent {
                key: Key::Left,
                modifiers: Default::default(),
                repeat: false,
            })
            .key
            .as_str(),
            "ArrowLeft"
        );
        let mut shell = Shell::new(Probe::default());
        assert!(shell.on_key(&RuntimeKeyEvent {
            key: Key::Escape,
            modifiers: Default::default(),
            repeat: false,
        }));
        assert_eq!(shell.root().keys, 1);
    }

    #[test]
    fn the_shell_forwards_the_wake_and_exit_hooks() {
        set_forced_theme(None);
        let mut shell = Shell::new(Probe::default());
        shell.on_wake();
        assert_eq!(shell.root().woken, 1);
        shell.on_exit();
        assert_eq!(shell.root().exited, 1);
        assert!(shell.on_app_event(&AppEvent::Clipboard));
        assert_eq!(shell.root().clipped, 1);
        // tray / hotkey events have no app state of their own but still
        // rebuild
        assert!(shell.on_app_event(&AppEvent::TrayClick));
        assert!(shell.on_app_event(&AppEvent::Hotkey(9)));
        assert!(shell.on_close_request());
        assert!(shell.on_quit_request());
        let forced = tokens::ant_dark();
        set_forced_theme(Some(forced));
        assert_eq!(current_theme().color_bg_base, forced.color_bg_base);
        set_forced_theme(None);
    }

    #[test]
    fn config_defaults_are_pinned() {
        let config = Config::default();
        assert_eq!(config.title, "Zant UI");
        assert_eq!(config.window.width, 560.0);
        assert_eq!(config.window.height, 420.0);
        assert!(!config.single_instance);
        assert!(config.bundle_id.is_empty());
    }
}
