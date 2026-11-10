//! createStore / useStore end to end (`tools/tsx/testdata/store.tsx`): every
//! component reading the store sees a change made anywhere.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_store");

const W = 520;
const H = 360;

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

test "tsx store: shared state across components" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    defer zant.tsx.deinitFields(&app);
    defer gen.settings.state.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();

    // the mount effect writes the store, which asks for another build
    frame(runtime, &renderer);
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("guest: light, 14px, 0 recent", text(runtime, "summary"));

    // FontSize changes it; App and FontSize both show the new value
    try click(runtime, &renderer, "bigger");
    try click(runtime, &renderer, "bigger");
    try click(runtime, &renderer, "smaller");
    try std.testing.expectEqualStrings("15px", text(runtime, "size"));
    try std.testing.expectEqualStrings("guest: light, 15px, 0 recent", text(runtime, "summary"));

    // ThemeToggle's switch
    try click(runtime, &renderer, "dark");
    try std.testing.expect(gen.settings.get().dark);
    try std.testing.expectEqualStrings("guest: dark, 15px, 0 recent", text(runtime, "summary"));

    // set() with a spread of the current value
    try click(runtime, &renderer, "remember");
    try click(runtime, &renderer, "remember");
    try std.testing.expectEqualStrings("guest: dark, 15px, 2 recent", text(runtime, "summary"));
    try std.testing.expectEqualStrings("size 15", gen.settings.get().recent[1]);
}
