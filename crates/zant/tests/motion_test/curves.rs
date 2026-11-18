//! Easing curves, animated values, disabled motion and presence transforms.

use super::*;

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
