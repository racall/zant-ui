//! JSX spread attributes end to end (`tools/tsx/testdata/spread.tsx`).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_spread");

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

test "tsx spread attributes" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    // spread variant / size; the later `variant` wins on "b"
    const a = runtime.tree.get(runtime.tree.idOfKey("a").?).data.button;
    const b = runtime.tree.get(runtime.tree.idOfKey("b").?).data.button;
    try std.testing.expectEqual(zant.node.ButtonType.primary, a.button_type);
    try std.testing.expectEqual(zant.node.ButtonType.default, b.button_type);
    try std.testing.expectEqual(zant.node.ButtonSize.small, b.size);

    try click(runtime, &renderer, "a");
    try click(runtime, &renderer, "a");
    try std.testing.expectEqualStrings("Clicks: 2", text(runtime, "Clicks"));
    try std.testing.expectEqualStrings("Override: 2", text(runtime, "Override"));
    // onClick spread from an object literal
    try click(runtime, &renderer, "b");
    try std.testing.expectEqualStrings("Clicks: 0", text(runtime, "Clicks"));
}
