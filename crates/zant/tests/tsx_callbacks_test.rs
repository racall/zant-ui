//! Child components call their callback props synchronously (`WP-TSX-BUGS`).
//!
//! `zant-tsx` generates a child handler that runs a parent callback prop inside
//! the same action, passing the runtime context through
//! (`fn _on_click1(&mut self, cx: &mut ui::ActionCx<'_>)` →
//! `(self.props.onDelete)(cx, id)`). Zig's `tsx.Callback` carried the component
//! pointer and called the callback directly; the `ui::effect` queue cannot be
//! used from an event handler because `ui::begin_frame` clears it before
//! `run_effects`.
//!
//! This drives the generated shape end to end (hand-written, mirroring
//! `NoteCard.tsx` from the notes example): the click must change the root
//! state in the same dispatch, and the next frame must show it.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use zant::runtime::Runtime;
use zant::ui::actions::{Action, ActionCx};
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::local_state;
use zant::ui::options::{BoxOptions, ButtonOptions, TextOptions};
use zant::ui::{button, column, text};

/// The generated callback prop type for `(id: u32) => void`.
type DeleteCallback = Rc<dyn Fn(&mut ActionCx<'_>, u32)>;

/// The child's props: a value and a callback prop (the generated
/// `std::rc::Rc<dyn Fn(&mut ui::ActionCx<'_>, u32)>`).
#[derive(Clone)]
struct CardProps {
    id: u32,
    on_delete: DeleteCallback,
}

/// `NoteCard`: local state slot, props and the `Weak` upgrade the generated
/// action uses.
struct Card {
    props: CardProps,
    _state: Weak<RefCell<Card>>,
}

impl Default for Card {
    fn default() -> Self {
        Card {
            props: CardProps {
                id: 0,
                on_delete: Rc::new(|_, _| {}),
            },
            _state: Weak::new(),
        }
    }
}

impl Card {
    fn build(props: CardProps, id: u64) -> Node {
        let state: Rc<RefCell<Card>> = local_state(id, Card::default);
        let mut instance = state.borrow_mut();
        instance.props = props;
        instance._state = Rc::downgrade(&state);
        instance.render()
    }

    fn render(&mut self) -> Node {
        let state = self._state.clone();
        button(ButtonOptions {
            key: Some("delete".into()),
            text: "Delete".into(),
            on_click: Some(Action::new(move |cx| {
                if let Some(state) = state.upgrade() {
                    state.borrow_mut().on_click(cx);
                }
            })),
            ..Default::default()
        })
    }

    fn on_click(&mut self, cx: &mut ActionCx<'_>) {
        (self.props.on_delete)(cx, self.props.id);
    }
}

struct Notes {
    items: Vec<u32>,
}

impl App for Notes {
    fn render(&mut self) -> Node {
        let card = Card::build(
            CardProps {
                id: 7,
                on_delete: Rc::new(move |cx: &mut ActionCx<'_>, id| {
                    cx.state::<Notes>().items.retain(|item| *item != id);
                }),
            },
            1,
        );
        let summary = if self.items.is_empty() {
            "empty"
        } else {
            "one note"
        };
        column(BoxOptions {
            children: zant::nodes![
                card,
                text(
                    summary,
                    TextOptions {
                        key: Some("summary".into()),
                        ..Default::default()
                    }
                ),
            ],
            ..Default::default()
        })
    }
}

fn frame(runtime: &mut Runtime, renderer: &mut zant::renderer::renderer::Renderer) {
    let canvas = renderer.begin(1.0).expect("canvas");
    runtime.render_frame(canvas, 300.0, 200.0, 1.0);
    renderer.end();
}

#[test]
fn a_child_callback_prop_runs_in_the_same_dispatch() {
    let mut runtime = Runtime::init(Notes { items: vec![7, 8] }, RunOptions::default());
    let mut renderer = zant::renderer::renderer::Renderer::create_raster(300, 200).expect("raster");
    frame(&mut runtime, &mut renderer);
    let id = runtime.find_key("delete").expect("delete button");
    let bounds = runtime.tree.get(id).bounds;
    let point = zant::core::events::PointerEvent {
        x: bounds.x + bounds.width * 0.5,
        y: bounds.y + bounds.height * 0.5,
        ..Default::default()
    };
    runtime.dispatch(zant::core::events::Event::PointerDown(point));
    runtime.dispatch(zant::core::events::Event::PointerUp(point));
    assert_eq!(
        runtime.app_state_mut::<Notes>().items,
        vec![8],
        "the child's handler must reach the parent callback synchronously"
    );
    assert!(runtime.needs_build, "the click asks for a rebuild");
    frame(&mut runtime, &mut renderer);
    let summary = runtime.find_key("summary").expect("summary text");
    match &runtime.tree.get(summary).data {
        zant::core::node::NodeData::Text(data) => {
            assert_eq!(data.content.to_string(), "one note");
        }
        _ => panic!("expected text"),
    }
}
