//! `<List>` end to end: `tools/tsx/testdata/list.tsx` with 10 000 items
//! driven through the runtime. Only the rows on screen are built, scrolling
//! builds others, and a row's button reaches its own item.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const Feed = @import("tsx_golden_list").Feed;
const Item = @import("tsx_golden_list").Item;

const W = 480;
const H = 600;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, W, H, 1.0);
    renderer.end();
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *Feed = @ptrCast(@alignCast(ctx));
    return self.render();
}

fn liveKeysWithPrefix(runtime: *Runtime, prefix: []const u8) usize {
    var n: usize = 0;
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.key) |k| if (std.mem.startsWith(u8, k, prefix)) {
            n += 1;
        };
    }
    return n;
}

test "tsx List builds only the visible rows of 10 000" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var feed = Feed{};
    defer feed.items.deinit();
    defer feed._list1.deinit();
    defer feed._list3.deinit();
    const items = try std.testing.allocator.alloc(Item, 10_000);
    defer std.testing.allocator.free(items);
    for (items, 0..) |*item, i| item.* = .{ .id = @intCast(i + 1), .title = "Row", .done = false };
    feed.items.set(items);

    const runtime = try Runtime.init(std.testing.allocator, &feed, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    frame(runtime, &renderer);

    const built = liveKeysWithPrefix(runtime, "row-");
    try std.testing.expect(built > 0);
    try std.testing.expect(built < 100); // a screenful plus overdraw, not 10 000

    // the first row's button picks item 1
    const pick = runtime.tree.boundsOfKey("pick-1") orelse return error.MissingKey;
    const p = zant.events.PointerEvent{ .x = pick.x + pick.width * 0.5, .y = pick.y + pick.height * 0.5 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, &renderer);
    try std.testing.expectEqual(@as(u32, 1), feed.picked);

    // scrolling far down builds rows from the middle of the list
    for (0..60) |_| {
        _ = runtime.dispatch(.{ .scroll = .{ .x = p.x, .y = p.y, .dx = 0, .dy = -400, .modifiers = .{} } });
        frame(runtime, &renderer);
    }
    try std.testing.expect(runtime.tree.boundsOfKey("row-1") == null);
    try std.testing.expect(liveKeysWithPrefix(runtime, "row-") < 100);
}
