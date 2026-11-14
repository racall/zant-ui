//! The animation tick.
//!
//! Animation frames only repaint: `Tree::poke` queues the nodes whose state
//! changed and [`Runtime::tick`] advances only those (never the whole tree).
//! Container hover / scroll animations, the button transitions and the
//! `components/{input,toggle,select}` transitions advance here; the caret blink
//! of the focused input is driven from `tick` as well.
//!
//! Exit transitions that finish detach and free the node here.
#![forbid(unsafe_code)]

use crate::components::{button, input, select, toggle};
use crate::core::node::{NodeData, NodeId, NodeKind};
use crate::motion::easing::Easing;
use crate::motion::motion::{self, Fx, Transition};

use super::Runtime;

impl Runtime {
    /// Whether `id` is in the animation queue (tests).
    #[cfg(test)]
    pub(crate) fn animating_contains(&self, id: NodeId) -> bool {
        self.tree.animating.contains(&id)
    }

    /// Advances one node's transitions; true while it keeps animating.
    fn animate_node(&mut self, id: NodeId) -> bool {
        let now = self.now_ms;
        let theme = self.theme;
        let mut active = match self.tree.get(id).kind() {
            NodeKind::Container => self.animate_container(id, now),
            NodeKind::Button => {
                if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                    button::animate(data, now, theme)
                } else {
                    false
                }
            }
            NodeKind::Input => {
                if let NodeData::Input(data) = &mut self.tree.get_mut(id).data {
                    input::animate(data, now, theme)
                } else {
                    false
                }
            }
            NodeKind::Toggle => {
                if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                    toggle::animate(data, now, theme)
                } else {
                    false
                }
            }
            NodeKind::Select => {
                if let NodeData::Select(data) = &mut self.tree.get_mut(id).data {
                    select::animate(data, now, theme)
                } else {
                    false
                }
            }
            _ => false,
        };
        // presence (enter / exit) and FLIP layout transitions
        let node = self.tree.get_mut(id);
        if let Some(anim) = node.anim.as_mut() {
            let kind = node.motion.presence;
            if anim.presence.done(now) {
                anim.fx = if node.exiting {
                    motion::presence_fx(kind, 0.0)
                } else {
                    Fx::default()
                };
            } else {
                anim.fx = motion::presence_fx(kind, anim.presence.value(now));
                active = true;
            }
            if anim.flip_active {
                let k = anim.flip.value(now);
                for (delta, from) in anim.flip_delta.iter_mut().zip(anim.flip_from.iter()) {
                    *delta = from * k;
                }
                if anim.flip.done(now) {
                    anim.flip_active = false;
                    anim.flip_delta = [0.0; 4];
                } else {
                    active = true;
                }
            }
        }
        active
    }

    /// The container share of a node's animation: hover look and the animated
    /// `scroll_into_view` offset.
    fn animate_container(&mut self, id: NodeId, now: i64) -> bool {
        let mid = Transition {
            duration_ms: self.theme.motion_duration_mid as u16,
            easing: Easing::EaseInOut,
            delay_ms: 0,
        };
        let (hover_active, scroll, scroll_done) = {
            let NodeData::Container(data) = &mut self.tree.get_mut(id).data else {
                return false;
            };
            let mut running = false;
            if data.hover_visual.is_some() {
                data.hover
                    .retarget(if data.hovered { 1.0 } else { 0.0 }, now, mid);
                data.hover_t = data.hover.value(now);
                running = !data.hover.done(now);
            }
            if !data.scroll_animating {
                (running, None, false)
            } else {
                let target = data.scroll_anim.value(now);
                let done = data.scroll_anim.done(now);
                (running, Some(target - data.scroll_offset), done)
            }
        };
        if let Some(delta) = scroll {
            self.tree.scroll_by(id, delta);
            if scroll_done && let NodeData::Container(data) = &mut self.tree.get_mut(id).data {
                data.scroll_animating = false;
            }
        }
        hover_active || (scroll.is_some() && !scroll_done)
    }

    /// Advances the queued animations, frees finished exit
    /// transitions, blinks the focused input's caret and reveals pending
    /// tooltips. Sets `any_animation` for the frame scheduler.
    pub(crate) fn tick(&mut self) {
        // only nodes whose state changed (`Tree::poke`) are visited
        let mut finished: Vec<NodeId> = Vec::new();
        let mut index = 0usize;
        while index < self.tree.animating.len() {
            let id = self.tree.animating[index];
            let alive = self.tree.get(id).alive;
            let active = alive && self.animate_node(id);
            if alive {
                self.tree.mark_dirty(id, false, true);
            }
            if active {
                index += 1;
            } else {
                if let Some(node) = self.tree.nodes.get_mut(id as usize) {
                    node.anim_queued = false;
                }
                self.tree.animating.swap_remove(index);
                if self.tree.get(id).alive && self.tree.get(id).exiting {
                    finished.push(id);
                }
            }
        }
        // exit transitions that ended: detach and free
        for id in finished {
            let (alive, exiting) = {
                let node = self.tree.get(id);
                (node.alive, node.exiting)
            };
            if !alive || !exiting {
                continue;
            }
            if let Some(parent) = self.tree.get(id).parent {
                if let Some(at) = self
                    .tree
                    .get(parent)
                    .children
                    .iter()
                    .position(|&child| child == id)
                {
                    self.tree.get_mut(parent).children.remove(at);
                }
                self.tree.mark_dirty(parent, false, true);
            }
            self.remove_subtree(id);
        }
        // caret blink of the focused input
        if let Some(id) = self.tree.focused {
            let (enabled, is_input) = {
                let node = self.tree.get(id);
                (node.enabled, matches!(node.data, NodeData::Input(_)))
            };
            if enabled && is_input {
                let changed = {
                    let node = self.tree.get_mut(id);
                    if let NodeData::Input(data) = &mut node.data {
                        super::ime::update_blink(data, node.enabled, self.now_ms)
                    } else {
                        false
                    }
                };
                if changed {
                    self.tree.mark_dirty(id, false, true);
                }
            }
        }
        if self.tooltip_target.is_some()
            && !self.tooltip_visible
            && self.now_ms >= self.tooltip_show_at
        {
            self.show_tooltip();
        }
        let mut tooltip_moving = false;
        if self.tooltip_visible || self.tooltip_hiding {
            if self.tooltip_anim.done(self.now_ms) {
                if self.tooltip_hiding {
                    self.drop_tooltip_layout();
                }
            } else {
                tooltip_moving = true;
            }
            self.tree.mark_paint_dirty();
        }
        self.any_animation = !self.tree.animating.is_empty() || tooltip_moving;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::{Event, Modifiers, PointerButton, PointerEvent};
    use crate::core::node::FlexStyle;
    use crate::motion::motion::Presence;
    use crate::renderer::renderer::Renderer;
    use crate::ui::app::App;
    use crate::ui::builders::{column, input};
    use crate::ui::options::{BoxOptions, InputOptions};
    use crate::ui::{Node, RunOptions, Style, styled, with_key};
    use std::sync::atomic::{AtomicI64, Ordering};

    fn pointer(x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            x,
            y,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        }
    }

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 300.0, 200.0, 1.0);
        renderer.end();
    }

    #[test]
    fn container_hover_transition_only_repaints() {
        motion::set_enabled(true);
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        fn clock() -> i64 {
            NOW.load(Ordering::Relaxed)
        }
        struct Hover;
        impl App for Hover {
            fn render(&mut self) -> Node {
                column(BoxOptions {
                    children: crate::nodes![column(BoxOptions {
                        width: Some(100.0),
                        height: Some(50.0),
                        hover_background: Some(0xffcc_cccc),
                        ..Default::default()
                    })],
                    ..Default::default()
                })
            }
        }
        NOW.store(1_000, Ordering::Relaxed);
        let mut runtime = Runtime::init(Hover, RunOptions::default());
        runtime.clock = clock;
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let inner = runtime
            .tree
            .nodes
            .iter()
            .find(|node| {
                node.alive
                    && matches!(&node.data, NodeData::Container(data) if data.hover_visual.is_some())
            })
            .map(|node| node.id)
            .expect("the hover container");
        runtime.dispatch(Event::PointerMove(pointer(20.0, 20.0)));
        assert_eq!(runtime.hover_box, Some(inner));
        assert!(!runtime.needs_build, "the hover look is paint-only");
        // the first animated frame starts the 200 ms transition
        frame(&mut runtime, &mut renderer);
        assert!(runtime.animating_contains(inner));
        assert!(runtime.any_animation);
        assert_eq!(runtime.next_delay_ms, 16);
        NOW.store(1_400, Ordering::Relaxed);
        frame(&mut runtime, &mut renderer);
        let hover_t = match &runtime.tree.get(inner).data {
            NodeData::Container(data) => data.hover_t,
            _ => panic!("expected the hover container"),
        };
        assert!((hover_t - 1.0).abs() < 0.01, "hover_t settled at {hover_t}");
        assert!(!runtime.any_animation);
        assert_eq!(runtime.next_delay_ms, -1);
        assert!(!runtime.needs_build, "animations never rebuild");
    }

    #[test]
    fn exit_transition_keeps_the_node_until_it_ends_then_frees_it() {
        motion::set_enabled(true);
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        fn clock() -> i64 {
            NOW.load(Ordering::Relaxed)
        }
        struct Panel {
            show: bool,
        }
        impl App for Panel {
            fn render(&mut self) -> Node {
                let panel = if self.show {
                    Some(with_key(
                        styled(
                            column(BoxOptions {
                                width: Some(50.0),
                                height: Some(20.0),
                                ..Default::default()
                            }),
                            Style {
                                motion: Some(Presence::Fade),
                                ..Default::default()
                            },
                        ),
                        "panel",
                    ))
                } else {
                    None
                };
                column(BoxOptions {
                    children: crate::nodes![panel],
                    ..Default::default()
                })
            }
        }
        NOW.store(1_000, Ordering::Relaxed);
        let mut runtime = Runtime::init(Panel { show: true }, RunOptions::default());
        runtime.clock = clock;
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let id = runtime.find_key("panel").expect("panel");
        // hide it: the exit transition starts and the node stays, out of flow
        runtime.app_state_mut::<Panel>().show = false;
        runtime.needs_build = true;
        frame(&mut runtime, &mut renderer);
        let exiting = runtime.tree.get(id).exiting;
        assert!(exiting, "the node is playing its exit transition");
        assert!(!runtime.tree.get(id).in_flow());
        assert!(runtime.animating_contains(id));
        assert_eq!(runtime.next_delay_ms, 16);
        NOW.store(1_400, Ordering::Relaxed);
        frame(&mut runtime, &mut renderer);
        assert!(!runtime.tree.get(id).alive, "the exit transition freed it");
        assert!(runtime.find_key("panel").is_none());
        assert!(!runtime.any_animation);
    }

    #[test]
    fn flippy_helper_animation_values_are_exposed_for_tests() {
        // `Animated::at` (motion primitive) is what a settled transition is.
        let settled = crate::motion::motion::Animated::at(1.0);
        assert!(settled.done(0));
        assert_eq!(settled.value(0), 1.0);
        let _ = FlexStyle::default();
    }

    #[test]
    fn the_focused_input_advances_its_transitions_and_caret_blink() {
        motion::set_enabled(true);
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        fn clock() -> i64 {
            NOW.load(Ordering::Relaxed)
        }
        struct Field;
        impl App for Field {
            fn render(&mut self) -> Node {
                column(BoxOptions {
                    children: crate::nodes![input(InputOptions {
                        key: Some("name".into()),
                        value: Some("hi".into()),
                        ..Default::default()
                    })],
                    ..Default::default()
                })
            }
        }
        NOW.store(1_000, Ordering::Relaxed);
        let mut runtime = Runtime::init(Field, RunOptions::default());
        runtime.clock = clock;
        let mut renderer = Renderer::create_raster(300, 200).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let id = runtime.find_key("name").expect("input");
        let bounds = runtime.tree.get(id).bounds;
        let point = pointer(bounds.x + 10.0, bounds.y + bounds.height * 0.5);
        runtime.dispatch(Event::PointerMove(point));
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
        assert_eq!(runtime.tree.focused, Some(id));
        // the first frame starts the hover / focus transition
        frame(&mut runtime, &mut renderer);
        assert!(runtime.animating_contains(id));
        let focus_t = match &runtime.tree.get(id).data {
            NodeData::Input(data) => data.focus_t,
            _ => panic!("expected an input"),
        };
        assert!(focus_t < 1.0, "the focus border is still easing in");
        // the 200 ms antd transition settles
        NOW.store(
            1_000 + i64::from(runtime.theme.motion_duration_mid),
            Ordering::Relaxed,
        );
        frame(&mut runtime, &mut renderer);
        let (hover_t, focus_t) = match &runtime.tree.get(id).data {
            NodeData::Input(data) => (data.hover_t, data.focus_t),
            _ => panic!("expected an input"),
        };
        assert!((hover_t - 1.0).abs() < 0.01, "hover settled at {hover_t}");
        assert!((focus_t - 1.0).abs() < 0.01, "focus settled at {focus_t}");
        assert!(!runtime.any_animation);
        // only the caret blink is scheduled now (first tick armed it at 1_530)
        let blink_remaining = crate::components::input::BLINK_INTERVAL_MS
            - i64::from(runtime.theme.motion_duration_mid);
        assert_eq!(runtime.next_delay_ms, blink_remaining as i32);
        // the blink flips at the interval and repaints
        NOW.store(
            1_000 + crate::components::input::BLINK_INTERVAL_MS,
            Ordering::Relaxed,
        );
        frame(&mut runtime, &mut renderer);
        let blink_on = match &runtime.tree.get(id).data {
            NodeData::Input(data) => data.blink_on,
            _ => panic!("expected an input"),
        };
        assert!(!blink_on, "the caret blinked off");
        assert_eq!(
            runtime.next_delay_ms,
            crate::components::input::BLINK_INTERVAL_MS as i32
        );
    }
}
