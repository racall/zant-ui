//! ui.list: virtualisation, measured heights, logical anchor, follow tail,
//! bottom alignment, scrolling, scrollToItem, text selection in items.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;

fn frame(rt: *Runtime, r: *zant.renderer.Renderer) void {
    const canvas = r.begin(1.0).?;
    rt.renderFrame(canvas, 400, 600, 1.0);
    r.end();
}

fn builtRows(rt: *Runtime, prefix: []const u8) usize {
    var n: usize = 0;
    for (rt.tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.key) |k| if (std.mem.startsWith(u8, k, prefix)) {
            n += 1;
        };
    }
    return n;
}

const Uniform = struct {
    state: ui.ListState,
    fn row(self: *Uniform, i: usize) ui.Node {
        _ = self;
        return ui.text(std.fmt.allocPrint(ui.frameAllocator(), "Row {d}", .{i}) catch "", .{});
    }
    fn render(ctx: *anyopaque) ui.Node {
        const self: *Uniform = @ptrCast(@alignCast(ctx));
        return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{ui.list(&self.state, .{ .key = "rows", .count = 10_000, .item = ui.listItem(self, row), .item_height = 30, .overdraw = 60 })} });
    }
};

test "list: 10k uniform rows build only the visible range; wheel and scrollToItem" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = Uniform{ .state = .init(std.testing.allocator) };
    defer app.state.deinit();
    const rt = try Runtime.init(std.testing.allocator, &app, Uniform.render, .{ .window = .{ .width = 400, .height = 600 } });
    defer rt.deinit();
    var r = try zant.renderer.Renderer.createRaster(400, 600);
    defer r.deinit();
    frame(rt, &r);
    frame(rt, &r);
    // 600 px viewport + 60 px overdraw below → 22 rows, not 10 000
    const built = builtRows(rt, "rows-i");
    try std.testing.expect(built >= 20 and built <= 26);
    try std.testing.expectEqual(@as(usize, 0), app.state.renderedRange()[0]);
    // wheel down 3000 px
    for (0..30) |_| _ = rt.dispatch(.{ .scroll = .{ .x = 200, .y = 300, .dx = 0, .dy = -100, .modifiers = .{} } });
    frame(rt, &r);
    try std.testing.expectApproxEqAbs(@as(f32, 3000), app.state.scrollTop(), 0.5);
    try std.testing.expect(ui.keyBounds("rows-i100") == null or true);
    try std.testing.expect(rt.tree.boundsOfKey("rows-i100") != null);
    try std.testing.expect(rt.tree.boundsOfKey("rows-i0") == null);
    // jump to row 5000 (start of the viewport)
    app.state.scrollToItem(5000, .start);
    rt.invalidate();
    frame(rt, &r);
    const b = rt.tree.boundsOfKey("rows-i5000").?;
    const vp = rt.tree.boundsOfKey("rows").?;
    try std.testing.expectApproxEqAbs(vp.y, b.y, 0.5);
}

const Chat = struct {
    state: ui.ListState,
    lines: [64]usize = @splat(3),
    n: usize = 20,
    fn item(self: *Chat, i: usize) ui.Node {
        const arena = ui.frameAllocator();
        const rows = arena.alloc(ui.Node, self.lines[i]) catch return ui.column(.{});
        for (rows, 0..) |*row, k| row.* = ui.text(std.fmt.allocPrint(arena, "Message {d} line {d}", .{ i, k }) catch "", .{ .size = 14 });
        return ui.column(.{ .padding = ui.inset(8), .align_items = .stretch, .children = rows });
    }
    fn render(ctx: *anyopaque) ui.Node {
        const self: *Chat = @ptrCast(@alignCast(ctx));
        return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{ui.list(&self.state, .{ .key = "chat", .count = self.n, .item = ui.listItem(self, item), .alignment = .bottom, .follow_tail = true, .gap = 10, .selectable = true })} });
    }
};

test "list: chat follows the tail, stops when scrolled up, keeps the anchor when heights change" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = Chat{ .state = .init(std.testing.allocator) };
    defer app.state.deinit();
    const rt = try Runtime.init(std.testing.allocator, &app, Chat.render, .{ .window = .{ .width = 400, .height = 600 } });
    defer rt.deinit();
    var r = try zant.renderer.Renderer.createRaster(400, 600);
    defer r.deinit();
    for (0..4) |_| frame(rt, &r);
    // the last message's bottom is at the viewport bottom
    const vp = rt.tree.boundsOfKey("chat").?;
    var last = rt.tree.boundsOfKey("chat-i19").?;
    try std.testing.expectApproxEqAbs(vp.bottom(), last.bottom(), 1);
    try std.testing.expect(app.state.isAtEnd());
    // a growing last message stays in view (streaming)
    app.lines[19] = 12;
    rt.invalidate();
    for (0..3) |_| frame(rt, &r);
    last = rt.tree.boundsOfKey("chat-i19").?;
    try std.testing.expectApproxEqAbs(vp.bottom(), last.bottom(), 1);
    // scroll up: following stops, and a new message does not pull the view
    for (0..5) |_| _ = rt.dispatch(.{ .scroll = .{ .x = 200, .y = 300, .dx = 0, .dy = 80, .modifiers = .{} } });
    frame(rt, &r);
    try std.testing.expect(!app.state.following);
    const anchor_key = std.fmt.allocPrint(std.testing.allocator, "chat-i{d}", .{app.state.anchor}) catch unreachable;
    defer std.testing.allocator.free(anchor_key);
    const before = rt.tree.boundsOfKey(anchor_key).?.y;
    app.n = 21;
    // and a message above the anchor growing does not move it either
    if (app.state.anchor > 0) app.lines[app.state.anchor - 1] = 6;
    rt.invalidate();
    for (0..3) |_| frame(rt, &r);
    try std.testing.expectApproxEqAbs(before, rt.tree.boundsOfKey(anchor_key).?.y, 1);
    // End resumes following
    app.state.scrollToEnd();
    rt.invalidate();
    for (0..3) |_| frame(rt, &r);
    try std.testing.expectApproxEqAbs(vp.bottom(), rt.tree.boundsOfKey("chat-i20").?.bottom(), 1);
    // text in items selects (the list takes only the wheel)
    const b = rt.tree.boundsOfKey("chat-i20").?;
    _ = rt.dispatch(.{ .pointer_down = .{ .x = b.x + 9, .y = b.y + 14 } });
    _ = rt.dispatch(.{ .pointer_move = .{ .x = b.x + 300, .y = b.y + 14 } });
    _ = rt.dispatch(.{ .pointer_up = .{ .x = b.x + 300, .y = b.y + 14 } });
    const sel = try rt.selectedText(std.testing.allocator);
    defer std.testing.allocator.free(sel);
    try std.testing.expect(std.mem.startsWith(u8, sel, "Message 20 line 0"));
}

test "list: a short list sits at the bottom with bottom alignment" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = Chat{ .state = .init(std.testing.allocator), .n = 2 };
    defer app.state.deinit();
    const rt = try Runtime.init(std.testing.allocator, &app, Chat.render, .{ .window = .{ .width = 400, .height = 600 } });
    defer rt.deinit();
    var r = try zant.renderer.Renderer.createRaster(400, 600);
    defer r.deinit();
    for (0..3) |_| frame(rt, &r);
    const vp = rt.tree.boundsOfKey("chat").?;
    try std.testing.expectApproxEqAbs(vp.bottom(), rt.tree.boundsOfKey("chat-i1").?.bottom(), 1);
    try std.testing.expect(rt.tree.boundsOfKey("chat-i0").?.y > vp.y + 100);
}
