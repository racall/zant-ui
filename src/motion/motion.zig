//! Motion primitives (Ant Design motion specs, Framer-Motion-style concepts).
//!
//! * `Animated`: a time-stamped tween. It never accumulates per-frame deltas,
//!   so dropped frames don't slow it down and it can be sampled at any time.
//! * `Presence`: enter / exit transitions of nodes (antd `zoom`, `slide`,
//!   `move`, `fade` motions).
//! * Layout transitions (FLIP) live in the tree; see `docs/motion.md`.
//!
//! Animation frames only repaint; they never rebuild the UI.
const std = @import("std");
pub const easing = @import("easing.zig");
pub const Easing = easing.Easing;

/// Global switch (Reduce Motion, `ZANT_NO_MOTION`, snapshots). When false
/// every transition completes immediately.
pub var enabled: bool = true;

pub const Transition = struct {
    duration_ms: u16 = 200,
    easing: Easing = .ease_in_out,
    delay_ms: u16 = 0,
};

pub const Animated = struct {
    from: f32 = 0,
    to: f32 = 0,
    start_ms: i64 = 0,
    duration_ms: u16 = 0,
    delay_ms: u16 = 0,
    easing: Easing = .linear,

    /// Already settled at `v`.
    pub fn at(v: f32) Animated {
        return .{ .from = v, .to = v };
    }

    pub fn start(from: f32, to: f32, now: i64, tr: Transition) Animated {
        if (!enabled or tr.duration_ms == 0) return at(to);
        return .{ .from = from, .to = to, .start_ms = now, .duration_ms = tr.duration_ms, .delay_ms = tr.delay_ms, .easing = tr.easing };
    }

    /// Animates towards `to` from the current value. A no-op when `to` is
    /// already the target; an interrupted tween continues from where it is,
    /// with the duration scaled by the remaining distance.
    pub fn retarget(self: *Animated, to: f32, now: i64, tr: Transition) void {
        if (self.to == to) return;
        const current = self.value(now);
        const span = @abs(self.to - self.from);
        var scaled = tr;
        if (!self.done(now) and span > 0.0001) {
            const ratio = std.math.clamp(@abs(to - current) / @max(span, @abs(to - self.from)), 0.25, 1);
            scaled.duration_ms = @intFromFloat(@as(f32, @floatFromInt(tr.duration_ms)) * ratio);
            scaled.delay_ms = 0;
        }
        self.* = start(current, to, now, scaled);
    }

    pub fn value(self: Animated, now: i64) f32 {
        if (self.duration_ms == 0) return self.to;
        const elapsed = now - self.start_ms - @as(i64, self.delay_ms);
        if (elapsed <= 0) return self.from;
        if (elapsed >= self.duration_ms) return self.to;
        const t = @as(f32, @floatFromInt(elapsed)) / @as(f32, @floatFromInt(self.duration_ms));
        return self.from + (self.to - self.from) * easing.apply(self.easing, t);
    }

    pub fn done(self: Animated, now: i64) bool {
        return self.duration_ms == 0 or now - self.start_ms >= @as(i64, self.delay_ms) + self.duration_ms;
    }

    /// Ends the tween at its target.
    pub fn settle(self: *Animated) void {
        self.* = at(self.to);
    }
};

/// Enter / exit transition presets (Ant Design).
pub const Presence = enum(u8) {
    none,
    /// Opacity (antd `fade`).
    fade,
    /// Scale 0.8 → 1 + opacity (antd `zoom-big`: modals).
    zoom_big,
    /// Same, 100 ms (antd `zoom-big-fast`: tooltips).
    zoom_big_fast,
    /// Vertical scale from the top edge + opacity (antd `slide-up`:
    /// dropdowns, select popups).
    slide_up,
    /// Same from the bottom edge (popups opening upwards).
    slide_down,
    /// Moves down 8px into place + opacity (antd `move-up`: messages).
    move_up,
};

pub const Origin = enum(u8) { top_left, top, top_right, left, center, right, bottom_left, bottom, bottom_right };

/// Paint-time transform of a node.
pub const Fx = struct {
    opacity: f32 = 1,
    scale_x: f32 = 1,
    scale_y: f32 = 1,
    dx: f32 = 0,
    dy: f32 = 0,

    pub fn isIdentity(self: Fx) bool {
        return self.opacity >= 0.997 and @abs(self.scale_x - 1) < 0.0005 and @abs(self.scale_y - 1) < 0.0005 and
            @abs(self.dx) < 0.01 and @abs(self.dy) < 0.01;
    }
};

pub fn presenceEnter(k: Presence) Transition {
    return switch (k) {
        .none => .{ .duration_ms = 0 },
        .fade => .{ .duration_ms = 200, .easing = .linear },
        .zoom_big => .{ .duration_ms = 200, .easing = .ease_out_circ },
        .zoom_big_fast => .{ .duration_ms = 100, .easing = .ease_out_circ },
        .slide_up, .slide_down => .{ .duration_ms = 200, .easing = .ease_out_quint },
        .move_up => .{ .duration_ms = 200, .easing = .ease_out },
    };
}

pub fn presenceExit(k: Presence) Transition {
    return switch (k) {
        .none => .{ .duration_ms = 0 },
        .fade => .{ .duration_ms = 200, .easing = .linear },
        .zoom_big => .{ .duration_ms = 200, .easing = .ease_in_out_circ },
        .zoom_big_fast => .{ .duration_ms = 100, .easing = .ease_in_out_circ },
        .slide_up, .slide_down => .{ .duration_ms = 200, .easing = .ease_in_quint },
        .move_up => .{ .duration_ms = 200, .easing = .ease_in_out },
    };
}

/// Transform for presence progress `v` (0 = hidden, 1 = fully shown). The
/// eased value is already applied by the `Animated` driving `v`.
pub fn presenceFx(k: Presence, v: f32) Fx {
    const o = std.math.clamp(v, 0, 1);
    return switch (k) {
        .none => .{},
        .fade => .{ .opacity = o },
        .zoom_big, .zoom_big_fast => .{ .opacity = o, .scale_x = 0.8 + 0.2 * v, .scale_y = 0.8 + 0.2 * v },
        .slide_up, .slide_down => .{ .opacity = o, .scale_y = 0.8 + 0.2 * v },
        .move_up => .{ .opacity = o, .dy = -8 * (1 - v) },
    };
}

/// Transform origin of a presence preset (slides grow from their edge).
pub fn presenceOrigin(k: Presence, fallback: Origin) Origin {
    return switch (k) {
        .slide_up => switch (fallback) {
            .top_left, .left, .bottom_left => .top_left,
            .top_right, .right, .bottom_right => .top_right,
            else => .top,
        },
        .slide_down => switch (fallback) {
            .top_left, .left, .bottom_left => .bottom_left,
            .top_right, .right, .bottom_right => .bottom_right,
            else => .bottom,
        },
        else => fallback,
    };
}

/// Point of `origin` in the rect (x, y, w, h).
pub fn originPoint(origin: Origin, x: f32, y: f32, w: f32, h: f32) [2]f32 {
    const fx: f32 = switch (origin) {
        .top_left, .left, .bottom_left => 0,
        .top, .center, .bottom => 0.5,
        .top_right, .right, .bottom_right => 1,
    };
    const fy: f32 = switch (origin) {
        .top_left, .top, .top_right => 0,
        .left, .center, .right => 0.5,
        .bottom_left, .bottom, .bottom_right => 1,
    };
    return .{ x + w * fx, y + h * fy };
}

/// Interpolates two ARGB colours.
pub fn mixColor(a: u32, b: u32, t: f32) u32 {
    const k = std.math.clamp(t, 0, 1);
    if (k <= 0) return a;
    if (k >= 1) return b;
    var out: u32 = 0;
    inline for (.{ 24, 16, 8, 0 }) |shift| {
        const ca: f32 = @floatFromInt((a >> shift) & 0xff);
        const cb: f32 = @floatFromInt((b >> shift) & 0xff);
        const c: u32 = @intFromFloat(@round(ca + (cb - ca) * k));
        out |= c << shift;
    }
    return out;
}
