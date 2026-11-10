//! Declarative list scrolling (`tools/tsx/testdata/list_scroll.tsx`):
//! scrollToIndex follows the selection, a new layoutKey scrolls again.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_list_scroll");

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

fn shown(runtime: *Runtime, n: usize) bool {
    var buf: [16]u8 = undefined;
    const key = std.fmt.bufPrint(&buf, "row-{d}", .{n}) catch return false;
    const r = runtime.tree.boundsOfKey(key) orelse return false;
    const list = runtime.tree.boundsOfKey("list") orelse return false;
    return r.y >= list.y and r.y + r.height <= list.y + list.height;
}

/// A wheel scroll over the list (by hand, away from the selection).
fn wheel(runtime: *Runtime, renderer: *Renderer, dy: f32) void {
    const list = runtime.tree.boundsOfKey("list").?;
    const x = list.x + list.width * 0.5;
    const y = list.y + list.height * 0.5;
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = x, .y = y } });
    _ = runtime.dispatch(.{ .scroll = .{ .x = x, .y = y, .dx = 0, .dy = dy, .modifiers = .{} } });
    frame(runtime, renderer);
}

test "tsx list scrolls to the selection" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expect(shown(runtime, 0));
    try click(runtime, &renderer, "jump");
    frame(runtime, &renderer);
    try std.testing.expect(shown(runtime, 150));
    try std.testing.expect(!shown(runtime, 0));
    try click(runtime, &renderer, "top");
    frame(runtime, &renderer);
    try std.testing.expect(shown(runtime, 0));
    // scrolled away by hand; the same index doesn't scroll back...
    wheel(runtime, &renderer, -2000);
    try std.testing.expect(!shown(runtime, 0));
    try click(runtime, &renderer, "top");
    frame(runtime, &renderer);
    try std.testing.expect(!shown(runtime, 0));
    // ...a new search (layoutKey) does
    try click(runtime, &renderer, "search");
    frame(runtime, &renderer);
    try std.testing.expect(shown(runtime, 0));
}
