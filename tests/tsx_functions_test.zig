//! Plain top-level functions end to end (`tools/tsx/testdata/functions.tsx`):
//! called while rendering, from handlers, as a handler, and changing a store.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_functions");

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

test "tsx plain functions" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("0 clicks, 1 digits, 0 KB", text(runtime, "label"));
    try std.testing.expectEqualStrings("0 items", text(runtime, "badge"));

    try click(runtime, &renderer, "inc");
    try std.testing.expectEqualStrings("1 click, 3 digits, 1 KB", text(runtime, "label"));
    try std.testing.expectEqualStrings("1 item", text(runtime, "badge"));
    // clamp() keeps the count at 5, the badge at 3
    for (0..6) |_| try click(runtime, &renderer, "inc");
    try std.testing.expectEqualStrings("5 clicks, 3 digits, 5 KB", text(runtime, "label"));
    try std.testing.expectEqualStrings("3 items", text(runtime, "badge"));

    // a store changed from a handler, then by a plain function as the handler
    try click(runtime, &renderer, "unit");
    try std.testing.expectEqualStrings("5 clicks, 3 digits, 5 MB", text(runtime, "label"));
    try click(runtime, &renderer, "reset");
    try std.testing.expectEqualStrings("5 clicks, 3 digits, 5 KB", text(runtime, "label"));
}
