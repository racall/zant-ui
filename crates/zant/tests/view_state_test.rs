use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;
use zant::core::events::{Event, Modifiers, PointerEvent, Scroll};
use zant::core::node::NodeData;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::{
    self, App, BoxOptions, InputOptions, ListOptions, ListState, Node, RunOptions, TextOptions,
};

struct Page {
    text: &'static str,
    list: Rc<RefCell<ListState>>,
    show: bool,
}
impl Default for Page {
    fn default() -> Self {
        Self {
            text: "hello 世界",
            list: Rc::default(),
            show: true,
        }
    }
}
impl App for Page {
    fn render(&mut self) -> Node {
        let children = if self.show {
            vec![
                ui::column(BoxOptions {
                    key: Some("scroll".into()),
                    width: Some(300.0),
                    scroll: true,
                    height: Some(80.0),
                    children: vec![ui::column(BoxOptions {
                        height: Some(800.0),
                        ..Default::default()
                    })],
                    ..Default::default()
                }),
                ui::list(
                    &self.list,
                    ListOptions {
                        key: "list".into(),
                        count: 1000,
                        height: Some(80.0),
                        grow: 0.0,
                        item_height: Some(24.0),
                        overdraw: 0.0,
                        item: Rc::new(|_| ui::text("row", TextOptions::default())),
                        ..Default::default()
                    },
                ),
                ui::input(InputOptions {
                    key: Some("input".into()),
                    value: Some(self.text.into()),
                    ..Default::default()
                }),
                ui::column(BoxOptions {
                    key: Some("doc".into()),
                    selectable: true,
                    children: vec![ui::text(
                        "Hello 世界",
                        TextOptions {
                            key: Some("text".into()),
                            ..Default::default()
                        },
                    )],
                    ..Default::default()
                }),
                ui::button(ui::ButtonOptions {
                    key: Some("button".into()),
                    text: "Reload".into(),
                    ..Default::default()
                }),
            ]
        } else {
            vec![]
        };
        ui::column(BoxOptions {
            children,
            ..Default::default()
        })
    }
}
fn runtime(page: Page) -> Runtime {
    Runtime::init(page, RunOptions::default())
}
fn frame(rt: &mut Runtime) {
    let mut renderer = Renderer::create_raster(320, 400).unwrap();
    let canvas = renderer.begin(1.0).unwrap();
    rt.render_frame(canvas, 320.0, 400.0, 1.0);
    renderer.end();
}
fn scroll(rt: &mut Runtime, key: &str, dy: f32) {
    let bounds = rt.tree.get(rt.find_key(key).unwrap()).bounds;
    rt.dispatch(Event::Scroll(Scroll {
        x: bounds.x + 10.0,
        y: bounds.y + 10.0,
        dx: 0.0,
        dy,
        modifiers: Modifiers::default(),
    }));
    frame(rt);
}
fn offset(rt: &Runtime) -> f32 {
    rt.tree
        .get(rt.find_key("scroll").unwrap())
        .container()
        .unwrap()
        .scroll_offset
}
#[test]
fn keyed_scroll_survives_restore_before_first_build() {
    let mut old = runtime(Page::default());
    frame(&mut old);
    scroll(&mut old, "scroll", -123.0);
    assert_eq!(offset(&old), 123.0);
    let mut new = runtime(Page::default());
    new.restore_view_state(old.view_state());
    frame(&mut new);
    frame(&mut new);
    assert_eq!(offset(&new), 123.0);
    let id = new.find_key("scroll").unwrap();
    assert!(!new.tree.get(id).container().unwrap().scroll_animating);
}

#[test]
fn pointer_hover_survives_reload_settled_without_restoring_press() {
    for pressed in [false, true] {
        let mut old = runtime(Page::default());
        frame(&mut old);
        let id = old.find_key("button").unwrap();
        let bounds = old.tree.get(id).bounds;
        let pointer = PointerEvent {
            x: bounds.x + 5.0,
            y: bounds.y + 5.0,
            ..Default::default()
        };
        old.dispatch(Event::PointerMove(pointer));
        assert_eq!(old.hovered, Some(id));
        if pressed {
            old.dispatch(Event::PointerDown(pointer));
            assert_eq!(old.pressed_button, Some(id));
        }
        let saved = old.view_state();
        assert_eq!(
            saved["pointer"],
            json!({
                "x": pointer.x, "y": pointer.y, "pressed": pressed,
            })
        );
        for theme in [
            zant::theme::tokens::ant_light(),
            zant::theme::tokens::ant_dark(),
        ] {
            for (width, height) in [(320, 400), (800, 600)] {
                let mut new = Runtime::init(
                    Page::default(),
                    RunOptions {
                        theme,
                        ..Default::default()
                    },
                );
                new.restore_view_state(saved.clone());
                let mut renderer = Renderer::create_raster(width, height).unwrap();
                for _ in 0..2 {
                    let canvas = renderer.begin(1.0).unwrap();
                    new.render_frame(canvas, width as f32, height as f32, 1.0);
                    renderer.end();
                    let id = new.find_key("button").unwrap();
                    assert_eq!(new.hovered, Some(id));
                    let NodeData::Button(data) = &new.tree.get(id).data else {
                        panic!("button")
                    };
                    assert!(data.hovered);
                    assert_eq!(data.hover_t, 1.0);
                    assert!(data.hover.done(new.now_ms));
                    assert!(!data.pressed);
                    assert_eq!(data.press_t, 0.0);
                    assert!(new.pressed_button.is_none());
                    assert!(new.pointer_capture.is_none());
                    assert!(!new.sel_dragging);
                    assert_eq!(new.view_state()["pointer"]["pressed"], false);
                }
            }
        }
    }
}

#[test]
fn pointer_outside_window_does_not_restore_hover() {
    let mut old = runtime(Page::default());
    frame(&mut old);
    let bounds = old.tree.get(old.find_key("button").unwrap()).bounds;
    let pointer = PointerEvent {
        x: bounds.x + 5.0,
        y: bounds.y + 5.0,
        ..Default::default()
    };
    old.dispatch(Event::PointerMove(pointer));
    old.dispatch(Event::PointerLeave(pointer));
    let mut new = runtime(Page::default());
    new.restore_view_state(old.view_state());
    frame(&mut new);
    frame(&mut new);
    assert!(new.hovered.is_none());
    assert!(new.view_state()["pointer"].is_null());
}
#[test]
fn virtual_list_survives_reload() {
    let mut old = runtime(Page::default());
    frame(&mut old);
    scroll(&mut old, "list", -321.0);
    let saved = old.app_state_mut::<Page>().list.borrow().scroll_top();
    assert_eq!(saved, 321.0);
    let mut new = runtime(Page::default());
    new.restore_view_state(old.view_state());
    frame(&mut new);
    frame(&mut new);
    assert_eq!(
        new.app_state_mut::<Page>().list.borrow().scroll_top(),
        saved
    );
    assert!(new.app_state_mut::<Page>().list.borrow().rendered_first > 0);
}
#[test]
fn focus_and_input_selection_survive_and_clamp_utf8() {
    let mut old = runtime(Page::default());
    frame(&mut old);
    let id = old.find_key("input").unwrap();
    old.tree.set_focus(Some(id));
    if let NodeData::Input(data) = &mut old.tree.get_mut(id).data {
        data.anchor = 2;
        data.cursor = 12;
    }
    let saved = old.view_state();
    for (text, expected) in [("hello 世界", (2, 12)), ("世", (0, 3)), ("", (0, 0))] {
        let mut new = runtime(Page {
            text,
            ..Default::default()
        });
        new.restore_view_state(saved.clone());
        frame(&mut new);
        frame(&mut new);
        let id = new.find_key("input").unwrap();
        assert_eq!(new.tree.focused, Some(id));
        let NodeData::Input(data) = &new.tree.get(id).data else {
            panic!("input")
        };
        assert_eq!((data.anchor, data.cursor), expected);
        assert_eq!(data.focus_t, 1.0);
        assert!(data.focus_anim.done(new.now_ms));
    }
}
#[test]
fn malformed_and_unknown_state_is_ignored_and_expires() {
    let mut rt = runtime(Page {
        show: false,
        ..Default::default()
    });
    for value in [
        json!(null),
        json!([]),
        json!({"nodes": true}),
        json!({"nodes": {"scroll": {"scroll": "bad"}, "input": {"focus": true, "input": {"cursor": -1}}}, "selection": {}}),
    ] {
        rt.restore_view_state(value);
        frame(&mut rt);
    }
    rt.restore_view_state(
        json!({"nodes": {"scroll": {"scroll": 100}, "unknown": {"focus": true}}}),
    );
    for _ in 0..10 {
        frame(&mut rt);
    }
    rt.app_state_mut::<Page>().show = true;
    rt.invalidate();
    frame(&mut rt);
    assert_eq!(offset(&rt), 0.0);
}
#[test]
fn delayed_keys_apply_once_and_scroll_clamps() {
    let mut rt = runtime(Page {
        show: false,
        ..Default::default()
    });
    rt.restore_view_state(json!({"nodes": {"scroll": {"scroll": 99999}, "missing": {}}}));
    frame(&mut rt);
    rt.app_state_mut::<Page>().show = true;
    rt.invalidate();
    frame(&mut rt);
    assert_eq!(offset(&rt), 720.0);
    let id = rt.find_key("scroll").unwrap();
    rt.tree.scroll_by(id, -20.0);
    frame(&mut rt);
    assert_eq!(offset(&rt), 700.0);
}
#[test]
fn selectable_text_selection_survives_reload() {
    let mut old = runtime(Page::default());
    frame(&mut old);
    old.sel_root = old.find_key("doc");
    let id = old.find_key("text").unwrap();
    old.sel_anchor = Some(zant::core::text_select::Point { node: id, byte: 0 });
    old.sel_focus = Some(zant::core::text_select::Point { node: id, byte: 9 });
    let mut new = runtime(Page::default());
    new.restore_view_state(old.view_state());
    frame(&mut new);
    frame(&mut new);
    assert_eq!(new.selected_text(), "Hello 世");
    let id = new.find_key("text").unwrap();
    assert_eq!(new.tree.get(id).text_sel, Some([0, 9]));
}

#[test]
fn duplicate_keys_use_first_in_tree_order_not_arena_order() {
    struct Duplicates;
    impl App for Duplicates {
        fn render(&mut self) -> Node {
            ui::column(BoxOptions {
                children: (0..2)
                    .map(|_| {
                        ui::column(BoxOptions {
                            key: Some("duplicate".into()),
                            scroll: true,
                            height: Some(80.0),
                            children: vec![ui::column(BoxOptions {
                                height: Some(800.0),
                                ..Default::default()
                            })],
                            ..Default::default()
                        })
                    })
                    .collect(),
                ..Default::default()
            })
        }
    }
    let mut old = Runtime::init(Duplicates, RunOptions::default());
    frame(&mut old);
    let root = old.tree.root;
    let children = old.tree.get(root).children.clone();
    old.tree.scroll_by(children[0], 20.0);
    old.tree.scroll_by(children[1], 90.0);
    old.tree.get_mut(root).children.reverse();
    let snapshot = old.view_state();
    assert_eq!(snapshot["nodes"]["duplicate"]["scroll"], 90.0);
    let mut new = Runtime::init(Duplicates, RunOptions::default());
    // Also reverse the destination's arena order before applying.
    frame(&mut new);
    let root = new.tree.root;
    new.tree.get_mut(root).children.reverse();
    new.restore_view_state(snapshot);
    frame(&mut new);
    frame(&mut new);
    let children = &new.tree.get(root).children;
    assert_eq!(
        new.tree.get(children[0]).container().unwrap().scroll_offset,
        90.0
    );
    assert_eq!(
        new.tree.get(children[1]).container().unwrap().scroll_offset,
        0.0
    );
}

#[test]
fn malformed_entries_on_existing_nodes_do_not_panic() {
    let mut rt = runtime(Page::default());
    frame(&mut rt);
    for value in [
        json!({"nodes": {"scroll": null, "input": true, "list": []}}),
        json!({"nodes": {"scroll": {"scroll": 1e300}, "input": {"focus": true, "input": {"anchor": -1, "cursor": "bad"}}, "list": {"list": {"anchor": 5, "offset": 1e300}}}}),
        json!({"selection": {"root": "doc", "anchor": {"key": "text", "byte": false}, "focus": {"key": "text", "byte": -10}}}),
        json!({"pointer": {"x": 1e300, "y": 1}}),
        json!({"pointer": {"x": 1, "y": "bad", "pressed": true}}),
    ] {
        rt.restore_view_state(value);
        frame(&mut rt);
        frame(&mut rt);
    }
    assert_eq!(offset(&rt), 0.0);
}

#[test]
fn restores_across_window_sizes_and_themes() {
    let mut old = runtime(Page::default());
    frame(&mut old);
    scroll(&mut old, "scroll", -55.0);
    let id = old.find_key("input").unwrap();
    old.tree.set_focus(Some(id));
    let saved = old.view_state();
    for theme in [
        zant::theme::tokens::ant_light(),
        zant::theme::tokens::ant_dark(),
    ] {
        for (width, height) in [(240, 250), (800, 600)] {
            let mut rt = Runtime::init(
                Page::default(),
                RunOptions {
                    theme,
                    ..Default::default()
                },
            );
            rt.restore_view_state(saved.clone());
            let mut renderer = Renderer::create_raster(width, height).unwrap();
            for _ in 0..2 {
                let canvas = renderer.begin(1.0).unwrap();
                rt.render_frame(canvas, width as f32, height as f32, 1.0);
                renderer.end();
            }
            assert_eq!(offset(&rt), 55.0);
            assert_eq!(rt.tree.focused, rt.find_key("input"));
        }
    }
}
