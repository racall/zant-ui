//! `<Image image={…}>` and `<HotkeyInput value onRecord>` from TSX
//! (`tests/tsx/media/`): the generated code compiles against zant and renders.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const App = @import("media_tsx").App;

fn render(ctx: *anyopaque) ui.Node {
    const self: *App = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx in-memory images and the hotkey recorder render" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = App{};
    defer app.hotkey.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = 320, .height = 200 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(320, 200);
    defer renderer.deinit();
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 320, 200, 1.0);
    renderer.end();
    try std.testing.expect(runtime.tree.idOfKey("thumb") != null);
    try std.testing.expect(runtime.tree.idOfKey("hk") != null);
    const label = runtime.tree.get(runtime.tree.idOfKey("label").?).data.text.content;
    try std.testing.expectEqualStrings("ctrl+cmd+v", label);
}
