//! Custom hooks (`tests/tsx/hooks/`): inlined into the root (its state) and
//! into a child component (its local state, one per instance).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("hooks_tsx");

const W = 480;
const H = 480;

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
    frame(runtime, renderer);
}

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    const node = runtime.tree.get(id);
    return switch (node.data) {
        .text => |t| t.content,
        else => "",
    };
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *gen.App = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx custom hooks" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("2", text(runtime, "count"));
    try std.testing.expectEqualStrings("10", text(runtime, "other"));
    try std.testing.expectEqualStrings("off 0", text(runtime, "on"));
    // two calls of one hook: two states
    try click(runtime, &renderer, "inc");
    try click(runtime, &renderer, "inc");
    try click(runtime, &renderer, "inc-other");
    try std.testing.expectEqualStrings("4", text(runtime, "count"));
    try std.testing.expectEqualStrings("11", text(runtime, "other"));
    try click(runtime, &renderer, "reset");
    try std.testing.expectEqualStrings("2", text(runtime, "count"));
    // a hook calling a hook, and the effect inside it
    try click(runtime, &renderer, "toggle");
    try std.testing.expectEqualStrings("on 1", text(runtime, "on"));
    try click(runtime, &renderer, "toggle");
    try click(runtime, &renderer, "toggle");
    try std.testing.expectEqualStrings("on 2", text(runtime, "on"));
    // each child keeps its own hook state
    try click(runtime, &renderer, "clicker-a");
    try click(runtime, &renderer, "clicker-a");
    try click(runtime, &renderer, "clicker-b");
    try std.testing.expectEqualStrings("a 102", text(runtime, "clicker-text-a"));
    try std.testing.expectEqualStrings("b 101", text(runtime, "clicker-text-b"));
}
