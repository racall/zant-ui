//! Child component state (`useState` in a child → `ui.localState`): every
//! instance keeps its own state across frames, keyed instances keep theirs
//! when siblings leave, and an instance's slot is freed when it unmounts.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const tsx = zant.tsx;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_local_state");

const W = 640;
const H = 640;

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
    const self: *gen.App = @ptrCast(@alignCast(ctx));
    return self.render();
}

/// The local state of the keyed `<Counter key={id}>` under the root.
fn counter(runtime: *Runtime, id: u32) ?*gen.Counter._Local {
    const entry = runtime.local_states.entries.get(tsx.childId(0, "Counter", id)) orelse return null;
    return @ptrCast(@alignCast(entry.ptr));
}

test "tsx child components keep local state per instance" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    defer app.ids.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    // #1, #2, the section's two counters and the feed's list
    try std.testing.expectEqual(@as(usize, 5), runtime.local_states.count());

    try click(runtime, &renderer, "inc-#2");
    try click(runtime, &renderer, "inc-#2");
    try click(runtime, &renderer, "inc-Section 2");
    try std.testing.expectEqual(@as(f64, 0), counter(runtime, 1).?.count);
    try std.testing.expectEqual(@as(f64, 2), counter(runtime, 2).?.count);

    // string and array state in a child
    try click(runtime, &renderer, "draft-#2");
    _ = runtime.dispatch(.{ .text_input = "hello" });
    _ = runtime.dispatch(.{ .key_down = .{ .key = .enter } });
    frame(runtime, &renderer);
    try std.testing.expectEqual(@as(usize, 1), counter(runtime, 2).?.log.get().len);
    try std.testing.expectEqualStrings("hello", counter(runtime, 2).?.log.get()[0]);
    try std.testing.expectEqualStrings("", counter(runtime, 2).?.draft.get());

    // #1 unmounts: its slot goes, #2 keeps its count under its key
    try click(runtime, &renderer, "drop");
    try std.testing.expect(counter(runtime, 1) == null);
    try std.testing.expectEqual(@as(f64, 2), counter(runtime, 2).?.count);
    try std.testing.expectEqual(@as(usize, 4), runtime.local_states.count());
}
