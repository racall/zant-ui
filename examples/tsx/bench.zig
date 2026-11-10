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
//!   todos     the TSX todo app vs the hand-written one: 1000 items added by
//!             typing + Enter, 500 toggles, 60 filter switches
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Counter = @import("counter_tsx").Counter;
const Manual = @import("counter_manual.zig").Counter;
const Settings = @import("settings_tsx").Settings;

const harness = @import("bench_harness.zig");
const W = harness.W;
const H = harness.H;
const Stats = harness.Stats;
const Harness = harness.Harness;
const todos_bench = @import("bench_todos.zig");

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
    if (want(args, "todos")) try todos_bench.run(gpa, &renderer);
}
