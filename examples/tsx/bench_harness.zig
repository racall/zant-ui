//! Shared parts of the TSX benchmark: per-frame timing statistics and a
//! headless runtime harness (raster canvas, fake clock, clicks by key).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;

pub const W: f32 = 640;
pub const H: f32 = 900;

var fake_now: i64 = 1_000_000;
fn fakeClock() i64 {
    return fake_now;
}

pub const Stats = struct {
    samples: std.ArrayList(f64) = .empty,

    pub fn add(self: *Stats, gpa: std.mem.Allocator, ms: f64) void {
        self.samples.append(gpa, ms) catch {};
    }

    pub fn report(self: *Stats, name: []const u8) void {
        const s = self.samples.items;
        if (s.len == 0) return;
        std.mem.sort(f64, s, {}, std.sort.asc(f64));
        var sum: f64 = 0;
        for (s) |v| sum += v;
        const p95 = s[@min(s.len - 1, (s.len * 95) / 100)];
        const avg = sum / @as(f64, @floatFromInt(s.len));
        std.debug.print("{s: <20} frames={d: >5}  avg={d: >6.3} ms  p95={d: >6.3} ms  max={d: >6.3} ms  ({d:.0} fps at avg)\n", .{
            name, s.len, avg, p95, s[s.len - 1], 1000.0 / avg,
        });
    }
};

/// Adapts `State.render(*State)` to the runtime's type-erased callback.
pub fn Render(comptime State: type) type {
    return struct {
        fn render(ctx: *anyopaque) ui.Node {
            const self: *State = @ptrCast(@alignCast(ctx));
            return self.render();
        }
    };
}

pub const Harness = struct {
    rt: *Runtime,
    renderer: *zant.renderer.Renderer,

    pub fn init(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, state: anytype) !Harness {
        const State = @typeInfo(@TypeOf(state)).pointer.child;
        const rt = try Runtime.init(gpa, state, Render(State).render, .{ .window = .{ .width = W, .height = H } });
        rt.clock = fakeClock;
        var h: Harness = .{ .rt = rt, .renderer = renderer };
        _ = h.frame();
        return h;
    }

    pub fn deinit(self: *Harness) void {
        self.rt.deinit();
    }

    /// One whole frame in milliseconds.
    pub fn frame(self: *Harness) f64 {
        fake_now += 16;
        const t0 = zant.platform.monotonicUs();
        const canvas = self.renderer.begin(1.0).?;
        self.rt.renderFrame(canvas, W, H, 1.0);
        self.renderer.end();
        return @as(f64, @floatFromInt(zant.platform.monotonicUs() - t0)) / 1000.0;
    }

    /// Clicks the control with `key` (down + up at its centre), then times
    /// the frame that shows the result. Fails when the key is not on screen.
    pub fn click(self: *Harness, key: []const u8) !f64 {
        const r = self.rt.tree.boundsOfKey(key) orelse return error.MissingKey;
        const p = zant.events.PointerEvent{ .x = r.x + r.width * 0.5, .y = r.y + r.height * 0.5 };
        _ = self.rt.dispatch(.{ .pointer_move = p });
        _ = self.rt.dispatch(.{ .pointer_down = p });
        _ = self.rt.dispatch(.{ .pointer_up = p });
        return self.frame();
    }
};
