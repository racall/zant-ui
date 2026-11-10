//! Named handlers that read render locals (`tools/tsx/testdata/named_handlers.tsx`):
//! each place they are passed captures that render's values.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_named_handlers");

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

test "tsx named handlers capture render locals" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("0 (step 1)", text(runtime, "count"));
    try click(runtime, &renderer, "bump");
    try click(runtime, &renderer, "again");
    try std.testing.expectEqualStrings("2 (step 1)", text(runtime, "count"));
    // announce calls bump(): its own copy captures `step` through the handler
    try click(runtime, &renderer, "announce");
    try std.testing.expectEqualStrings("3 (step 1)", text(runtime, "count"));
    try std.testing.expectEqualStrings("Hello ", text(runtime, "said"));
    // a callback prop: the child's + calls onStep(1) with this render's step
    try click(runtime, &renderer, "plus");
    try std.testing.expectEqualStrings("4 (step 1)", text(runtime, "count"));
}
