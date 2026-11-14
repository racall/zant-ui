//! App-level events: tray clicks and menu items, global hotkeys, Dock clicks,
//! clipboard changes and the exit notification.
//!
//! The native side delivers these through one C callback
//! (`zant_app_set_handler`). `platform` owns the safe trampoline; this module
//! installs a handler that forwards them to the [`App`] hooks: `on_app_event`
//! (true = state changed, every window rebuilds) and `on_exit`.
#![forbid(unsafe_code)]

use crate::platform::platform;
use crate::platform::platform::app_event_type;
use crate::runtime::window::SharedApp;

/// One app-level event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEvent {
    /// Windows: left click on the tray icon (the menu opens on right click).
    TrayClick,
    /// A tray / status bar menu item, by id.
    TrayMenu(u32),
    /// A registered hotkey, by id.
    Hotkey(u32),
    /// macOS: the Dock icon was clicked (show the hidden main window).
    Reopen,
    /// The clipboard changed (any app, this one included) — see
    /// [`crate::desktop::watch_clipboard`].
    Clipboard,
}

/// The event id, with negative native ids clamped to zero.
fn event_id(event: &platform::AppEvent) -> u32 {
    u32::try_from(event.id.max(0)).unwrap_or(0)
}

/// Translates a native event; `None` for the types the app hooks do not see
/// (the exit notification is handled separately).
pub(crate) fn translate(event: &platform::AppEvent) -> Option<AppEvent> {
    match event.type_ {
        app_event_type::TRAY_CLICK => Some(AppEvent::TrayClick),
        app_event_type::TRAY_MENU => Some(AppEvent::TrayMenu(event_id(event))),
        app_event_type::HOTKEY => Some(AppEvent::Hotkey(event_id(event))),
        app_event_type::REOPEN => Some(AppEvent::Reopen),
        app_event_type::CLIPBOARD => Some(AppEvent::Clipboard),
        _ => None,
    }
}

/// Routes the native app events to the application hooks (installed by
/// `ui::app::run`).
pub(crate) struct DesktopAppHandler {
    app: SharedApp,
}

impl platform::AppEventHandler for DesktopAppHandler {
    fn event(&mut self, event: &platform::AppEvent) {
        if event.type_ == app_event_type::EXIT {
            // the last chance to stop workers and save; the process exits after
            self.app.borrow_mut().on_exit();
            return;
        }
        let Some(event) = translate(event) else {
            return;
        };
        if self.app.borrow_mut().on_app_event(&event) {
            crate::runtime::window::invalidate_all();
        }
    }
}

/// Installs the app-event handler of `ui::app::run`.
pub(crate) fn install(app: SharedApp) {
    platform::install_app_event_handler(DesktopAppHandler { app });
}

/// Stops delivering app events (the app ended).
pub(crate) fn clear() {
    platform::clear_app_event_handler();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::platform::AppEventHandler;
    use crate::runtime::Runtime;
    use crate::runtime::window as runtime_window;
    use crate::ui::RunOptions;
    use crate::ui::app::App;
    use crate::ui::builders::column;
    use crate::ui::element::Node;
    use crate::ui::options::BoxOptions;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn raw(type_: i32, id: i32) -> platform::AppEvent {
        platform::AppEvent { type_, id }
    }

    #[test]
    fn native_event_types_map_to_app_events() {
        assert_eq!(
            translate(&raw(app_event_type::TRAY_CLICK, 0)),
            Some(AppEvent::TrayClick)
        );
        assert_eq!(
            translate(&raw(app_event_type::TRAY_MENU, 3)),
            Some(AppEvent::TrayMenu(3))
        );
        assert_eq!(
            translate(&raw(app_event_type::HOTKEY, 1)),
            Some(AppEvent::Hotkey(1))
        );
        assert_eq!(
            translate(&raw(app_event_type::REOPEN, 0)),
            Some(AppEvent::Reopen)
        );
        assert_eq!(
            translate(&raw(app_event_type::CLIPBOARD, 0)),
            Some(AppEvent::Clipboard)
        );
        assert_eq!(translate(&raw(app_event_type::EXIT, 0)), None);
        assert_eq!(translate(&raw(99, 0)), None);
        // negative ids are clamped to zero
        assert_eq!(
            translate(&raw(app_event_type::TRAY_MENU, -4)),
            Some(AppEvent::TrayMenu(0))
        );
    }

    #[derive(Default)]
    struct Recorder {
        events: Vec<AppEvent>,
        exits: u32,
    }

    impl App for Recorder {
        fn render(&mut self) -> Node {
            column(BoxOptions::default())
        }
        fn on_app_event(&mut self, event: &AppEvent) -> bool {
            self.events.push(*event);
            true
        }
        fn on_exit(&mut self) {
            self.exits += 1;
        }
    }

    #[test]
    fn app_events_reach_the_hooks_and_rebuild_the_windows() {
        let app = SharedApp::new(Recorder::default());
        let core = Rc::new(RefCell::new(Runtime::init_shared(
            app.clone(),
            RunOptions::default(),
        )));
        runtime_window::register(&core);
        let mut handler = DesktopAppHandler { app: app.clone() };
        handler.event(&raw(app_event_type::TRAY_MENU, 2));
        assert_eq!(
            core.borrow().app_state_mut::<Recorder>().events,
            vec![AppEvent::TrayMenu(2)]
        );
        assert!(
            core.borrow().needs_build,
            "a changed app event rebuilds every window"
        );
        handler.event(&raw(app_event_type::EXIT, 0));
        assert_eq!(
            core.borrow().app_state_mut::<Recorder>().exits,
            1,
            "the exit hook ran once"
        );
        runtime_window::clear();
    }
}
