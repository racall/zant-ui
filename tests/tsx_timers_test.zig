//! setTimeout / clearTimeout end to end (`tools/tsx/testdata/timers.tsx`): the
//! test runs due timers itself (`tsx.timers.fire`), like the app shell on wake.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_timers");

const W = 480;
const H = 320;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, W, H, 1.0);
    renderer.end();
}

fn click(runtime: *Runtime, renderer: *Renderer, key: []const u8) !void {
    frame(runtime, renderer);
    const r = runtime.tree.boundsOfKey(key) orelse return error.MissingKey;
    const p = zant.events.PointerEvent{ .x = r.x + r.width * 0.5, .y = r.y + r.height * 0.5 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, renderer);
}

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    return runtime.tree.get(id).data.text.content;
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *gen.App = @ptrCast(@alignCast(ctx));
    return self.render();
}

fn later(ms: i64) i64 {
    return zant.platform.monotonicMs() + ms;
}

test "tsx timers" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    const timers = zant.tsx.timers;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    frame(runtime, &renderer); // the mount effect ran: its timer waits
    try std.testing.expectEqual(@as(usize, 2), timers.pending());
    try std.testing.expect(timers.fire(later(20)));
    runtime.invalidate(); // what the app's wake does after running timers
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("mounted 3", text(runtime, "note"));

    // the deadline keeps the number it captured when it was set
    try click(runtime, &renderer, "start");
    try std.testing.expect(!timers.fire(later(0)));
    try std.testing.expect(timers.fire(later(9000)));
    runtime.invalidate();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("expired 30 2", text(runtime, "note"));

    // cleared before it is due: nothing happens
    try click(runtime, &renderer, "start");
    try click(runtime, &renderer, "stop");
    // the effect's timer (captured `doubled` = 6, through the effect) is still due later
    try std.testing.expectEqual(@as(usize, 1), timers.pending());
    try std.testing.expect(timers.fire(later(30000)));
    runtime.invalidate();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("doubled 4 2", text(runtime, "note"));
}
