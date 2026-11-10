//! Child components end to end: `tools/tsx/testdata/children.tsx` (compiled
//! by zant-tsx during the build) driven through the runtime. Callbacks reach
//! the parent through a method, an inline arrow and a forwarded ui event.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const golden = @import("tsx_golden_children");
const App = golden.App;

const W = 480;
const H = 400;

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

fn render(ctx: *anyopaque) ui.Node {
    const self: *App = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx child components call back into the parent" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = App{};
    defer app.todos.deinit();
    defer app.last.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    // TodoRow's checkbox → onToggle={toggle} (a parent method, u32 argument)
    try click(runtime, &renderer, "check-1");
    try std.testing.expect(app.todos.get()[0].done);
    try click(runtime, &renderer, "check-1");
    try std.testing.expect(!app.todos.get()[0].done);

    // TodoRow's button → onRemove={(id) => ...} (an inline arrow)
    try click(runtime, &renderer, "remove-1");
    try std.testing.expectEqual(@as(usize, 0), app.todos.get().len);
    try std.testing.expect(runtime.tree.boundsOfKey("check-1") == null);

    // Panel forwards onClear to a ui button: onClick={props.onClear}
    app.todos.set(&.{
        .{ .id = 7, .title = "a", .done = false },
        .{ .id = 8, .title = "b", .done = true },
    });
    runtime.invalidate();
    frame(runtime, &renderer);
    try std.testing.expect(runtime.tree.boundsOfKey("check-8") != null);
    try click(runtime, &renderer, "clear");
    try std.testing.expectEqual(@as(usize, 0), app.todos.get().len);
}
