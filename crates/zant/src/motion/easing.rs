//! Port of `src/motion/easing.zig`: Ant Design motion curves evaluated with
//! the same cubic-bezier solver browsers use (WebKit `UnitBezier`).
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Easing {
    #[default]
    Linear,
    /// CSS `ease`.
    Ease,
    /// antd `motionEaseInOut`.
    EaseInOut,
    /// antd `motionEaseOut`.
    EaseOut,
    EaseOutCirc,
    EaseInOutCirc,
    EaseOutBack,
    EaseInBack,
    EaseOutQuint,
    EaseInQuint,
}

impl Easing {
    /// Port of `std.enums.values(Easing)`.
    pub const ALL: [Easing; 10] = [
        Easing::Linear,
        Easing::Ease,
        Easing::EaseInOut,
        Easing::EaseOut,
        Easing::EaseOutCirc,
        Easing::EaseInOutCirc,
        Easing::EaseOutBack,
        Easing::EaseInBack,
        Easing::EaseOutQuint,
        Easing::EaseInQuint,
    ];
}

struct Curve {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

fn curve(e: Easing) -> Curve {
    match e {
        Easing::Linear => Curve {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        },
        Easing::Ease => Curve {
            x1: 0.25,
            y1: 0.1,
            x2: 0.25,
            y2: 1.0,
        },
        Easing::EaseInOut => Curve {
            x1: 0.645,
            y1: 0.045,
            x2: 0.355,
            y2: 1.0,
        },
        Easing::EaseOut => Curve {
            x1: 0.215,
            y1: 0.61,
            x2: 0.355,
            y2: 1.0,
        },
        Easing::EaseOutCirc => Curve {
            x1: 0.08,
            y1: 0.82,
            x2: 0.17,
            y2: 1.0,
        },
        Easing::EaseInOutCirc => Curve {
            x1: 0.78,
            y1: 0.14,
            x2: 0.15,
            y2: 0.86,
        },
        Easing::EaseOutBack => Curve {
            x1: 0.12,
            y1: 0.4,
            x2: 0.29,
            y2: 1.46,
        },
        Easing::EaseInBack => Curve {
            x1: 0.71,
            y1: -0.46,
            x2: 0.88,
            y2: 0.6,
        },
        Easing::EaseOutQuint => Curve {
            x1: 0.23,
            y1: 1.0,
            x2: 0.32,
            y2: 1.0,
        },
        Easing::EaseInQuint => Curve {
            x1: 0.755,
            y1: 0.05,
            x2: 0.855,
            y2: 0.06,
        },
    }
}

/// CSS `cubic-bezier(x1, y1, x2, y2)` at progress `x` (0..1).
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;
    let eps: f32 = 1e-5;

    fn sample_x(a: f32, b: f32, c: f32, t: f32) -> f32 {
        ((a * t + b) * t + c) * t
    }

    // Newton's method first (fast), bisection as the fallback
    let mut t = x;
    let mut solved = false;
    for _ in 0..8 {
        let err = sample_x(ax, bx, cx, t) - x;
        if err.abs() < eps {
            solved = true;
            break;
        }
        let d = (3.0 * ax * t + 2.0 * bx) * t + cx;
        if d.abs() < 1e-6 {
            break;
        }
        t -= err / d;
    }
    if !solved {
        let mut lo: f32 = 0.0;
        let mut hi: f32 = 1.0;
        t = x;
        let mut n: usize = 0;
        while lo < hi && n < 32 {
            let v = sample_x(ax, bx, cx, t);
            if (v - x).abs() < eps {
                break;
            }
            if x > v {
                lo = t;
            } else {
                hi = t;
            }
            t = (hi - lo) * 0.5 + lo;
            n += 1;
        }
    }
    ((ay * t + by) * t + cy) * t
}

/// Eased progress of `t` (clamped to 0..1).
pub fn apply(e: Easing, t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    if e == Easing::Linear {
        return x;
    }
    let c = curve(e);
    cubic_bezier(c.x1, c.y1, c.x2, c.y2, x)
}
