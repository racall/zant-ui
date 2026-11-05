//! Easing curves (Ant Design motion tokens) evaluated with the same
//! cubic-bezier solver browsers use (WebKit `UnitBezier`).
const std = @import("std");

pub const Easing = enum(u8) {
    linear,
    /// CSS `ease`.
    ease,
    /// antd `motionEaseInOut`.
    ease_in_out,
    /// antd `motionEaseOut`.
    ease_out,
    ease_out_circ,
    ease_in_out_circ,
    ease_out_back,
    ease_in_back,
    ease_out_quint,
    ease_in_quint,
};

const Curve = struct { x1: f32, y1: f32, x2: f32, y2: f32 };

fn curve(e: Easing) Curve {
    return switch (e) {
        .linear => .{ .x1 = 0, .y1 = 0, .x2 = 1, .y2 = 1 },
        .ease => .{ .x1 = 0.25, .y1 = 0.1, .x2 = 0.25, .y2 = 1 },
        .ease_in_out => .{ .x1 = 0.645, .y1 = 0.045, .x2 = 0.355, .y2 = 1 },
        .ease_out => .{ .x1 = 0.215, .y1 = 0.61, .x2 = 0.355, .y2 = 1 },
        .ease_out_circ => .{ .x1 = 0.08, .y1 = 0.82, .x2 = 0.17, .y2 = 1 },
        .ease_in_out_circ => .{ .x1 = 0.78, .y1 = 0.14, .x2 = 0.15, .y2 = 0.86 },
        .ease_out_back => .{ .x1 = 0.12, .y1 = 0.4, .x2 = 0.29, .y2 = 1.46 },
        .ease_in_back => .{ .x1 = 0.71, .y1 = -0.46, .x2 = 0.88, .y2 = 0.6 },
        .ease_out_quint => .{ .x1 = 0.23, .y1 = 1, .x2 = 0.32, .y2 = 1 },
        .ease_in_quint => .{ .x1 = 0.755, .y1 = 0.05, .x2 = 0.855, .y2 = 0.06 },
    };
}

/// CSS `cubic-bezier(x1, y1, x2, y2)` at progress `x` (0..1).
pub fn cubicBezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) f32 {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    const cx = 3 * x1;
    const bx = 3 * (x2 - x1) - cx;
    const ax = 1 - cx - bx;
    const cy = 3 * y1;
    const by = 3 * (y2 - y1) - cy;
    const ay = 1 - cy - by;
    const eps: f32 = 1e-5;

    const sampleX = struct {
        fn f(a: f32, b: f32, c: f32, t: f32) f32 {
            return ((a * t + b) * t + c) * t;
        }
    }.f;

    // Newton's method first (fast), bisection as the fallback
    var t = x;
    var i: usize = 0;
    var solved = false;
    while (i < 8) : (i += 1) {
        const err = sampleX(ax, bx, cx, t) - x;
        if (@abs(err) < eps) {
            solved = true;
            break;
        }
        const d = (3 * ax * t + 2 * bx) * t + cx;
        if (@abs(d) < 1e-6) break;
        t -= err / d;
    }
    if (!solved) {
        var lo: f32 = 0;
        var hi: f32 = 1;
        t = x;
        var n: usize = 0;
        while (lo < hi and n < 32) : (n += 1) {
            const v = sampleX(ax, bx, cx, t);
            if (@abs(v - x) < eps) break;
            if (x > v) lo = t else hi = t;
            t = (hi - lo) * 0.5 + lo;
        }
    }
    return ((ay * t + by) * t + cy) * t;
}

/// Eased progress of `t` (clamped to 0..1).
pub fn apply(e: Easing, t: f32) f32 {
    const x = std.math.clamp(t, 0, 1);
    if (e == .linear) return x;
    const c = curve(e);
    return cubicBezier(c.x1, c.y1, c.x2, c.y2, x);
}
