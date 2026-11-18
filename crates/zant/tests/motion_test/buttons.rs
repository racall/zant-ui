//! Button hover transitions over time.

use super::*;

pub(super) fn button_hover_t(runtime: &Runtime, id: NodeId) -> f32 {
    match &runtime.tree.get(id).data {
        NodeData::Button(data) => data.hover_t,
        _ => panic!("expected a button"),
    }
}

pub(super) struct ButtonState;

impl App for ButtonState {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            padding: inset(20.0),
            children: zant::nodes![button(ButtonOptions {
                key: Some("b".into()),
                text: "Hover me".into(),
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn button_hover_eases_over_time_and_then_stops_animating() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(ButtonState);
    h.frame();
    let button = h.find("b");
    let bounds = h.runtime.tree.get(button).bounds;
    h.runtime
        .dispatch(Event::PointerMove(pointer(bounds.x + 10.0, bounds.y + 8.0)));
    h.frame();
    assert_eq!(button_hover_t(&h.runtime, button), 0.0);
    advance(100);
    h.frame();
    let mid = button_hover_t(&h.runtime, button);
    assert!(mid > 0.0 && mid < 1.0, "hover ease midway: {mid}");
    assert_eq!(h.runtime.next_delay_ms, 16);
    advance(150);
    h.frame();
    assert_eq!(button_hover_t(&h.runtime, button), 1.0);
    assert!(h.runtime.tree.animating.is_empty());
    assert_eq!(h.runtime.next_delay_ms, -1);
}

#[test]
fn with_motion_disabled_state_changes_apply_at_once() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(false);
    let mut h = Harness::init(ButtonState);
    h.frame();
    let button = h.find("b");
    let bounds = h.runtime.tree.get(button).bounds;
    h.runtime
        .dispatch(Event::PointerMove(pointer(bounds.x + 10.0, bounds.y + 8.0)));
    h.frame();
    assert_eq!(button_hover_t(&h.runtime, button), 1.0);
}
