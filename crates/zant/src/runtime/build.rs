//! Port of `src/ui_runtime.zig:292-382` + `setTheme`: the build stage.
//!
//! [`Runtime::rebuild`] runs the app's `render`, expands portals, performs the
//! `ui::wake_after` / `ui::document_edited` bookkeeping and reconciles the
//! declarative root into the retained tree; then it runs the queued effects.
//! [`Runtime::set_theme`] switches the theme and drops every cached text
//! layout.
//!
//! **Hard rule (plan.md §4):** the build context borrow is released before
//! `App::render` runs — every `ui::*` call borrows it again.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::core::node::{NodeData, NodeKind};
use crate::theme::tokens::{self, Theme};
use crate::ui::element::Node;

use super::{Runtime, action_cx};

impl Runtime {
    /// Runs one rebuild: `buildTree` plus the `ui::effect` callbacks. An effect
    /// that changed state asks for another build (`needs_build`).
    pub(crate) fn rebuild(&mut self) {
        self.build_tree();
        // `ui::effect` callbacks: after the tree is in place; they may set
        // state, so another build follows
        let requested = {
            let mut app = self.app.borrow_mut();
            let mut cx = action_cx(&mut app);
            crate::ui::run_effects(&mut cx)
        };
        if requested {
            self.needs_build = true;
            self.request_redraw();
        }
    }

    /// Zig `drawsCaptionButtons`: a hidden-titlebar window on Windows draws
    /// its own caption buttons (macOS keeps the traffic lights).
    fn draws_caption_buttons(&self) -> bool {
        cfg!(target_os = "windows")
            && self.config.window.style == crate::platform::platform::WindowStyle::HiddenTitlebar
    }

    /// Height of the app-drawn title bar (Windows 11 default: 32).
    fn caption_height(&self) -> f32 {
        let height = self.config.window.titlebar_height;
        if height > 0.0 { height } else { 32.0 }
    }

    /// Zig `buildTree`: theme / window style / build states, then `render` and
    /// [`Runtime::reconcile_root`].
    pub(crate) fn build_tree(&mut self) {
        let theme = self.app.borrow().theme();
        if let Some(theme) = theme {
            self.set_theme(theme);
        }
        crate::ui::set_build_theme(self.theme);
        crate::ui::set_build_window_style(self.config.window.style);
        // memo subtrees build during reconcile, so the store stays set (and
        // sweeps unmounted slots) until the end
        self.local_states.borrow_mut().begin_build();
        crate::ui::set_build_states(Some(Rc::clone(&self.local_states)));
        crate::ui::begin_frame();
        // `ui::key_bounds` answers from this window's snapshot while it builds
        // (Zig pointed the build context at the live tree; multi-window needs
        // the hook to follow the window being built).
        let bounds = Rc::clone(&self.key_bounds);
        crate::ui::set_key_bounds_hook(Some(Rc::new(move |key: &str| {
            bounds.borrow().get(key).copied()
        })));
        let prev_owner = crate::ui::set_command_owner(Some(self.owner_ptr()));

        let mut root = match &self.window_hooks {
            Some(hooks) => (hooks.render)(),
            None => self.app.borrow_mut().render(),
        };
        let portals = crate::ui::take_portals();
        if !portals.is_empty() && root.element().kind == NodeKind::Container {
            root = crate::ui::append_children(root, &portals);
        }
        // App-drawn Windows caption buttons (Zig `drawsCaptionButtons`): a
        // hidden-titlebar window on Windows gets the minimize / maximize /
        // close buttons in the top-right corner of its own title bar.
        if self.draws_caption_buttons() && root.element().kind == NodeKind::Container {
            let options = crate::ui::WindowControlsOptions {
                height: self.caption_height(),
                maximized: self
                    .window
                    .as_ref()
                    .is_some_and(|window| window.is_maximized()),
                active: self.active,
                on_minimize: Some(crate::ui::Action::new(|_cx| {
                    crate::ui::window_minimize();
                })),
                on_maximize: Some(crate::ui::Action::new(|_cx| {
                    crate::ui::window_maximize();
                })),
                on_close: Some(crate::ui::Action::new(|_cx| {
                    crate::ui::window_close();
                })),
            };
            root = crate::ui::append_children(root, &[crate::ui::window_controls(options)]);
        }
        self.wake_at_ms = crate::ui::take_wake().map(|ms| self.now_ms + i64::from(ms));
        if let Some(edited) = crate::ui::take_document_edited()
            && edited != self.document_edited
        {
            self.document_edited = edited;
            if let Some(window) = &self.window {
                window.set_document_edited(edited);
            }
        }
        if root.element().kind == NodeKind::Container {
            self.reconcile_root(root);
        } else {
            eprintln!("Zant UI: render() must return a container (ui.column/ui.row/ui.card)");
        }
        self.needs_build = false;
        // Keep the hook installed for the rest of the frame: layout followers
        // (`notify_resizes`, app `on_resize` callbacks) still ask `ui::key_bounds`
        // for the boxes just laid out. The snapshot is refreshed after every
        // layout and cleared when the runtime is dropped.
        crate::ui::set_command_owner(prev_owner);
        crate::ui::set_build_states(None);
        crate::ui::end_frame();
        self.local_states.borrow_mut().sweep();
    }

    /// Zig `reconcileRoot`: the root always fills the window and needs a
    /// background, so the spec is normalised before it is compared.
    pub(crate) fn reconcile_root(&mut self, root_spec: Node) -> bool {
        let root_id = self.tree.root;
        let spec = root_spec.element();
        let mut want = spec.layout;
        want.width = None;
        want.height = None;
        want.grow = 0.0;
        let mut want_visual = spec.visual;
        if want_visual.background.is_none() {
            want_visual.background = Some(self.theme.color_bg_base);
        }
        let mut changed_layout = {
            let root = self.tree.get(root_id);
            root.style != want || root.visual != want_visual
        };
        {
            let root = self.tree.get_mut(root_id);
            root.style = want;
            root.visual = want_visual;
            root.enabled = true;
            root.focusable = false;
        }
        // keep the window's own background (visible while resizing) and the
        // frame clear colour in sync with the app's root background
        if let Some(background) = want_visual.background
            && background != self.clear_color
            && (background >> 24) == 0xff
        {
            self.clear_color = background;
            if let Some(window) = &self.window {
                window.set_background(background);
            }
        }
        if self.reconcile_children(root_id, &spec.children) {
            changed_layout = true;
        }
        if changed_layout {
            self.tree.mark_dirty(root_id, true, true);
        }
        changed_layout
    }

    /// Zig `setTheme`: switches the active theme and invalidates every cached
    /// text layout (the colours are baked into the shaped paragraphs).
    pub(crate) fn set_theme(&mut self, theme: Theme) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        self.tree.theme = theme;
        self.tree.selection_color = tokens::with_alpha(theme.color_primary, 0x4d);
        for node in &mut self.tree.nodes {
            match &mut node.data {
                NodeData::Text(data) => data.invalidate(),
                NodeData::Button(data) => data.invalidate_layout(),
                NodeData::Input(data) => {
                    data.invalidate_layout();
                    data.placeholder_layout = None;
                }
                NodeData::Toggle(data) => data.invalidate_layout(),
                NodeData::Select(data) => {
                    data.layout = None;
                    data.option_layouts.clear();
                    data.layout_dirty = true;
                }
                NodeData::RichText(data) => data.invalidate_layout(),
                _ => {}
            }
        }
        self.tree.mark_layout_dirty();
        self.tree.mark_paint_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Rect;
    use crate::core::node::{NodeData, NodeId};
    use crate::renderer::renderer::Renderer;
    use crate::ui::actions::action;
    use crate::ui::app::App;
    use crate::ui::builders::{column, text};
    use crate::ui::options::{BoxOptions, MemoOptions, TextOptions};
    use crate::ui::{RunOptions, Style, portal, styled};
    use std::cell::Cell;
    use std::rc::Rc;

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 300.0, 200.0, 1.0);
        renderer.end();
    }

    #[test]
    fn caption_buttons_follow_the_platform_and_window_style() {
        #[derive(Default)]
        struct Empty;
        impl App for Empty {
            fn render(&mut self) -> crate::ui::Node {
                column(BoxOptions::default())
            }
        }
        let mut options = RunOptions::default();
        options.window.style = crate::platform::platform::WindowStyle::HiddenTitlebar;
        let mut runtime = Runtime::init(Empty, options);
        // macOS keeps the traffic lights (and Zig only drew the buttons on
        // Windows); the flag also tracks the window style.
        assert_eq!(cfg!(target_os = "windows"), runtime.draws_caption_buttons());
        assert_eq!(32.0, runtime.caption_height());
        runtime.config.window.titlebar_height = 52.0;
        assert_eq!(52.0, runtime.caption_height());
        runtime.config.window.style = crate::platform::platform::WindowStyle::Native;
        assert!(!runtime.draws_caption_buttons());
    }

    #[test]
    fn portals_are_appended_to_the_root_after_the_app_children() {
        struct WithPortal;
        impl App for WithPortal {
            fn render(&mut self) -> Node {
                column(BoxOptions {
                    children: crate::nodes![
                        text("page", TextOptions::default()),
                        portal(column(BoxOptions {
                            width: Some(40.0),
                            height: Some(20.0),
                            ..Default::default()
                        })),
                    ],
                    ..Default::default()
                })
            }
        }
        let mut runtime = Runtime::init(WithPortal, RunOptions::default());
        runtime.rebuild_for_test();
        let root = runtime.tree.get(runtime.tree.root);
        assert_eq!(root.children.len(), 3, "text, placeholder, portal");
        let last = *root.children.last().expect("portal child");
        let portal_node = runtime.tree.get(last);
        assert_eq!(portal_node.kind(), crate::core::node::NodeKind::Container);
        assert_eq!(portal_node.style.width, Some(40.0));
    }

    #[test]
    fn effects_run_after_the_build_and_can_request_another_one() {
        struct Effects {
            runs: u32,
            rebuilds: u32,
        }
        impl App for Effects {
            fn render(&mut self) -> Node {
                let rebuild = self.rebuilds > 0;
                crate::ui::effect(action(move |state: &mut Effects| {
                    state.runs += 1;
                }));
                if rebuild {
                    crate::ui::effect(action(move |_state: &mut Effects| {
                        crate::ui::request_rebuild();
                    }));
                }
                column(BoxOptions::default())
            }
        }
        let mut runtime = Runtime::init(
            Effects {
                runs: 0,
                rebuilds: 0,
            },
            RunOptions::default(),
        );
        runtime.rebuild_for_test();
        assert_eq!(runtime.app_state_mut::<Effects>().runs, 1);
        // an effect asking for a rebuild leaves the tree dirty
        runtime.app_state_mut::<Effects>().rebuilds = 1;
        runtime.needs_build = true;
        runtime.rebuild_for_test();
        assert_eq!(runtime.app_state_mut::<Effects>().runs, 2);
        assert!(runtime.needs_build, "the effect requested another build");
    }

    #[test]
    fn memo_subtrees_are_skipped_and_keep_their_local_state() {
        struct Memo {
            hash: u64,
            builds: Rc<Cell<u32>>,
        }
        impl App for Memo {
            fn render(&mut self) -> Node {
                let builds = Rc::clone(&self.builds);
                let hash = self.hash;
                let block = crate::ui::memo(
                    MemoOptions {
                        key: Some("block".into()),
                        hash,
                    },
                    move || {
                        builds.set(builds.get() + 1);
                        let count = crate::ui::local_state(7u64, || 0u32);
                        *count.borrow_mut() += 1;
                        let value = format!("{}", *count.borrow());
                        column(BoxOptions {
                            children: crate::nodes![text(
                                value,
                                TextOptions {
                                    key: Some("count".into()),
                                    ..Default::default()
                                }
                            )],
                            ..Default::default()
                        })
                    },
                );
                column(BoxOptions {
                    children: crate::nodes![block],
                    ..Default::default()
                })
            }
        }
        let builds = Rc::new(Cell::new(0));
        let mut runtime = Runtime::init(
            Memo {
                hash: 1,
                builds: Rc::clone(&builds),
            },
            RunOptions::default(),
        );
        runtime.rebuild_for_test();
        assert_eq!(builds.get(), 1);
        assert_eq!(count_text(&runtime), "1");
        // same hash: the memo build is skipped, but its local state survives
        runtime.needs_build = true;
        runtime.rebuild_for_test();
        assert_eq!(builds.get(), 1, "the memo was skipped");
        assert_eq!(count_text(&runtime), "1", "the local state was not reset");
        // a new hash rebuilds it, and the component state persists
        runtime.app_state_mut::<Memo>().hash = 2;
        runtime.needs_build = true;
        runtime.rebuild_for_test();
        assert_eq!(builds.get(), 2);
        assert_eq!(count_text(&runtime), "2");
    }

    fn count_text(runtime: &Runtime) -> String {
        let id = runtime.find_key("count").expect("count text");
        match &runtime.tree.get(id).data {
            NodeData::Text(data) => data.content.to_string(),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn focus_commands_run_after_the_layout() {
        struct Focus {
            queued: bool,
        }
        impl App for Focus {
            fn render(&mut self) -> Node {
                if !self.queued {
                    self.queued = true;
                    crate::ui::focus("field");
                }
                let field = Node::new(crate::ui::Element {
                    kind: crate::core::node::NodeKind::Input,
                    focusable: true,
                    layout: crate::core::node::FlexStyle {
                        width: Some(120.0),
                        height: Some(32.0),
                        ..Default::default()
                    },
                    ..Default::default()
                });
                // the input has no key in `Element`; wrap it in a keyed container
                let keyed = crate::ui::with_key(field, "field");
                column(BoxOptions {
                    children: crate::nodes![keyed],
                    ..Default::default()
                })
            }
        }
        let mut runtime = Runtime::init(Focus { queued: false }, RunOptions::default());
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let field = runtime.find_key("field").expect("keyed input");
        assert_eq!(
            runtime.tree.focused,
            Some(field),
            "ui::focus ran after layout"
        );
    }

    #[test]
    fn key_bounds_reads_the_previous_frame_boxes() {
        struct Bounds {
            seen: Option<Rect>,
        }
        impl App for Bounds {
            fn render(&mut self) -> Node {
                self.seen = crate::ui::key_bounds("marker");
                column(BoxOptions {
                    children: crate::nodes![with_marker()],
                    ..Default::default()
                })
            }
        }
        fn with_marker() -> Node {
            crate::ui::with_key(
                styled(
                    column(BoxOptions {
                        width: Some(60.0),
                        height: Some(24.0),
                        ..Default::default()
                    }),
                    Style::default(),
                ),
                "marker",
            )
        }
        let mut runtime = Runtime::init(Bounds { seen: None }, RunOptions::default());
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        assert!(
            runtime.app_state_mut::<Bounds>().seen.is_none(),
            "first frame"
        );
        runtime.needs_build = true;
        frame(&mut runtime, &mut renderer);
        let seen = runtime
            .app_state_mut::<Bounds>()
            .seen
            .expect("second frame");
        assert_eq!((seen.width, seen.height), (60.0, 24.0));
    }

    #[test]
    fn document_edited_and_theme_hooks_are_applied_to_the_runtime() {
        struct Doc;
        impl App for Doc {
            fn render(&mut self) -> Node {
                crate::ui::document_edited(true);
                column(BoxOptions::default())
            }
            fn theme(&self) -> Option<Theme> {
                Some(crate::theme::tokens::ant_dark())
            }
        }
        let mut runtime = Runtime::init(Doc, RunOptions::default());
        runtime.rebuild_for_test();
        assert!(runtime.document_edited);
        assert_eq!(runtime.theme, crate::theme::tokens::ant_dark());
        assert_eq!(runtime.tree.theme, crate::theme::tokens::ant_dark());
    }

    #[test]
    fn rebuild_requests_from_callbacks_reach_the_frame_loop() {
        struct Counter {
            clicks: u32,
        }
        impl App for Counter {
            fn render(&mut self) -> Node {
                column(BoxOptions {
                    children: crate::nodes![crate::ui::button(crate::ui::ButtonOptions {
                        key: Some("add".into()),
                        text: "Add".into(),
                        on_click: Some(action(|state: &mut Counter| state.clicks += 1)),
                        ..Default::default()
                    })],
                    ..Default::default()
                })
            }
        }
        let mut runtime = Runtime::init(Counter { clicks: 0 }, RunOptions::default());
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let id = runtime.find_key("add").expect("button");
        let bounds = runtime.tree.get(id).bounds;
        let point = crate::core::events::PointerEvent {
            x: bounds.x + bounds.width * 0.5,
            y: bounds.y + bounds.height * 0.5,
            ..Default::default()
        };
        runtime.dispatch(crate::core::events::Event::PointerDown(point));
        runtime.dispatch(crate::core::events::Event::PointerUp(point));
        assert_eq!(runtime.app_state_mut::<Counter>().clicks, 1);
        assert!(runtime.needs_build, "the click asks for a rebuild");
        frame(&mut runtime, &mut renderer);
        assert!(!runtime.needs_build);
        assert_eq!(runtime.app_state_mut::<Counter>().clicks, 1);
    }

    struct BareApp;

    impl App for BareApp {
        fn render(&mut self) -> Node {
            column(BoxOptions::default())
        }
    }

    #[test]
    fn the_tree_snapshot_hook_tracks_the_latest_layout() {
        let mut runtime = Runtime::init(BareApp, RunOptions::default());
        runtime.rebuild_for_test();
        runtime.refresh_key_bounds();
        assert!(runtime.key_bounds.borrow().is_empty());
        let id: NodeId = runtime.tree.root;
        runtime.tree.set_key(id, Some("root-key"));
        runtime.refresh_key_bounds();
        assert!(runtime.key_bounds.borrow().contains_key("root-key"));
    }
}
