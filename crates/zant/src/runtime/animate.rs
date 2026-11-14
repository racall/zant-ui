//! Port of `src/ui_runtime.zig:1129-1218`: the animation tick.
//!
//! Animation frames only repaint: `Tree::poke` queues the nodes whose state
//! changed and [`Runtime::tick`] advances only those (never the whole tree).
//! Container hover / scroll animations and the button transitions are advanced
//! here; the input / toggle / select component transitions belong to
//! `components/{input,toggle,select}.zig` (WP-2.1 / WP-2.2) and are not ported
//! yet, so those nodes simply settle.
//!
//! Exit transitions that finish detach and free the node here (Zig did it in
//! `tick` as well).
#![forbid(unsafe_code)]

use crate::components::button;
use crate::core::node::{NodeData, NodeId};
use crate::motion::easing::Easing;
use crate::motion::motion::{self, Fx, Transition};

use super::Runtime;

impl Runtime {
    /// Advances one node's transitions; true while it keeps animating.
    fn animate_node(&mut self, id: NodeId) -> bool {
        let now = self.now_ms;
        let mut active = match self.tree.get(id).kind() {
            crate::core::node::NodeKind::Container => self.animate_container(id, now),
            crate::core::node::NodeKind::Button => {
                let theme = self.theme;
                if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                    button::animate(data, now, theme)
                } else {
                    false
                }
            }
            // TODO(WP-2.1 / WP-2.2): components::input / toggle / select
            // `animate` are not ported yet (their paint/layout is missing too).
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

    /// The container share of `animateNode`: hover look and the animated
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

    /// Zig `tick`: advances the queued animations, frees finished exit
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
