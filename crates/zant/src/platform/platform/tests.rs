//! Tests and shared test lock for the native platform wrappers.

use super::app::app_event_trampoline;
use super::window_options::parse_window_size;
use super::*;

/// Serializes the tests that call native services which macOS does not expect
/// from several threads at once (the pasteboard, the status bar, …). The
/// framework itself calls them on the UI thread only; the test harness runs
/// every test on its own thread.
#[cfg(test)]
static NATIVE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquires [`NATIVE_TEST_LOCK`] (a poisoned lock is reused: one failing test
/// must not break the others).
#[cfg(test)]
pub(crate) fn native_test_lock() -> std::sync::MutexGuard<'static, ()> {
    NATIVE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn titlebar_insets_reserve_the_system_controls() {
    assert_eq!(
        titlebar_insets(WindowStyle::Native),
        TitlebarInsets::default()
    );
    let hidden = titlebar_insets(WindowStyle::HiddenTitlebar);
    if IS_MACOS {
        assert_eq!(hidden.left, 84.0, "traffic lights on the left");
        assert_eq!(hidden.right, 0.0);
    } else if IS_WINDOWS {
        assert_eq!(hidden.left, 0.0);
        assert_eq!(hidden.right, CAPTION_BUTTON_WIDTH * 3.0);
    }
}

#[test]
fn window_size_override_parses_and_ignores_invalid_values() {
    assert_eq!(parse_window_size("900x640"), Some((900.0, 640.0)));
    assert_eq!(parse_window_size("0x640"), None);
    assert_eq!(parse_window_size("900x"), None);
    assert_eq!(parse_window_size("900"), None);
    assert_eq!(parse_window_size("abcxdef"), None);
}

#[test]
fn monotonic_clock_advances() {
    let first = monotonic_us();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let second = monotonic_us();
    assert!(second > first);
    assert_eq!(monotonic_ms(), monotonic_us() / 1000);
}

#[test]
fn the_app_event_trampoline_forwards_to_the_installed_handler() {
    #[derive(Default)]
    struct Recorder {
        events: Vec<(i32, i32)>,
    }
    impl AppEventHandler for Recorder {
        fn event(&mut self, event: &AppEvent) {
            self.events.push((event.type_, event.id));
        }
    }
    struct Shared(std::rc::Rc<std::cell::RefCell<Recorder>>);
    impl AppEventHandler for Shared {
        fn event(&mut self, event: &AppEvent) {
            self.0.borrow_mut().event(event);
        }
    }
    let recorder = std::rc::Rc::new(std::cell::RefCell::new(Recorder::default()));
    install_app_event_handler(Shared(std::rc::Rc::clone(&recorder)));
    let event = AppEvent {
        type_: app_event_type::TRAY_MENU,
        id: 3,
    };
    // SAFETY: the trampoline only reads the event for the call, exactly as
    // the native side calls it.
    unsafe { app_event_trampoline(std::ptr::null_mut(), &event) };
    assert_eq!(
        recorder.borrow().events,
        vec![(app_event_type::TRAY_MENU, 3)]
    );
    clear_app_event_handler();
    // after clearing, events are dropped
    // SAFETY: same as above.
    unsafe { app_event_trampoline(std::ptr::null_mut(), &event) };
    assert_eq!(recorder.borrow().events.len(), 1);
}

#[test]
fn unsupported_platform_helpers_are_quiet() {
    let _guard = native_test_lock();
    // Every helper must be safe to call before a window exists.
    let _ = reduce_motion();
    let _ = system_dark_mode();
    let _ = launched_at_login();
    let _ = clipboard_change_count();
    let _ = tray_icon_pixels();
    let _ = device_name();
    let _ = downloads_dir();
}
