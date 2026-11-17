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

#[test]
fn every_easing_curve_maps_0_to_0_and_1_to_1() {
    let _guard = lock();
    for e in Easing::ALL {
        assert!((0.0_f32 - easing::apply(e, 0.0)).abs() <= 1e-4);
        assert!((1.0_f32 - easing::apply(e, 1.0)).abs() <= 1e-4);
    }
    // antd ease-in-out is near-symmetric (y2 = 1, so slightly above 0.5)
    assert!((0.5_f32 - easing::apply(Easing::EaseInOut, 0.5)).abs() <= 0.02);
    // reference value: CSS `ease` at 50% ≈ 0.8024
    assert!((0.8024_f32 - easing::apply(Easing::Ease, 0.5)).abs() <= 2e-3);
    // back-out overshoots
    let mut max: f32 = 0.0;
    let mut i: usize = 1;
    while i < 100 {
        max = max.max(easing::apply(Easing::EaseOutBack, i as f32 / 100.0));
        i += 1;
    }
    assert!(max > 1.0);
    // ease-out is ahead of linear early on
    assert!(easing::apply(Easing::EaseOut, 0.3) > 0.3);
}

#[test]
fn animated_values_follow_the_clock() {
    let _guard = lock();
    let mut a = Animated::start(0.0, 10.0, 1000, linear(200));
    assert_eq!(a.value(1000), 0.0);
    assert!((a.value(1100) - 5.0).abs() <= 1e-3);
    assert!(!a.done(1100));
    assert_eq!(a.value(1200), 10.0);
    assert!(a.done(1200));

    // delay holds the start value
    let d = Animated::start(
        0.0,
        1.0,
        0,
        Transition {
            duration_ms: 100,
            delay_ms: 50,
            easing: Easing::Linear,
        },
    );
    assert_eq!(d.value(40), 0.0);
    assert!((d.value(100) - 0.5).abs() <= 1e-3);
    assert!(d.done(150));

    // retargeting mid-flight continues from the current value
    a = Animated::start(0.0, 1.0, 0, linear(200));
    let mid = a.value(100);
    a.retarget(0.0, 100, linear(200));
    assert!((mid - a.value(100)).abs() <= 1e-3);
    assert!(a.duration_ms < 200);
    assert!(a.done(100 + 200));
    // retarget to the same target is a no-op
    let before = a;
    a.retarget(0.0, 150, Transition::default());
    assert_eq!(before.start_ms, a.start_ms);
}

#[test]
fn disabled_motion_settles_immediately() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(false);
    let a = Animated::start(
        0.0,
        1.0,
        0,
        Transition {
            duration_ms: 300,
            ..Default::default()
        },
    );
    assert_eq!(a.value(0), 1.0);
    assert!(a.done(0));
}

#[test]
fn presence_transforms() {
    let _guard = lock();
    let hidden = presence_fx(Presence::SlideUp, 0.0);
    assert_eq!(hidden.opacity, 0.0);
    assert!((0.8_f32 - hidden.scale_y).abs() <= 1e-4);
    assert!(presence_fx(Presence::ZoomBig, 1.0).is_identity());
    assert!((-8.0_f32 - presence_fx(Presence::MoveUp, 0.0).dy).abs() <= 1e-4);
    assert_eq!(
        presence_origin(Presence::SlideUp, Origin::TopRight),
        Origin::TopRight
    );
    assert_eq!(mix_color(0xff000000, 0xffffffff, 0.502), 0xff808080);
}

// ---- runtime integration (fake clock) ----
//
// `Harness`: a runtime with a fake clock, a 400×300 raster canvas and
// `find(key)` over the retained tree. The fake timestamp is module state and
// every `Harness::init` resets it to 1000, so tests advance it explicitly
// between frames and never sleep.

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

fn button_hover_t(runtime: &Runtime, id: NodeId) -> f32 {
    match &runtime.tree.get(id).data {
        NodeData::Button(data) => data.hover_t,
        _ => panic!("expected a button"),
    }
}

struct ButtonState;

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

struct Panel {
    show: bool,
}

impl App for Panel {
    fn render(&mut self) -> Node {
        let panel = styled(
            column(BoxOptions {
                key: Some("panel".into()),
                width: Some(120.0),
                height: Some(40.0),
                ..Default::default()
            }),
            Style {
                motion: Some(Presence::SlideUp),
                block_pointer: Some(true),
                background: Some(0xffeeeeee),
                ..Default::default()
            },
        );
        let a = column(BoxOptions {
            key: Some("a".into()),
            width: Some(50.0),
            height: Some(20.0),
            ..Default::default()
        });
        let b = column(BoxOptions {
            key: Some("b".into()),
            width: Some(50.0),
            height: Some(20.0),
            ..Default::default()
        });
        let children: Vec<Node> = if self.show {
            vec![a, panel, b]
        } else {
            vec![a, b]
        };
        column(BoxOptions {
            padding: inset(10.0),
            children,
            ..Default::default()
        })
    }
}

#[test]
fn presence_enter_exit_out_of_flow_free_revive() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(Panel { show: false });
    h.frame();
    let b_y_without = h.runtime.tree.get(h.find("b")).bounds.y;

    // enter
    h.runtime.app_state_mut::<Panel>().show = true;
    h.runtime.invalidate();
    h.frame();
    let panel_id = h.find("panel");
    assert_eq!(
        h.runtime
            .tree
            .get(panel_id)
            .anim
            .expect("enter anim")
            .fx
            .opacity,
        0.0
    );
    advance(100);
    h.frame();
    let opacity = h
        .runtime
        .tree
        .get(panel_id)
        .anim
        .expect("enter anim")
        .fx
        .opacity;
    assert!(opacity > 0.0 && opacity < 1.0, "presence midway: {opacity}");
    assert_eq!(h.runtime.next_delay_ms, 16);
    advance(150);
    h.frame();
    assert!(
        h.runtime
            .tree
            .get(panel_id)
            .anim
            .expect("enter anim")
            .fx
            .is_identity()
    );
    assert!(h.runtime.tree.get(h.find("b")).bounds.y > b_y_without);

    // exit: still a child, out of flow and not hittable
    h.runtime.app_state_mut::<Panel>().show = false;
    h.runtime.invalidate();
    h.frame();
    assert!(
        h.runtime.tree.get(panel_id).alive && h.runtime.tree.get(panel_id).exiting,
        "the node plays its exit transition"
    );
    let parent = h
        .runtime
        .tree
        .get(panel_id)
        .parent
        .expect("the exiting node keeps its parent");
    assert!(
        h.runtime.tree.get(parent).children.contains(&panel_id),
        "the exiting node stays a child while it paints"
    );
    let c = h.runtime.tree.get(panel_id).bounds;
    assert_ne!(
        h.runtime
            .tree
            .hit_test(c.x + c.width / 2.0, c.y + c.height / 2.0),
        Some(panel_id),
        "an exiting node is out of hit testing"
    );
    // ... but it still paints: the frame shows its own background while the
    // exit transition starts at full opacity (the box is the old one, because
    // an exiting node is out of layout)
    let painted = h
        .renderer
        .pixel_at(
            (c.x + c.width * 0.75) as usize,
            (c.y + c.height * 0.5) as usize,
        )
        .expect("pixel inside the exiting node");
    assert_eq!(
        painted & 0x00ff_ffff,
        0x00ee_eeee,
        "the exiting node keeps painting during its transition"
    );
    assert_eq!(h.runtime.tree.get(h.find("b")).bounds.y, b_y_without);
    assert_eq!(h.runtime.next_delay_ms, 16);

    // revive mid-exit keeps the same node
    advance(100);
    h.frame();
    h.runtime.app_state_mut::<Panel>().show = true;
    h.runtime.invalidate();
    h.frame();
    assert_eq!(h.find("panel"), panel_id);
    assert!(!h.runtime.tree.get(panel_id).exiting);
    advance(300);
    h.frame();
    assert!(
        h.runtime
            .tree
            .get(panel_id)
            .anim
            .expect("enter anim")
            .fx
            .is_identity()
    );

    // exit fully: freed after the transition
    h.runtime.app_state_mut::<Panel>().show = false;
    h.runtime.invalidate();
    h.frame();
    advance(250);
    h.frame();
    assert!(
        h.find_alive("panel").is_none(),
        "the exit transition freed it"
    );
    assert!(
        h.runtime.tree.free_ids.contains(&panel_id),
        "the freed id is recycled"
    );
    assert!(h.runtime.tree.animating.is_empty());
}

#[test]
fn next_frame_delay_tracks_pending_animations() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(Panel { show: true });
    h.frame();
    let panel_id = h.find("panel");
    // the enter transition started on this frame: a frame is scheduled
    assert!(h.runtime.tree.animating.contains(&panel_id));
    assert_eq!(h.runtime.next_delay_ms, 16);
    advance(250);
    h.frame();
    // settled: no frame timer and no queued nodes
    assert!(h.runtime.tree.animating.is_empty());
    assert_eq!(h.runtime.next_delay_ms, -1);

    // the exit transition schedules frames again until it frees the node
    h.runtime.app_state_mut::<Panel>().show = false;
    h.runtime.invalidate();
    h.frame();
    assert_eq!(h.runtime.next_delay_ms, 16);
    advance(250);
    h.frame();
    assert!(h.find_alive("panel").is_none());
    assert_eq!(h.runtime.next_delay_ms, -1);
}

struct SegState {
    selected: usize,
}

impl App for SegState {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            padding: inset(10.0),
            align_items: zant::flex::Align::Center,
            grow: 1.0,
            children: zant::nodes![segmented(SegmentedOptions {
                key: Some("seg".into()),
                options: vec![
                    SegmentedOption {
                        label: "One".into(),
                        ..Default::default()
                    },
                    SegmentedOption {
                        label: "Two".into(),
                        ..Default::default()
                    },
                    SegmentedOption {
                        label: "Three".into(),
                        ..Default::default()
                    },
                ],
                selected: self.selected,
                ..Default::default()
            })],
            ..Default::default()
        })
    }
}

#[test]
fn segmented_thumb_slides_with_flip_resizing_does_not_animate() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(SegState { selected: 0 });
    h.frame();
    let thumb = h.find("seg-thumb");
    assert!(!h.anim(thumb).flip_active);
    let x0 = h.runtime.tree.get(thumb).bounds.x;

    h.runtime.app_state_mut::<SegState>().selected = 2;
    h.runtime.invalidate();
    h.frame();
    assert!(h.runtime.tree.get(thumb).bounds.x > x0);
    assert!(h.anim(thumb).flip_active, "layout transition started");
    // the next frame is scheduled even though the slide started in layout
    assert_eq!(h.runtime.next_delay_ms, 16);
    // painted at the old position on the first frame
    let thumb_bounds = h.runtime.tree.get(thumb).bounds;
    assert!(
        (x0 - (thumb_bounds.x + h.anim(thumb).flip_delta[0])).abs() <= 0.5,
        "the first FLIP frame paints the old box"
    );
    // ... and the raster shows it: the raised (elevated) thumb is still at the
    // old box, the new box is the flat track
    let theme = zant::theme::tokens::ant_light();
    let row_y = (thumb_bounds.y + thumb_bounds.height * 0.5) as usize;
    let old_pixel = h
        .renderer
        .pixel_at((x0 + 2.5) as usize, row_y)
        .expect("pixel at the old thumb box");
    let new_pixel = h
        .renderer
        .pixel_at((thumb_bounds.x + 2.5) as usize, row_y)
        .expect("pixel at the new thumb box");
    assert_eq!(
        old_pixel & 0x00ff_ffff,
        theme.color_bg_elevated & 0x00ff_ffff,
        "the thumb paints at the old position on the first FLIP frame"
    );
    assert_eq!(
        new_pixel & 0x00ff_ffff,
        theme.color_bg_layout & 0x00ff_ffff,
        "the new position is still the flat track"
    );
    advance(150);
    h.frame();
    let d = h.anim(thumb).flip_delta[0];
    let new_x = h.runtime.tree.get(thumb).bounds.x;
    assert!(d < 0.0 && d > x0 - new_x, "the slide interpolates: {d}");
    assert_eq!(h.runtime.next_delay_ms, 16);
    assert!(!h.runtime.needs_build, "a FLIP frame only repaints");
    advance(200);
    h.frame();
    assert_eq!(h.anim(thumb).flip_delta[0], 0.0);
    assert!(!h.anim(thumb).flip_active);
    assert_eq!(h.runtime.next_delay_ms, -1);
    // settled: the raised thumb is at the new box, the old one is flat track
    let settled_old = h
        .renderer
        .pixel_at((x0 + 2.5) as usize, row_y)
        .expect("pixel at the old thumb box");
    let settled_new = h
        .renderer
        .pixel_at((thumb_bounds.x + 2.5) as usize, row_y)
        .expect("pixel at the new thumb box");
    assert_eq!(
        settled_old & 0x00ff_ffff,
        theme.color_bg_layout & 0x00ff_ffff
    );
    assert_eq!(
        settled_new & 0x00ff_ffff,
        theme.color_bg_elevated & 0x00ff_ffff
    );

    // a viewport change re-lays out without starting a transition
    let before_resize = h.runtime.tree.get(thumb).bounds.x;
    h.frame_at(360.0, 300.0);
    assert_ne!(h.runtime.tree.get(thumb).bounds.x, before_resize);
    assert!(!h.anim(thumb).flip_active);
}

struct CmdState {
    renders: usize,
    wake: bool,
}

impl App for CmdState {
    fn render(&mut self) -> Node {
        self.renders += 1;
        if self.wake {
            ui::wake_after(100);
        }
        let rows: Vec<Node> = (0..30)
            .map(|i| {
                column(BoxOptions {
                    key: if i == 29 { Some("far".into()) } else { None },
                    height: Some(40.0),
                    ..Default::default()
                })
            })
            .collect();
        column(BoxOptions {
            grow: 1.0,
            children: zant::nodes![
                zant::ui::input(zant::ui::InputOptions {
                    key: Some("x".into()),
                    ..Default::default()
                }),
                column(BoxOptions {
                    key: Some("list".into()),
                    grow: 1.0,
                    scroll: true,
                    children: rows,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn ui_focus_and_ui_scroll_into_view_run_after_the_next_layout() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(CmdState {
        renders: 0,
        wake: false,
    });
    h.frame();
    ui::focus("x");
    h.frame();
    assert_eq!(h.runtime.tree.focused, Some(h.find("x")));

    ui::scroll_into_view("far");
    h.frame();
    let list = h.find("list");
    assert_eq!(h.runtime.next_delay_ms, 16);
    advance(300);
    h.frame();
    let offset = match &h.runtime.tree.get(list).data {
        NodeData::Container(data) => data.scroll_offset,
        _ => panic!("expected the scroll container"),
    };
    assert!(offset > 0.0, "scrollIntoView animated to {offset}");
    let list_bounds = h.runtime.tree.get(list).bounds;
    let far = h.find("far");
    assert!(
        h.runtime.tree.get(far).bounds.y < list_bounds.y + list_bounds.height,
        "the far row scrolled into view"
    );
}

#[test]
fn ui_wake_after_rebuilds_once_the_time_has_passed() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(CmdState {
        renders: 0,
        wake: true,
    });
    h.frame();
    let n = h.runtime.app_state_mut::<CmdState>().renders;
    assert!(
        h.runtime.next_delay_ms > 0 && h.runtime.next_delay_ms <= 100,
        "wakeAfter schedules the next frame: {}",
        h.runtime.next_delay_ms
    );
    advance(50);
    h.frame();
    assert_eq!(h.runtime.app_state_mut::<CmdState>().renders, n);
    advance(60);
    h.frame();
    assert_eq!(h.runtime.app_state_mut::<CmdState>().renders, n + 1);
}

// ---- modal / message presence and motion off ----

struct OverlayState {
    show: bool,
}

impl App for OverlayState {
    fn render(&mut self) -> Node {
        let children: Vec<Node> = if self.show {
            zant::nodes![
                modal(ModalOptions {
                    title: "Help".into(),
                    content: text("Body", TextOptions::default()),
                    ..Default::default()
                }),
                message(MessageOptions {
                    text: "Saved".into(),
                    ..Default::default()
                }),
            ]
        } else {
            Vec::new()
        };
        column(BoxOptions {
            children,
            ..Default::default()
        })
    }
}

#[test]
fn modal_and_message_presence_work_end_to_end_headless() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    set_enabled(true);
    let mut h = Harness::init(OverlayState { show: false });
    h.frame();

    // enter: antd fade (overlay), zoom-big (card) and move-up (message)
    h.runtime.app_state_mut::<OverlayState>().show = true;
    h.runtime.invalidate();
    h.frame();
    let overlay = h.find("modal");
    let card = h.find("modal-card");
    let pill = h.find("message");
    assert_eq!(h.anim(overlay).fx.opacity, 0.0, "fade starts hidden");
    let card_fx = h.anim(card).fx;
    assert_eq!(card_fx.opacity, 0.0);
    assert!((card_fx.scale_x - 0.8).abs() <= 1e-4, "zoom starts at 0.8");
    let pill_fx = h.anim(pill).fx;
    assert_eq!(pill_fx.opacity, 0.0);
    assert!((pill_fx.dy + 8.0).abs() <= 1e-4, "move-up starts 8 px low");
    assert_eq!(h.runtime.next_delay_ms, 16);

    advance(100);
    h.frame();
    let overlay_opacity = h.anim(overlay).fx.opacity;
    assert!(overlay_opacity > 0.0 && overlay_opacity < 1.0);
    let card_scale = h.anim(card).fx.scale_x;
    assert!(card_scale > 0.8 && card_scale < 1.0);
    let pill_dy = h.anim(pill).fx.dy;
    assert!(pill_dy < 0.0 && pill_dy > -8.0);

    advance(200);
    h.frame();
    assert!(h.anim(overlay).fx.is_identity());
    assert!(h.anim(card).fx.is_identity());
    assert!(h.anim(pill).fx.is_identity());
    assert!(h.runtime.tree.animating.is_empty());
    assert_eq!(h.runtime.next_delay_ms, -1);
    // the modal is up: the mask takes clicks outside the card and the card
    // takes input inside its box
    let mask = h.find("modal-mask");
    assert_eq!(
        h.runtime.tree.hit_test(2.0, 2.0),
        Some(mask),
        "the mask dismisses clicks beside the card"
    );
    let card_bounds = h.runtime.tree.get(card).bounds;
    assert_eq!(
        h.runtime.tree.hit_test(
            card_bounds.x + card_bounds.width / 2.0,
            card_bounds.y + card_bounds.height / 2.0
        ),
        Some(card),
        "the card takes input while the modal is open"
    );

    // exit: both overlays keep painting while leaving, then are freed
    h.runtime.app_state_mut::<OverlayState>().show = false;
    h.runtime.invalidate();
    h.frame();
    assert!(h.runtime.tree.get(overlay).exiting);
    assert!(h.runtime.tree.get(pill).exiting);
    assert!(!h.runtime.tree.get(overlay).in_flow());
    assert!(!h.runtime.tree.get(pill).in_flow());
    assert!(
        h.find_alive("modal").is_some(),
        "the exiting overlay still paints"
    );
    assert_eq!(h.runtime.next_delay_ms, 16);
    advance(250);
    h.frame();
    assert!(h.find_alive("modal").is_none(), "the overlay was freed");
    assert!(h.find_alive("modal-card").is_none());
    assert!(h.find_alive("message").is_none());
    assert!(h.runtime.tree.animating.is_empty());
    assert_eq!(h.runtime.next_delay_ms, -1);
}

#[test]
fn snapshot_and_reduce_motion_disable_transitions() {
    let _guard = lock();
    let _restore = RestoreEnabled(enabled());
    // What `ui::app::run` does for `ZANT_NO_MOTION=1`, `ZANT_SNAPSHOT` and the
    // system's Reduce Motion / Animation effects setting.
    set_enabled(false);
    let mut h = Harness::init(Panel { show: true });
    h.frame();
    let panel_id = h.find("panel");
    // no enter transition: the node is shown as is
    assert!(
        h.runtime
            .tree
            .get(panel_id)
            .anim
            .is_none_or(|anim| anim.fx.is_identity())
    );
    assert!(h.runtime.tree.animating.is_empty());
    assert_eq!(h.runtime.next_delay_ms, -1);

    // no exit transition either: the node is freed in the same rebuild
    h.runtime.app_state_mut::<Panel>().show = false;
    h.runtime.invalidate();
    h.frame();
    assert!(
        h.find_alive("panel").is_none(),
        "motion off frees a removed node immediately"
    );
    assert!(h.runtime.tree.free_ids.contains(&panel_id));
    assert!(h.runtime.tree.animating.is_empty());
    assert_eq!(h.runtime.next_delay_ms, -1);
}
