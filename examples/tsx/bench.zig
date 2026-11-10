//! Headless benchmark for TSX components (`zig build bench-tsx`): clicks
//! and typing driven through the runtime with a raster canvas, every frame
//! timed (rebuild + layout + paint), like `zant-bench`.
//!
//!   zig build -Doptimize=ReleaseFast --prefix zig-out-release
//!   ./zig-out-release/bin/zant-tsx-bench              # all scenarios
//!   ./zig-out-release/bin/zant-tsx-bench clicks       # some of them
//!
//! Scenarios:
//!   clicks    3000 clicks cycling through every control of `counter.tsx`
//!   manual    the same clicks on the hand-written Zig counter (baseline)
//!   typing    300 characters typed into the counter's input
//!   settings  2000 switch clicks on `settings.tsx` (40 rows, one state each)
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Counter = @import("counter_tsx").Counter;
const Manual = @import("counter_manual.zig").Counter;
const Settings = @import("settings_tsx").Settings;

const W: f32 = 640;
const H: f32 = 900;

var fake_now: i64 = 1_000_000;
fn fakeClock() i64 {
    return fake_now;
}

const Stats = struct {
    samples: std.ArrayList(f64) = .empty,

    fn add(self: *Stats, gpa: std.mem.Allocator, ms: f64) void {
        self.samples.append(gpa, ms) catch {};
    }

    fn report(self: *Stats, name: []const u8) void {
        const s = self.samples.items;
        if (s.len == 0) return;
        std.mem.sort(f64, s, {}, std.sort.asc(f64));
        var sum: f64 = 0;
        for (s) |v| sum += v;
        const p95 = s[@min(s.len - 1, (s.len * 95) / 100)];
        const avg = sum / @as(f64, @floatFromInt(s.len));
        std.debug.print("{s: <9} frames={d: >5}  avg={d: >6.3} ms  p95={d: >6.3} ms  max={d: >6.3} ms  ({d:.0} fps at avg)\n", .{
            name, s.len, avg, p95, s[s.len - 1], 1000.0 / avg,
        });
    }
};

/// Adapts `State.render(*State)` to the runtime's type-erased callback.
fn Render(comptime State: type) type {
    return struct {
        fn render(ctx: *anyopaque) ui.Node {
            const self: *State = @ptrCast(@alignCast(ctx));
            return self.render();
        }
    };
}

const Harness = struct {
    rt: *Runtime,
    renderer: *zant.renderer.Renderer,

    fn init(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, state: anytype) !Harness {
        const State = @typeInfo(@TypeOf(state)).pointer.child;
        const rt = try Runtime.init(gpa, state, Render(State).render, .{ .window = .{ .width = W, .height = H } });
        rt.clock = fakeClock;
        var h: Harness = .{ .rt = rt, .renderer = renderer };
        _ = h.frame();
        return h;
    }

    fn deinit(self: *Harness) void {
        self.rt.deinit();
    }

    /// One whole frame in milliseconds.
    fn frame(self: *Harness) f64 {
        fake_now += 16;
        const t0 = zant.platform.monotonicUs();
        const canvas = self.renderer.begin(1.0).?;
        self.rt.renderFrame(canvas, W, H, 1.0);
        self.renderer.end();
        return @as(f64, @floatFromInt(zant.platform.monotonicUs() - t0)) / 1000.0;
    }

    /// Clicks the control with `key` (down + up at its centre), then times
    /// the frame that shows the result. Fails when the key is not on screen.
    fn click(self: *Harness, key: []const u8) !f64 {
        const r = self.rt.tree.boundsOfKey(key) orelse return error.MissingKey;
        const p = zant.events.PointerEvent{ .x = r.x + r.width * 0.5, .y = r.y + r.height * 0.5 };
        _ = self.rt.dispatch(.{ .pointer_move = p });
        _ = self.rt.dispatch(.{ .pointer_down = p });
        _ = self.rt.dispatch(.{ .pointer_up = p });
        return self.frame();
    }
};

/// The counter's controls in the order the scenario clicks them; `reset`
/// is skipped while it is disabled (count 0), as a user would.
const counter_keys = [_][]const u8{ "inc", "inc", "step", "inc", "large", "notify", "reset", "inc", "step" };

fn clickCounter(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, comptime State: type, name: []const u8) !void {
    var state = State{};
    var h = try Harness.init(gpa, renderer, &state);
    defer h.deinit();
    var st: Stats = .{};
    defer st.samples.deinit(gpa);
    var i: usize = 0;
    while (st.samples.items.len < 3000) : (i += 1) {
        const key = counter_keys[i % counter_keys.len];
        if (std.mem.eql(u8, key, "reset") and state.count == 0) continue;
        st.add(gpa, try h.click(key));
    }
    st.report(name);
}

pub fn main(init: std.process.Init) !void {
    const gpa = std.heap.c_allocator;
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    zant.motion.enabled = false;
    var renderer = try zant.renderer.Renderer.createRaster(@intFromFloat(W), @intFromFloat(H));
    defer renderer.deinit();
    const want = struct {
        fn f(list: []const [:0]const u8, name: []const u8) bool {
            if (list.len <= 1) return true;
            for (list[1..]) |a| if (std.mem.eql(u8, a, name)) return true;
            return false;
        }
    }.f;
    std.debug.print("zant-tsx-bench: raster canvas {d}x{d}\n", .{ W, H });

    if (want(args, "clicks")) try clickCounter(gpa, &renderer, Counter, "clicks");
    if (want(args, "manual")) try clickCounter(gpa, &renderer, Manual, "manual");
    if (want(args, "typing")) {
        var state = Counter{};
        defer state.name.deinit();
        var h = try Harness.init(gpa, &renderer, &state);
        defer h.deinit();
        _ = try h.click("name");
        var st: Stats = .{};
        defer st.samples.deinit(gpa);
        for (0..300) |_| {
            _ = h.rt.dispatch(.{ .text_input = "a" });
            st.add(gpa, h.frame());
        }
        st.report("typing");
    }
    if (want(args, "settings")) {
        var state = Settings{};
        var h = try Harness.init(gpa, &renderer, &state);
        defer h.deinit();
        var st: Stats = .{};
        defer st.samples.deinit(gpa);
        var key_buf: [16]u8 = undefined;
        // the first rows fit in the window; cycle through them
        for (0..2000) |i| {
            const key = try std.fmt.bufPrint(&key_buf, "sw-{d}", .{i % 12});
            st.add(gpa, try h.click(key));
        }
        st.report("settings");
        if (state.changes != 2000) {
            std.debug.print("settings: expected 2000 changes, got {d}\n", .{state.changes});
            return error.LostClicks;
        }
    }
}
