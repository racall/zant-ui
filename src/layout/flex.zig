const std = @import("std");

pub const Direction = enum {
    row,
    column,
};

pub const Justify = enum {
    start,
    center,
    end,
    space_between,
    space_around,
    space_evenly,
};

pub const Align = enum {
    start,
    center,
    end,
    stretch,
};

pub const Item = struct {
    base: f32 = 0,
    grow: f32 = 0,
    shrink: f32 = 1,
    min: f32 = 0,
    max: f32 = std.math.inf(f32),
    margin_start: f32 = 0,
    margin_end: f32 = 0,
};

pub const AxisSize = struct {
    /// Space occupied by item content (excluding margins).
    size: f32 = 0,
    margin_start: f32 = 0,
    margin_end: f32 = 0,
};

pub const Distribution = struct {
    /// Absolute offset added before the first item (justify-content).
    start_offset: f32 = 0,
    /// Extra space inserted between adjacent items.
    between: f32 = 0,
    /// Leftover space in the main axis after flexing and justification.
    free: f32 = 0,
};

fn clamp(v: f32, lo: f32, hi: f32) f32 {
    return @max(lo, @min(v, hi));
}

/// Resolves a single-line flex distribution. `available` excludes container
/// padding but includes item margins and gaps. `out` receives content sizes for
/// each item. The returned value is the leftover free space (usually 0 after a
/// successful grow/shrink pass, or the justified space if no flex factors).
pub fn resolve(items: []const Item, available: f32, gap: f32, out: []f32) f32 {
    std.debug.assert(items.len == out.len);
    const n = items.len;
    if (n == 0) return @max(0, available);

    var total: f32 = 0;
    for (items, 0..) |item, i| {
        out[i] = clamp(item.base, item.min, item.max);
        total += out[i] + item.margin_start + item.margin_end;
    }
    const gap_total = gap * @as(f32, @floatFromInt(if (n > 1) n - 1 else 0));
    total += gap_total;

    var free = available - total;

    if (free > 0.0001) {
        var grow_total: f32 = 0;
        for (items) |item| grow_total += @max(0, item.grow);
        if (grow_total > 0.0001) {
            var remaining = free;
            var active: [64]bool = undefined;
            @memset(&active, true);
            if (n > active.len) {
                // The UI's node count per flex line is intentionally small. For
                // larger lists, fall back to a single proportional pass.
                for (items, 0..) |item, i| {
                    if (item.grow <= 0) continue;
                    out[i] = clamp(out[i] + remaining * item.grow / grow_total, item.min, item.max);
                }
                free = 0;
            } else {
                var pass: usize = 0;
                while (pass < 8 and remaining > 0.0001) : (pass += 1) {
                    var weight: f32 = 0;
                    for (items, 0..) |item, i| {
                        if (active[i] and item.grow > 0) weight += item.grow;
                    }
                    if (weight <= 0.0001) break;
                    var used: f32 = 0;
                    for (items, 0..) |item, i| {
                        if (!active[i] or item.grow <= 0) continue;
                        const add = remaining * item.grow / weight;
                        const old = out[i];
                        out[i] = clamp(out[i] + add, item.min, item.max);
                        used += out[i] - old;
                        if (out[i] >= item.max - 0.0001) {
                            active[i] = false;
                        }
                    }
                    remaining -= used;
                    if (used <= 0.0001) break;
                }
                free = remaining;
            }
        }
    } else if (free < -0.0001) {
        var shrink_weight: f32 = 0;
        for (items, 0..) |item, i| {
            shrink_weight += @max(0, item.shrink) * @max(out[i], 0);
        }
        if (shrink_weight > 0.0001) {
            var deficit = -free;
            var active: [64]bool = undefined;
            @memset(&active, true);
            if (n > active.len) {
                for (items, 0..) |item, i| {
                    const weight = @max(0, item.shrink) * @max(out[i], 0);
                    if (weight <= 0) continue;
                    out[i] = clamp(out[i] - deficit * weight / shrink_weight, item.min, item.max);
                }
                free = 0;
            } else {
                var pass: usize = 0;
                while (pass < 8 and deficit > 0.0001) : (pass += 1) {
                    var weight: f32 = 0;
                    for (items, 0..) |item, i| {
                        if (active[i] and item.shrink > 0) {
                            weight += @max(0, item.shrink) * @max(out[i], 0);
                        }
                    }
                    if (weight <= 0.0001) break;
                    var used: f32 = 0;
                    for (items, 0..) |item, i| {
                        if (!active[i] or item.shrink <= 0) continue;
                        const w = @max(0, item.shrink) * @max(out[i], 0);
                        if (w <= 0) continue;
                        const old = out[i];
                        out[i] = clamp(out[i] - deficit * w / weight, item.min, item.max);
                        used += old - out[i];
                        if (out[i] <= item.min + 0.0001) active[i] = false;
                    }
                    deficit -= used;
                    if (used <= 0.0001) break;
                }
                free = -deficit;
            }
        }
    }

    if (free > 0.0001) return free;
    if (free < -0.0001) return free;
    return 0;
}

pub fn justifyDistribution(justify: Justify, free: f32, count: usize) Distribution {
    if (count == 0 or free <= 0) return .{};
    const f = @max(free, 0);
    const n: f32 = @floatFromInt(count);
    return switch (justify) {
        .start => .{ .free = f },
        .center => .{ .start_offset = f / 2, .free = f },
        .end => .{ .start_offset = f, .free = f },
        .space_between => if (count > 1)
            .{ .between = f / (n - 1), .free = f }
        else
            .{ .free = f },
        .space_around => .{ .start_offset = f / (2 * n), .between = f / n, .free = f },
        .space_evenly => .{ .start_offset = f / (n + 1), .between = f / (n + 1), .free = f },
    };
}

test "row grow distribution" {
    const items = [_]Item{
        .{ .base = 100, .grow = 1 },
        .{ .base = 100, .grow = 1 },
    };
    var out: [2]f32 = undefined;
    const free = resolve(&items, 400, 0, &out);
    try std.testing.expectApproxEqAbs(@as(f32, 0), free, 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 200), out[0], 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 200), out[1], 0.01);
}

test "grow respects max size" {
    const items = [_]Item{
        .{ .base = 100, .grow = 1, .max = 120 },
        .{ .base = 100, .grow = 1 },
    };
    var out: [2]f32 = undefined;
    _ = resolve(&items, 400, 0, &out);
    try std.testing.expectApproxEqAbs(@as(f32, 120), out[0], 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 280), out[1], 0.01);
}

test "shrink weighted by base" {
    const items = [_]Item{
        .{ .base = 200, .shrink = 1 },
        .{ .base = 100, .shrink = 1 },
    };
    var out: [2]f32 = undefined;
    _ = resolve(&items, 200, 0, &out);
    try std.testing.expectApproxEqAbs(@as(f32, 133.333), out[0], 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 66.666), out[1], 0.01);
}

test "gap and margins participate in main size" {
    const items = [_]Item{
        .{ .base = 50, .margin_start = 5, .margin_end = 5 },
        .{ .base = 50, .margin_start = 5, .margin_end = 5 },
    };
    var out: [2]f32 = undefined;
    const free = resolve(&items, 200, 10, &out);
    try std.testing.expectApproxEqAbs(@as(f32, 70), free, 0.01);
}

test "justify variants" {
    const between = justifyDistribution(.space_between, 60, 3);
    try std.testing.expectApproxEqAbs(@as(f32, 30), between.between, 0.01);
    const around = justifyDistribution(.space_around, 60, 3);
    try std.testing.expectApproxEqAbs(@as(f32, 10), around.start_offset, 0.01);
    const evenly = justifyDistribution(.space_evenly, 60, 3);
    try std.testing.expectApproxEqAbs(@as(f32, 15), evenly.start_offset, 0.01);
}
