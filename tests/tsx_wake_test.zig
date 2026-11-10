//! useWake end to end (`tests/tsx/zig_types/Worker.tsx` + `jobs.zig`): a
//! native module's thread posts a wake; the hook reads its result.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("wake_tsx");

const W = 240;
const H = 120;
const Shell = zant.tsx.app.Shell(gen.WakeApp);

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, W, H, 1.0);
    renderer.end();
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *Shell = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx useWake reads a background result" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var shell = Shell{ .forced = zant.tokens.antLight() };
    const runtime = try Runtime.init(std.testing.allocator, &shell, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    // the mount effect starts the worker
    frame(runtime, &renderer);
    var tries: usize = 0;
    while (shell.root.value == 0 and tries < 100_000) : (tries += 1) {
        shell.onWake();
        std.Thread.yield() catch {};
    }
    try std.testing.expectEqual(@as(u32, 42), shell.root.value);
    runtime.invalidate();
    frame(runtime, &renderer);
    const id = runtime.tree.idOfKey("value").?;
    try std.testing.expectEqualStrings("42", runtime.tree.get(id).data.text.content);
}
