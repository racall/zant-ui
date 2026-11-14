//! Port of `src/motion/motion.zig`: motion primitives (Ant Design motion
//! specs, Framer-Motion-style concepts).
//!
//! * `Animated`: a time-stamped tween. It never accumulates per-frame deltas,
//!   so dropped frames don't slow it down and it can be sampled at any time.
//! * `Presence`: enter / exit transitions of nodes (antd `zoom`, `slide`,
//!   `move`, `fade` motions).
//! * Layout transitions (FLIP) live in the tree; see `docs/motion.md`.
//!
//! Animation frames only repaint; they never rebuild the UI.
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::motion::easing;
pub use crate::motion::easing::Easing;

/// Global switch (Reduce Motion, `ZANT_NO_MOTION`, snapshots). When false
/// every transition completes immediately.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Port of the Zig `motion.enabled` global.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Port of `motion.enabled = value` (the Zig module-level `var`).
pub fn set_enabled(value: bool) {
    ENABLED.store(value, Ordering::Relaxed);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub duration_ms: u16,
    pub easing: Easing,
    pub delay_ms: u16,
}

impl Default for Transition {
    fn default() -> Self {
        Transition {
            duration_ms: 200,
            easing: Easing::EaseInOut,
            delay_ms: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animated {
    pub from: f32,
    pub to: f32,
    pub start_ms: i64,
    pub duration_ms: u16,
    pub delay_ms: u16,
    pub easing: Easing,
}

impl Default for Animated {
    fn default() -> Self {
        Animated {
            from: 0.0,
            to: 0.0,
            start_ms: 0,
            duration_ms: 0,
            delay_ms: 0,
            easing: Easing::Linear,
        }
    }
}

impl Animated {
    /// Already settled at `v`.
    pub fn at(v: f32) -> Animated {
        Animated {
            from: v,
            to: v,
            ..Default::default()
        }
    }

    pub fn start(from: f32, to: f32, now: i64, tr: Transition) -> Animated {
        if !enabled() || tr.duration_ms == 0 {
            return Animated::at(to);
        }
        Animated {
            from,
            to,
            start_ms: now,
            duration_ms: tr.duration_ms,
            delay_ms: tr.delay_ms,
            easing: tr.easing,
        }
    }

    /// Animates towards `to` from the current value. A no-op when `to` is
    /// already the target; an interrupted tween continues from where it is,
    /// with the duration scaled by the remaining distance.
    pub fn retarget(&mut self, to: f32, now: i64, tr: Transition) {
        if self.to == to {
            return;
        }
        let current = self.value(now);
        let span = (self.to - self.from).abs();
        let mut scaled = tr;
        if !self.done(now) && span > 0.0001 {
            let ratio = ((to - current).abs() / span.max((to - self.from).abs())).clamp(0.25, 1.0);
            scaled.duration_ms = (f32::from(tr.duration_ms) * ratio) as u16;
            scaled.delay_ms = 0;
        }
        *self = Animated::start(current, to, now, scaled);
    }

    pub fn value(&self, now: i64) -> f32 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let elapsed = now - self.start_ms - i64::from(self.delay_ms);
        if elapsed <= 0 {
            return self.from;
        }
        if elapsed >= i64::from(self.duration_ms) {
            return self.to;
        }
        let t = elapsed as f32 / f32::from(self.duration_ms);
        self.from + (self.to - self.from) * easing::apply(self.easing, t)
    }

    pub fn done(&self, now: i64) -> bool {
        self.duration_ms == 0
            || now - self.start_ms >= i64::from(self.delay_ms) + i64::from(self.duration_ms)
    }

    /// Ends the tween at its target.
    pub fn settle(&mut self) {
        *self = Animated::at(self.to);
    }
}

/// Enter / exit transition presets (Ant Design).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Presence {
    #[default]
    None,
    /// Opacity (antd `fade`).
    Fade,
    /// Scale 0.8 → 1 + opacity (antd `zoom-big`: modals).
    ZoomBig,
    /// Same, 100 ms (antd `zoom-big-fast`: tooltips).
    ZoomBigFast,
    /// Vertical scale from the top edge + opacity (antd `slide-up`:
    /// dropdowns, select popups).
    SlideUp,
    /// Same from the bottom edge (popups opening upwards).
    SlideDown,
    /// Moves down 8px into place + opacity (antd `move-up`: messages).
    MoveUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Origin {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// Paint-time transform of a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fx {
    pub opacity: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Default for Fx {
    fn default() -> Self {
        Fx {
            opacity: 1.0,
            scale_x: 1.0,
            scale_y: 1.0,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

impl Fx {
    pub fn is_identity(&self) -> bool {
        self.opacity >= 0.997
            && (self.scale_x - 1.0).abs() < 0.0005
            && (self.scale_y - 1.0).abs() < 0.0005
            && self.dx.abs() < 0.01
            && self.dy.abs() < 0.01
    }
}

pub fn presence_enter(k: Presence) -> Transition {
    match k {
        Presence::None => Transition {
            duration_ms: 0,
            ..Default::default()
        },
        Presence::Fade => Transition {
            duration_ms: 200,
            easing: Easing::Linear,
            ..Default::default()
        },
        Presence::ZoomBig => Transition {
            duration_ms: 200,
            easing: Easing::EaseOutCirc,
            ..Default::default()
        },
        Presence::ZoomBigFast => Transition {
            duration_ms: 100,
            easing: Easing::EaseOutCirc,
            ..Default::default()
        },
        Presence::SlideUp | Presence::SlideDown => Transition {
            duration_ms: 200,
            easing: Easing::EaseOutQuint,
            ..Default::default()
        },
        Presence::MoveUp => Transition {
            duration_ms: 200,
            easing: Easing::EaseOut,
            ..Default::default()
        },
    }
}

pub fn presence_exit(k: Presence) -> Transition {
    match k {
        Presence::None => Transition {
            duration_ms: 0,
            ..Default::default()
        },
        Presence::Fade => Transition {
            duration_ms: 200,
            easing: Easing::Linear,
            ..Default::default()
        },
        Presence::ZoomBig => Transition {
            duration_ms: 200,
            easing: Easing::EaseInOutCirc,
            ..Default::default()
        },
        Presence::ZoomBigFast => Transition {
            duration_ms: 100,
            easing: Easing::EaseInOutCirc,
            ..Default::default()
        },
        Presence::SlideUp | Presence::SlideDown => Transition {
            duration_ms: 200,
            easing: Easing::EaseInQuint,
            ..Default::default()
        },
        Presence::MoveUp => Transition {
            duration_ms: 200,
            easing: Easing::EaseInOut,
            ..Default::default()
        },
    }
}

/// Transform for presence progress `v` (0 = hidden, 1 = fully shown). The
/// eased value is already applied by the `Animated` driving `v`.
pub fn presence_fx(k: Presence, v: f32) -> Fx {
    let o = v.clamp(0.0, 1.0);
    match k {
        Presence::None => Fx::default(),
        Presence::Fade => Fx {
            opacity: o,
            ..Default::default()
        },
        Presence::ZoomBig | Presence::ZoomBigFast => Fx {
            opacity: o,
            scale_x: 0.8 + 0.2 * v,
            scale_y: 0.8 + 0.2 * v,
            ..Default::default()
        },
        Presence::SlideUp | Presence::SlideDown => Fx {
            opacity: o,
            scale_y: 0.8 + 0.2 * v,
            ..Default::default()
        },
        Presence::MoveUp => Fx {
            opacity: o,
            dy: -8.0 * (1.0 - v),
            ..Default::default()
        },
    }
}

/// Transform origin of a presence preset (slides grow from their edge).
pub fn presence_origin(k: Presence, fallback: Origin) -> Origin {
    match k {
        Presence::SlideUp => match fallback {
            Origin::TopLeft | Origin::Left | Origin::BottomLeft => Origin::TopLeft,
            Origin::TopRight | Origin::Right | Origin::BottomRight => Origin::TopRight,
            _ => Origin::Top,
        },
        Presence::SlideDown => match fallback {
            Origin::TopLeft | Origin::Left | Origin::BottomLeft => Origin::BottomLeft,
            Origin::TopRight | Origin::Right | Origin::BottomRight => Origin::BottomRight,
            _ => Origin::Bottom,
        },
        _ => fallback,
    }
}

/// Point of `origin` in the rect (x, y, w, h).
pub fn origin_point(origin: Origin, x: f32, y: f32, w: f32, h: f32) -> [f32; 2] {
    let fx: f32 = match origin {
        Origin::TopLeft | Origin::Left | Origin::BottomLeft => 0.0,
        Origin::Top | Origin::Center | Origin::Bottom => 0.5,
        Origin::TopRight | Origin::Right | Origin::BottomRight => 1.0,
    };
    let fy: f32 = match origin {
        Origin::TopLeft | Origin::Top | Origin::TopRight => 0.0,
        Origin::Left | Origin::Center | Origin::Right => 0.5,
        Origin::BottomLeft | Origin::Bottom | Origin::BottomRight => 1.0,
    };
    [x + w * fx, y + h * fy]
}

/// Interpolates two ARGB colours.
pub fn mix_color(a: u32, b: u32, t: f32) -> u32 {
    let k = t.clamp(0.0, 1.0);
    if k <= 0.0 {
        return a;
    }
    if k >= 1.0 {
        return b;
    }
    let mut out: u32 = 0;
    for shift in [24u32, 16, 8, 0] {
        let ca = ((a >> shift) & 0xff) as f32;
        let cb = ((b >> shift) & 0xff) as f32;
        let c = (ca + (cb - ca) * k).round() as u32;
        out |= c << shift;
    }
    out
}
