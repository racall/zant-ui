//! Optionals narrowed by conditions end to end (`tools/tsx/testdata/narrowing.tsx`):
//! rendered while null (the guards keep `.?` from running) and with a value.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_narrowing");

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

test "tsx optional narrowing" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    // null: the else branches, the child's early return, no file buttons
    try std.testing.expectEqualStrings("untitled 0 empty", text(runtime, "title"));
    // `<PeerCard key="card">` keys the child's root: here its early-return text
    try std.testing.expectEqualStrings("No peer", text(runtime, "card"));
    try std.testing.expectEqualStrings("alone", text(runtime, "describe"));
    try std.testing.expect(runtime.tree.idOfKey("files") == null);
    try click(runtime, &renderer, "connect"); // returns early: label stays null
    try std.testing.expectEqualStrings("untitled 0 empty", text(runtime, "title"));

    try click(runtime, &renderer, "add");
    try std.testing.expectEqualStrings("untitled 2 has files", text(runtime, "title"));
    try std.testing.expectEqualStrings("mac (2)", text(runtime, "name"));
    try std.testing.expectEqualStrings("MAC+files", text(runtime, "describe"));
    try std.testing.expect(runtime.tree.idOfKey("note") == null);
    // an arrow made in the narrowed branch captured `current`
    try click(runtime, &renderer, "b.png");
    try std.testing.expectEqualStrings("mac/b.png", text(runtime, "picked"));
    try click(runtime, &renderer, "connect");
    try std.testing.expectEqualStrings("mac 2 has files", text(runtime, "title"));
    try std.testing.expectEqualStrings("mac", text(runtime, "note"));
}
