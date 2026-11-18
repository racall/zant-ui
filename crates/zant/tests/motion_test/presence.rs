//! Enter / exit presence and the next frame delay.

use super::*;

pub(super) struct Panel {
    pub(super) show: bool,
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
