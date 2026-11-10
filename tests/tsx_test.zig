//! End to end: `examples/tsx/counter.tsx` compiled by zant-tsx during the
//! build, driven through the runtime like a user would.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const Counter = @import("counter_tsx").Counter;

const W = 520;
const H = 480;

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

/// Text of a live text / button node by key; null when it is not shown.
fn textOf(runtime: *Runtime, key: []const u8) ?[]const u8 {
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive or node.exiting) continue;
        const k = node.key orelse continue;
        if (!std.mem.eql(u8, k, key)) continue;
        return switch (node.data) {
            .text => |d| d.content,
            .button => |d| d.label,
            else => null,
        };
    }
    return null;
}

const Render = struct {
    fn render(ctx: *anyopaque) ui.Node {
        const self: *Counter = @ptrCast(@alignCast(ctx));
        return self.render();
    }
};

test "tsx counter: state, events, derived values and conditional children" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var state = Counter{};
    const runtime = try Runtime.init(std.testing.allocator, &state, Render.render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    try std.testing.expectEqualStrings("Hello, Zant!", textOf(runtime, "title").?);
    try std.testing.expectEqualStrings("Count: 0 (×2 = 0)", textOf(runtime, "count").?);
    try std.testing.expectEqualStrings("+1", textOf(runtime, "inc").?);
    try std.testing.expect(textOf(runtime, "note") != null);
    try std.testing.expect(textOf(runtime, "many") == null);

    try click(runtime, &renderer, "inc");
    try click(runtime, &renderer, "inc");
    try std.testing.expectEqual(@as(u32, 2), state.count);
    try std.testing.expectEqualStrings("Count: 2 (×2 = 4)", textOf(runtime, "count").?);

    // functional update: step 1 → 5
    try click(runtime, &renderer, "step");
    try std.testing.expectEqual(@as(u32, 5), state.step);
    try std.testing.expectEqualStrings("+5", textOf(runtime, "inc").?);
    try click(runtime, &renderer, "inc");
    try click(runtime, &renderer, "inc");
    try std.testing.expectEqual(@as(u32, 12), state.count);
    try std.testing.expect(textOf(runtime, "many") != null);

    try click(runtime, &renderer, "reset");
    try std.testing.expectEqual(@as(u32, 0), state.count);
    try std.testing.expect(textOf(runtime, "many") == null);

    // setter passed straight to onChange, and an inline arrow with a param
    try click(runtime, &renderer, "large");
    try std.testing.expect(state.large);
    try click(runtime, &renderer, "notify");
    try std.testing.expect(!state.notify);
    try std.testing.expect(textOf(runtime, "note") == null);

    // string state through the input
    frame(runtime, &renderer);
    const name_box = runtime.tree.boundsOfKey("name").?;
    const p = zant.events.PointerEvent{ .x = name_box.x + name_box.width - 4, .y = name_box.y + name_box.height * 0.5 };
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    _ = runtime.dispatch(.{ .text_input = "!" });
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("Zant!", state.name.get());
    try std.testing.expectEqualStrings("Hello, Zant!!", textOf(runtime, "title").?);
    state.name.deinit();
}
