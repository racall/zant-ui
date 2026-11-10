//! Types across files (`tests/tsx/crossfile/`): props interfaces using a type
//! imported from Zig, and components passing it on to each other.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("crossfile_tsx");

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

test "tsx types across files" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("0", text(runtime, "picked"));
    try std.testing.expectEqualStrings("#12", text(runtime, "consts"));
    // the second row comes through Rows (another file, a virtual list) with
    // the Zig type, and Rows forwards its own callback prop
    // the list is Rows' root and gets Rows' key ("all"): it still finds its viewport
    frame(runtime, &renderer);
    try click(runtime, &renderer, "pick-2");
    try std.testing.expectEqualStrings("2", text(runtime, "picked"));
}
