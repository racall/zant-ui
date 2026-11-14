//! Port of `tests/lifecycle_test.zig` — node lifecycle: removed subtrees are
//! freed and recycled, runtime references to them are cleared, keyed children
//! match strictly.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use zant::core::events::{Event, PointerEvent};
use zant::core::node::NodeId;
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::actions::action;
use zant::ui::app::{App, RunOptions};
use zant::ui::builders::{button, column, input, text};
use zant::ui::element::Node;
use zant::ui::options::{BoxOptions, ButtonOptions, InputOptions, TextOptions};

fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, 400.0, 300.0, 1.0);
    renderer.end();
}

fn center(runtime: &Runtime, id: NodeId) -> PointerEvent {
    let b = runtime.tree.get(id).bounds;
    PointerEvent {
        x: b.x + b.width * 0.5,
        y: b.y + b.height * 0.5,
        ..Default::default()
    }
}

struct State {
    show_panel: bool,
    panel_key: &'static str,
    clicks: usize,
}

impl State {
    fn new() -> State {
        State {
            show_panel: true,
            panel_key: "menu",
            clicks: 0,
        }
    }
}

impl App for State {
    fn render(&mut self) -> Node {
        if !self.show_panel {
            return column(BoxOptions {
                padding: zant::ui::inset(16.0),
                children: zant::nodes![text("empty", TextOptions::default())],
                ..Default::default()
            });
        }
        column(BoxOptions {
            padding: zant::ui::inset(16.0),
            gap: 8.0,
            children: zant::nodes![
                text("header", TextOptions::default()),
                column(BoxOptions {
                    key: Some(self.panel_key.into()),
                    gap: 4.0,
                    children: zant::nodes![
                        button(ButtonOptions {
                            key: Some("action".into()),
                            text: "Action".into(),
                            on_click: Some(action(|state: &mut State| state.clicks += 1)),
                            ..Default::default()
                        }),
                        input(InputOptions {
                            key: Some("field".into()),
                            value: Some("abc".into()),
                            ..Default::default()
                        }),
                        text("footer", TextOptions::default()),
                    ],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn toggling_a_subtree_recycles_node_slots() {
    let mut runtime = Runtime::init(State::new(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");

    frame(&mut runtime, &mut renderer);
    let mut high_water: usize = 0;
    for _ in 0..100 {
        {
            let state = &mut *runtime.app_state_mut::<State>();
            state.show_panel = !state.show_panel;
        }
        runtime.invalidate();
        frame(&mut runtime, &mut renderer);
        high_water = high_water.max(runtime.tree.nodes.len());
    }
    // slots are reused: the slot count stops growing after the first toggles
    assert!(runtime.tree.nodes.len() <= high_water);
    assert!(high_water < 20);
}

#[test]
fn removed_nodes_leave_no_dangling_runtime_references() {
    let mut runtime = Runtime::init(State::new(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");
    frame(&mut runtime, &mut renderer);

    let button_id = runtime.tree.id_of_key("action").expect("action button");
    let field_id = runtime.tree.id_of_key("field").expect("field");
    let p = center(&runtime, button_id);
    // focus the input, then hover and press the button
    let field_center = center(&runtime, field_id);
    runtime.dispatch(Event::PointerDown(field_center));
    runtime.dispatch(Event::PointerUp(field_center));
    assert_eq!(runtime.tree.focused, Some(field_id));
    runtime.dispatch(Event::PointerMove(p));
    runtime.dispatch(Event::PointerDown(p));
    assert!(runtime.pressed_button.is_some());

    runtime.app_state_mut::<State>().show_panel = false;
    runtime.invalidate();
    frame(&mut runtime, &mut renderer);
    assert!(runtime.pressed_button.is_none());
    assert!(runtime.hovered.is_none());
    assert!(runtime.tree.focused.is_none());
    // releasing the mouse over where the button was must not fire or crash
    runtime.dispatch(Event::PointerUp(p));
    assert_eq!(runtime.app_state_mut::<State>().clicks, 0);
}

#[test]
fn keyed_children_never_take_over_a_node_with_another_key() {
    let mut runtime = Runtime::init(State::new(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");
    frame(&mut runtime, &mut renderer);

    let menu_id = runtime.tree.id_of_key("menu").expect("menu");
    runtime.app_state_mut::<State>().panel_key = "outline";
    runtime.invalidate();
    frame(&mut runtime, &mut renderer);
    let outline = runtime.tree.id_of_key("outline").expect("outline");
    assert!(runtime.tree.id_of_key("menu").is_none());
    // the old "menu" node was freed (its slot may be reused by a new node)
    let old_slot = &runtime.tree.nodes[menu_id as usize];
    assert!(!old_slot.alive || old_slot.key.is_none() || old_slot.key.as_deref() != Some("menu"));
    assert!(runtime.tree.get(outline).alive);
}

#[test]
fn an_unchanged_rebuild_does_not_relayout_the_tree() {
    let mut runtime = Runtime::init(State::new(), RunOptions::default());
    let mut renderer = Renderer::create_raster(400, 300).expect("raster renderer");
    frame(&mut runtime, &mut renderer);
    runtime.invalidate();
    frame(&mut runtime, &mut renderer);
    runtime.needs_build = true;
    runtime.rebuild_for_test();
    assert!(!runtime.tree.needs_layout());
}

struct VetoState {
    veto: bool,
}

impl App for VetoState {
    fn render(&mut self) -> Node {
        column(BoxOptions::default())
    }

    fn on_close_request(&mut self) -> bool {
        !self.veto
    }
}

#[test]
fn close_requests_are_vetoed_by_close_fn() {
    let mut runtime = Runtime::init(VetoState { veto: true }, RunOptions::default());
    assert_eq!(runtime.dispatch(Event::WindowCloseRequest), 1);
    // Zig cleared `config.close_fn`; Rust's `App::on_close_request` default
    // allows the close, so the state flag turns the veto off instead.
    runtime.app_state_mut::<VetoState>().veto = false;
    assert_eq!(runtime.dispatch(Event::WindowCloseRequest), 0);
}

/// A click handler that runs a native modal loop (file dialog, confirm
/// alert) gets frames painted while it waits. Those frames must not rebuild
/// the tree under the dispatching code: here the rebuild would free the
/// clicked button and recycle its slot as a container.
///
/// The frame is painted through the runtime's `Rc<RefCell<Runtime>>` (the
/// `RuntimeHost` path); the borrow is already held by `dispatch`, so the
/// nested paint is skipped — the Rust port of "keep showing the current tree".
struct ModalInner {
    replaced: bool,
    core: Option<Weak<RefCell<Runtime>>>,
    renderer: Option<Rc<RefCell<Renderer>>>,
}

struct ModalState {
    inner: Rc<RefCell<ModalInner>>,
}

impl ModalState {
    fn new(inner: Rc<RefCell<ModalInner>>) -> ModalState {
        ModalState { inner }
    }
}

impl App for ModalState {
    fn render(&mut self) -> Node {
        if self.inner.borrow().replaced {
            return column(BoxOptions {
                padding: zant::ui::inset(16.0),
                children: zant::nodes![
                    column(BoxOptions {
                        key: Some("panel".into()),
                        height: Some(40.0),
                        ..Default::default()
                    }),
                    column(BoxOptions {
                        key: Some("other".into()),
                        height: Some(40.0),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            });
        }
        let inner = Rc::clone(&self.inner);
        column(BoxOptions {
            padding: zant::ui::inset(16.0),
            children: zant::nodes![button(ButtonOptions {
                key: Some("open".into()),
                text: "Open".into(),
                on_click: Some(action(move |_state: &mut ModalState| {
                    let (core, renderer) = {
                        let mut tip = inner.borrow_mut();
                        tip.replaced = true;
                        (tip.core.clone(), tip.renderer.clone())
                    };
                    if let (Some(core), Some(renderer)) = (core, renderer)
                        && let Some(core) = core.upgrade()
                        && let Ok(mut runtime) = core.try_borrow_mut()
                        && let Ok(mut renderer) = renderer.try_borrow_mut()
                    {
                        // what the native modal loop does: paint a frame
                        // before the action returns (skipped while dispatching)
                        frame(&mut runtime, &mut renderer);
                    }
                })),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn a_frame_painted_inside_a_click_handler_does_not_rebuild_under_it() {
    let inner = Rc::new(RefCell::new(ModalInner {
        replaced: false,
        core: None,
        renderer: None,
    }));
    let core = Rc::new(RefCell::new(Runtime::init(
        ModalState::new(Rc::clone(&inner)),
        RunOptions::default(),
    )));
    let renderer = Rc::new(RefCell::new(
        Renderer::create_raster(400, 300).expect("raster renderer"),
    ));
    {
        let mut tip = inner.borrow_mut();
        tip.core = Some(Rc::downgrade(&core));
        tip.renderer = Some(Rc::clone(&renderer));
    }

    {
        let mut rt = core.borrow_mut();
        let mut r = renderer.borrow_mut();
        frame(&mut rt, &mut r);
        let id = rt.tree.id_of_key("open").expect("open button");
        let p = center(&rt, id);
        rt.dispatch(Event::PointerDown(p));
        rt.dispatch(Event::PointerUp(p));
    }
    assert!(inner.borrow().replaced);
    // the deferred rebuild runs on the next normal frame
    {
        let mut rt = core.borrow_mut();
        let mut r = renderer.borrow_mut();
        frame(&mut rt, &mut r);
        assert!(rt.tree.id_of_key("open").is_none());
        assert!(rt.tree.id_of_key("panel").is_some());
    }
}
