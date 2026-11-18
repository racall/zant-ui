//! Unit tests.

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
    // macOS keeps the traffic lights (the buttons are only drawn on
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
