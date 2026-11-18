//! Motion: the pure primitives and their runtime integration.
//!
//! The first four cases are the pure motion primitives. The second half is the
//! runtime integration (`button hover …`, `with motion disabled state changes
//! …`, `presence: enter, exit out of flow, free, revive`, `segmented thumb
//! slides with FLIP …`, `ui.focus and ui.scrollIntoView …`, `ui.wakeAfter
//! rebuilds …`) plus the rest of the motion wiring:
//!
//! * presence enter / exit lifetime and freeing (`presence: …` and
//!   `modal_and_message_presence_…`),
//! * FLIP bounds interpolation and `layout_transition` (`segmented …`),
//! * `next_frame_delay` before / after transitions settle
//!   (`next_frame_delay_tracks_pending_animations`),
//! * motion off — what `ui::app::run` does for `ZANT_NO_MOTION=1`,
//!   `ZANT_SNAPSHOT` and the system Reduce Motion setting — makes presence
//!   exits immediate (`snapshot_and_reduce_motion_disable_transitions`).
//!
//! Everything runs headless on a raster canvas with a fake clock (a
//! module-level timestamp); there are no sleeps.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, MutexGuard};

use zant::core::events::{Event, Modifiers, PointerButton, PointerEvent};
use zant::core::node::{NodeData, NodeId};
use zant::motion::easing::{self, Easing};
use zant::motion::motion::{
    Animated, Origin, Presence, Transition, enabled, mix_color, presence_fx, presence_origin,
    set_enabled,
};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::ui::{
    self, App, BoxOptions, ButtonOptions, MessageOptions, ModalOptions, Node, RunOptions,
    SegmentedOption, SegmentedOptions, Style, TextOptions, button, column, inset, message, modal,
    segmented, styled, text,
};

mod buttons;
mod curves;
mod presence;
mod widgets;

use self::presence::*;

/// Tests here mutate the process-wide `motion.enabled` switch, so serialise
/// them (the test harness runs tests on several threads).
static MOTION_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    MOTION_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn linear(duration_ms: u16) -> Transition {
    Transition {
        duration_ms,
        easing: Easing::Linear,
        ..Default::default()
    }
}

struct RestoreEnabled(bool);

impl Drop for RestoreEnabled {
    fn drop(&mut self) {
        set_enabled(self.0);
    }
}

/// The fake timestamp in milliseconds (starts at 1000).
static FAKE_NOW: AtomicI64 = AtomicI64::new(1_000);

fn fake_clock() -> i64 {
    FAKE_NOW.load(Ordering::Relaxed)
}

/// Advances the fake clock by `ms`.
fn advance(ms: i64) {
    FAKE_NOW.fetch_add(ms, Ordering::Relaxed);
}

fn pointer(x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        x,
        y,
        button: PointerButton::Left,
        modifiers: Modifiers::default(),
    }
}

struct Harness {
    runtime: Runtime,
    renderer: Renderer,
}

impl Harness {
    fn init(app: impl App) -> Harness {
        FAKE_NOW.store(1_000, Ordering::Relaxed);
        let mut runtime = Runtime::init(app, RunOptions::default());
        runtime.clock = fake_clock;
        Harness {
            runtime,
            renderer: Renderer::create_raster(400, 300).expect("raster renderer"),
        }
    }

    fn frame(&mut self) {
        self.frame_at(400.0, 300.0);
    }

    fn frame_at(&mut self, width: f32, height: f32) {
        let canvas = self.renderer.begin(1.0).expect("frame canvas");
        self.runtime.render_frame(canvas, width, height, 1.0);
        self.renderer.end();
    }

    /// Any live node with the key (exiting nodes included).
    fn find(&self, key: &str) -> NodeId {
        self.find_alive(key)
            .unwrap_or_else(|| panic!("key not found: {key}"))
    }

    /// The node while it is alive, `None` once it was freed.
    fn find_alive(&self, key: &str) -> Option<NodeId> {
        self.runtime
            .tree
            .nodes
            .iter()
            .find(|node| node.alive && node.key.as_deref() == Some(key))
            .map(|node| node.id)
    }

    /// A copy of the node's animation state (the borrowed node cannot outlive
    /// the frame).
    fn anim(&self, id: NodeId) -> zant::core::node::NodeAnim {
        self.runtime
            .tree
            .get(id)
            .anim
            .expect("Zant UI: the node has animation state")
    }
}
