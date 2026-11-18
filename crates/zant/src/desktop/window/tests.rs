//! Unit tests.

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
