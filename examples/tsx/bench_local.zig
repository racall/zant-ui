//! Child component state benchmark: `local_bench.tsx` (a `useState` per
//! `<Counter>`) against `local_manual.zig` (`ui.localState` by hand): 300
//! rows mounted one click at a time, then 300 clicks on row counters.
const std = @import("std");
const zant = @import("zant");
const harness = @import("bench_harness.zig");
const Stats = harness.Stats;
const Harness = harness.Harness;
const Tsx = @import("local_bench_tsx").LocalBench;
const Manual = @import("local_manual.zig").LocalBench;

const rows = 300;

pub fn run(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer) !void {
    var tsx_app = Tsx{};
    try scenario(gpa, renderer, &tsx_app, "tsx");
    var manual_app = Manual{};
    defer manual_app.deinit();
    try scenario(gpa, renderer, &manual_app, "manual");
}

fn scenario(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, app: anytype, label: []const u8) !void {
    var h = try Harness.init(gpa, renderer, app);
    defer h.deinit();
    var mount: Stats = .{};
    defer mount.samples.deinit(gpa);
    for (0..rows) |_| mount.add(gpa, try h.click("add"));
    if (h.rt.local_states.count() != rows) return error.WrongSlotCount;
    var click: Stats = .{};
    defer click.samples.deinit(gpa);
    var key: [32]u8 = undefined;
    for (0..300) |i| {
        // the rows on screen
        const k = try std.fmt.bufPrint(&key, "inc-{d}", .{i % 12 + 1});
        click.add(gpa, try h.click(k));
    }
    var name: [32]u8 = undefined;
    mount.report(try std.fmt.bufPrint(&name, "local-mount-{s}", .{label}));
    click.report(try std.fmt.bufPrint(&name, "local-click-{s}", .{label}));
}
