//! A TS + TSX app built like `zant create` builds one (`examples/tsx/app`,
//! `addTsxApp`): the generated entry's root component, run through the same
//! shell `zant.tsx.app.run` uses.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const app = @import("tsx_app");

const W = 480;
const H = 360;
const Shell = zant.tsx.app.Shell(app.entry.App);

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
    const self: *Shell = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx app: generated entry, config and components" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    try std.testing.expectEqualStrings("TSX App", app.config.title);
    var shell = Shell{ .forced = zant.tokens.antLight() };
    const runtime = try Runtime.init(std.testing.allocator, &shell, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    try click(runtime, &renderer, "plus-Apples");
    try click(runtime, &renderer, "plus-Apples");
    try click(runtime, &renderer, "minus-Pears");
    try std.testing.expectEqualStrings("Apples: 2", text(runtime, "label-Apples"));
    try std.testing.expectEqualStrings("Pears: -1", text(runtime, "label-Pears"));
    try std.testing.expectEqualStrings("Total: 1", text(runtime, "total"));
}
