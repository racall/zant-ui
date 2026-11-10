//! App hooks end to end (`tools/tsx/testdata/app_hooks.tsx`), through the
//! shell a TS + TSX app runs in (`zant.tsx.app.Shell`).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_app_hooks");

const W = 360;
const H = 200;
const Shell = zant.tsx.app.Shell(gen.App);

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
    const self: *Shell = @ptrCast(@alignCast(ctx));
    return self.render();
}

fn onKey(ctx: *anyopaque, k: zant.events.KeyEvent) bool {
    const self: *Shell = @ptrCast(@alignCast(ctx));
    return self.onKey(k);
}

test "tsx app hooks: close request and keys" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var shell = Shell{ .forced = zant.tokens.antLight() };
    defer shell.root.last.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &shell, render, .{
        .window = .{ .width = W, .height = H },
        .key_fn = onKey,
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("dirty 0 ", text(runtime, "state"));

    // dirty: the close is refused (and counted); quitting asks the same
    try std.testing.expect(!shell.onCloseRequest());
    try std.testing.expect(!shell.onQuitRequest());
    runtime.invalidate();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("dirty 2 ", text(runtime, "state"));

    // keys reach useKeyDown through the runtime
    _ = runtime.dispatch(.{ .key_down = .{ .key = .{ .character = 'x' } } });
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("dirty 2 x", text(runtime, "state"));
    const mod: zant.events.Modifiers = if (zant.keys.is_macos) .{ .meta = true } else .{ .control = true };
    try std.testing.expect(shell.onKey(.{ .key = .{ .character = 's' }, .modifiers = mod }));
    _ = runtime.dispatch(.{ .key_down = .{ .key = .escape } });
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("clean 2 Escape", text(runtime, "state"));
    try std.testing.expect(shell.onCloseRequest());
}
