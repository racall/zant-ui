//! App-generation teardown. Keep this list in sync with framework statics/TLS.
//!
//! Audit of intentionally retained state (all lives in the framework image):
//! - image/timer/hotkey NEXT_ID: numeric identities, kept monotonic;
//! - platform EPOCH, motion ENABLED, TSX FORCED/SYSTEM/RANDOM_STATE: time,
//!   flags, copied theme tokens and RNG integers, with no app references;
//! - layout IDS_POOL/ITEMS_POOL/FLOATS_POOL: numeric scratch buffers;
//! - renderer LAYOUT_STATS, markdown MEMO_BUILDS: numeric counters;
//! - markdown data_images ENTRIES: content hashes and owned native images,
//!   no asset addresses, app bytes, closures or app vtables;
//! - math transition/regex tables, DEFAULT_TOOLBAR, TSX_REGISTRY: immutable
//!   framework-owned data; test-only clocks/counters/locks are not in apps.
//!
//! The hot_entry API static belongs to the app image, not a framework registry.

/// Releases the framework's references to the stopped app version.
///
/// Call on the UI thread, outside all render / event callbacks, after stopping
/// the instance and before unloading its library. App-created windows close;
/// the native main window owned by the hot host stays open. App-owned threads
/// and globals remain the app's responsibility.
pub fn release_app() {
    crate::tsx::timers::set_running(false);
    crate::platform::platform::set_post_wake_window(std::ptr::null_mut());
    crate::desktop::clear_app_hooks();
    crate::desktop::set_main_runtime(None);
    crate::runtime::window::release_app();
    crate::tsx::native::clear();
    crate::ui::local_state::release_app();
    crate::ui::context::release_app();
    crate::tsx::desktop::release_app();
    crate::platform::platform::release_app();
    crate::tsx::timers::release_app();
    crate::tsx::task::release_app();
    crate::runtime::view_state::release_app();
    crate::ui::commands::release_app();
    crate::tsx::platform::release_app();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::platform;
    use crate::runtime::Runtime;
    use crate::runtime::view_state::{self, ListBuildGuard, Lists};
    use crate::runtime::window::{self, WindowHooks};
    use crate::{tsx, ui};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct App {
        held: Rc<()>,
        // The same cycle made by an app keeping a desktop::Window handle.
        panel: Option<Rc<RefCell<Runtime>>>,
    }

    fn action(held: Rc<()>) -> ui::Action {
        ui::Action::new(move |_| assert!(Rc::strong_count(&held) > 0))
    }

    impl ui::App for App {
        fn render(&mut self) -> ui::Node {
            ui::column(ui::BoxOptions {
                children: crate::nodes![ui::button(ui::ButtonOptions {
                    text: "held".into(),
                    on_click: Some(action(self.held.clone())),
                    ..Default::default()
                })],
                ..Default::default()
            })
        }
    }

    struct Handler(Rc<()>);
    impl platform::AppEventHandler for Handler {
        fn event(&mut self, _: &platform::AppEvent) {
            assert!(Rc::strong_count(&self.0) > 0);
        }
    }

    #[test]
    fn releases_all_app_registries_and_window_cycles() {
        let _native = platform::native_test_lock();
        tsx::desktop::set_system_enabled(false);
        release_app();
        let held = Rc::new(());
        let weak = Rc::downgrade(&held);
        assert!(tsx::native::register_named("release-test", held.clone()));
        tsx::timers::set_clock(Some(|| -1_000_000));
        tsx::timers::set_interval(100.0, action(held.clone()));
        tsx::desktop::update(tsx::desktop::TrayOptions {
            items: vec![tsx::desktop::TrayItem {
                title: "app menu".into(),
                on_select: Some(action(held.clone())),
                ..Default::default()
            }],
            on_click: Some(action(held.clone())),
            ..Default::default()
        });
        let mut hotkey = tsx::desktop::Hotkey::default();
        assert!(hotkey.update("ctrl+alt+k", action(held.clone())));
        platform::install_app_event_handler(Handler(held.clone()));
        ui::local_state(123, || held.clone());
        tsx::platform::set_app_id("dev.zant.release-test");
        assert!(
            tsx::platform::asset_image(include_bytes!(
                "../../../../scaffold/template-ts/assets/icons/statusbar.png"
            ))
            .is_some()
        );
        assert_eq!(tsx::platform::asset_image_count(), 1);

        let core = Rc::new(RefCell::new(Runtime::init(
            App {
                held: held.clone(),
                panel: None,
            },
            ui::RunOptions::default(),
        )));
        crate::desktop::set_main_runtime(Some(core.clone()));
        window::register(&core);
        core.borrow_mut().rebuild_for_test();
        let panel = Rc::new(RefCell::new(Runtime::init_window(
            core.borrow().shared_app(),
            WindowHooks::new({
                let held = held.clone();
                move || {
                    ui::column(ui::BoxOptions {
                        children: crate::nodes![ui::button(ui::ButtonOptions {
                            text: "panel".into(),
                            on_click: Some(action(held.clone())),
                            ..Default::default()
                        })],
                        ..Default::default()
                    })
                }
            }),
            ui::RunOptions::default(),
        )));
        window::register(&panel);
        panel.borrow_mut().rebuild_for_test();
        core.borrow().app_state_mut::<App>().panel = Some(panel.clone());
        window::request_close(panel.borrow().owner_ptr());
        assert_eq!(window::open_count(), 2);
        let panel_weak = Rc::downgrade(&panel);
        drop(panel);
        ui::effect(action(held.clone()));
        ui::push_portal(ui::button(ui::ButtonOptions {
            text: "portal".into(),
            on_click: Some(action(held.clone())),
            ..Default::default()
        }));
        ui::set_key_bounds_hook(Some(Rc::new({
            let held = held.clone();
            move |_| {
                assert!(Rc::strong_count(&held) > 0);
                None
            }
        })));
        tsx::timers::set_running(true);
        ui::set_build_states(Some(core.borrow().local_states.clone()));
        ui::local_state(456, || held.clone());
        ui::set_command_owner(Some(core.borrow().owner_ptr()));
        ui::focus("app-key");
        let lists = Lists::default();
        let list_weak = Rc::downgrade(&lists);
        // Leave a populated build registry (e.g. an interrupted build).
        std::mem::forget(ListBuildGuard::enter(lists.clone()));
        view_state::register_list("list", &Rc::new(RefCell::new(ui::ListState::new())));
        assert_eq!(lists.borrow().len(), 1);
        drop(lists);
        drop(held);

        release_app();
        assert!(
            weak.upgrade().is_none(),
            "all app values / callbacks dropped"
        );
        assert!(
            panel_weak.upgrade().is_none(),
            "secondary runtime cycle broken"
        );
        assert!(list_weak.upgrade().is_none());
        assert_eq!(core.borrow().local_states.borrow().count(), 0);
        assert!(core.borrow().window_hooks.is_none());
        assert!(!tsx::native::registered("release-test"));
        assert_eq!(tsx::timers::pending(), 0);
        assert!(!tsx::timers::is_running());
        assert_ne!(tsx::timers::now(), -1_000_000);
        assert!(!hotkey.registered());
        tsx::desktop::assert_app_released();
        platform::assert_app_released();
        crate::desktop::window::assert_app_released();
        window::assert_app_released();
        view_state::assert_app_released();
        ui::context::assert_app_released();
        assert!(ui::take_commands().is_empty());
        assert_eq!(ui::set_command_owner(None), None);
        assert_eq!(tsx::platform::asset_image_count(), 0);
        assert!(tsx::platform::app_id().is_none());
        // The fallback slot can now be reused with a different app type.
        assert!(ui::local_slot(123, || String::from("new version")).fresh);
        release_app(); // idempotent
    }

    #[test]
    fn stop_releases_after_app_drop_on_reload_and_exit() {
        let _native = platform::native_test_lock();
        struct Stopping(Rc<Cell<u32>>);
        impl ui::App for Stopping {
            fn render(&mut self) -> ui::Node {
                ui::Node::default()
            }
            fn on_exit(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        impl Drop for Stopping {
            fn drop(&mut self) {
                // Teardown must also discard work queued by destructors.
                tsx::timers::set_timeout(1.0, ui::Action::new(|_| panic!("old app")));
                ui::effect(ui::Action::new(|_| panic!("old app")));
            }
        }
        let exits = Rc::new(Cell::new(0));
        for exiting in [false, true] {
            let core = Rc::new(RefCell::new(Runtime::init(
                Stopping(exits.clone()),
                ui::RunOptions::default(),
            )));
            let weak = Rc::downgrade(&core);
            let (native, _) = platform::Window::adopt(
                std::ptr::null_mut(),
                crate::runtime::host::RuntimeHost::boxed(core.clone()),
            );
            window::register(&core);
            crate::desktop::set_main_runtime(Some(core.clone()));
            crate::hot::Instance {
                core,
                _window: Rc::new(native),
            }
            .stop(exiting);
            assert!(weak.upgrade().is_none());
            assert_eq!(tsx::timers::pending(), 0);
            ui::context::assert_app_released();
            window::assert_app_released();
        }
        assert_eq!(exits.get(), 1);
    }
}
