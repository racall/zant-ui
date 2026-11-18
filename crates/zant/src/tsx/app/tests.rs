//! Unit tests.

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
    assert!(!config.remember);
    assert!(config.bundle_id.is_empty());
}

#[test]
fn remembered_windows_use_the_bundle_id_only_when_enabled() {
    for (remember, bundle_id, expected) in [
        (false, "com.example.app", None),
        (true, "", None),
        (true, "com.example.app", Some("com.example.app")),
    ] {
        let (_, options) = prepare::<Probe>(Config {
            bundle_id: bundle_id.into(),
            remember,
            ..Default::default()
        });
        assert_eq!(options.autosave_name.as_deref(), expected);
    }
    timers::set_running(false);
}
