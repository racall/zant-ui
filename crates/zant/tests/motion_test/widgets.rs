//! Segmented thumbs, focus / scroll commands, wakeups, modal and message presence, reduce motion.

use super::*;

pub(super) struct SegState {
    pub(super) selected: usize,
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

pub(super) struct CmdState {
    pub(super) renders: usize,
    pub(super) wake: bool,
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

pub(super) struct OverlayState {
    pub(super) show: bool,
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
