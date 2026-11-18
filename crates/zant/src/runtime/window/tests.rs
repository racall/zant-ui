//! Unit tests.

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
