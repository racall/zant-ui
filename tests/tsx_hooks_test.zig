//! useRef / useMemo / useEffect end to end (`tools/tsx/testdata/hooks.tsx`):
//! effects run after the build and rebuild only when they set state, memos
//! recompute only when their dependencies change, refs persist without
//! rebuilding.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_hooks");

const W = 520;
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

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    return runtime.tree.get(id).data.text.content;
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *gen.App = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx hooks: effects, memos and refs" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    defer zant.tsx.deinitFields(&app);
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();

    // the mount effect loads the first item and asks for a rebuild
    frame(runtime, &renderer);
    try std.testing.expect(runtime.needs_build);
    frame(runtime, &renderer);
    try std.testing.expect(app.loaded);
    try std.testing.expectEqualStrings("1 shown", text(runtime, "visible"));
    try std.testing.expectEqualStrings("1 / 1 done", text(runtime, "stats"));
    // the child's ref-only effect runs every build without rebuilding again
    frame(runtime, &renderer);
    try std.testing.expect(!runtime.needs_build);

    // unchanged dependencies: the cached value is reused
    const cached = app._memo10.value.?.ptr;
    runtime.invalidate();
    frame(runtime, &renderer);
    try std.testing.expectEqual(cached, app._memo10.value.?.ptr);

    try click(runtime, &renderer, "add");
    try click(runtime, &renderer, "add");
    try std.testing.expectEqualStrings("3 shown", text(runtime, "visible"));
    try std.testing.expectEqualStrings("1 / 3 done", text(runtime, "stats"));
    try std.testing.expectEqualStrings("Item 2", app.items.get()[2].title);
    try std.testing.expectEqual(@as(u32, 3), app.nextId);

    // the query effect copies the query into a ref
    try click(runtime, &renderer, "query");
    _ = runtime.dispatch(.{ .text_input = "Item 1" });
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("1 shown", text(runtime, "visible"));
    try std.testing.expectEqualStrings("Item 1", app.lastQuery.get());
    // the cleanup of the previous run saw its own query ("", then "Item 1"
    // typed at once: one change)
    try std.testing.expectEqual(@as(f64, 0), app.cleanups);
    _ = runtime.dispatch(.{ .text_input = "0" });
    frame(runtime, &renderer);
    try std.testing.expectEqual(@as(f64, 6), app.cleanups);

    // the ticker unmounts: its slot goes and its cleanup runs (it reads its
    // own ref through the copied component)
    const slots = runtime.local_states.count();
    try click(runtime, &renderer, "ticking");
    try std.testing.expect(runtime.tree.idOfKey("ticker-tick") == null);
    try std.testing.expectEqual(slots - 1, runtime.local_states.count());
    try click(runtime, &renderer, "ticking");
    try std.testing.expect(runtime.tree.idOfKey("ticker-tick") != null);
}
