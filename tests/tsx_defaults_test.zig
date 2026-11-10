//! Default values end to end (`tools/tsx/testdata/defaults.tsx`): parameters
//! left out by calls, optional props left unset by the parent.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_defaults");

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

fn size(runtime: *Runtime, key: []const u8) f32 {
    const id = runtime.tree.idOfKey(key) orelse return 0;
    return runtime.tree.get(id).data.text.style.font_size;
}

test "tsx default values" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    // label(count): unit "item" and plural by default; Badge size 14 by default
    try std.testing.expectEqualStrings("0 items", text(runtime, "count"));
    try std.testing.expectEqualStrings("0 file", text(runtime, "files"));
    try std.testing.expectEqual(@as(f32, 14), size(runtime, "count"));
    try std.testing.expectEqual(@as(f32, 20), size(runtime, "files"));
    try click(runtime, &renderer, "one");
    try std.testing.expectEqualStrings("1 item", text(runtime, "count"));
    try click(runtime, &renderer, "step"); // add() adds the default, the constant step
    try std.testing.expectEqualStrings("6 items", text(runtime, "count"));
}
