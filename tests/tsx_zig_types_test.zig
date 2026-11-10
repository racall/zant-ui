//! Zig structs and enums used from TSX (`tests/tsx/zig_types/`): fields,
//! nested structs, enums compared with string literals, slices, optionals
//! and functions taking / returning them.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const Devices = @import("zig_types_tsx").Devices;

const W = 480;
const H = 320;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, W, H, 1.0);
    renderer.end();
}

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    return runtime.tree.get(id).data.text.content;
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *Devices = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx uses Zig structs and enums" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = Devices{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("💻 Mac 10.0.0.2:7070 1 1", text(runtime, "Mac"));
    try std.testing.expectEqualStrings("🖥 PC 10.0.0.3:7070 0 -1", text(runtime, "PC"));
    try std.testing.expectEqualStrings("Mac", text(runtime, "first"));

    const r = runtime.tree.boundsOfKey("only").?;
    const p = zant.events.PointerEvent{ .x = r.x + 4, .y = r.y + 4 };
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("", text(runtime, "PC"));
}
