const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

const State = struct {
    clicks: usize = 0,
    fn click(self: *State) void {
        self.clicks += 1;
    }
};

test "icons build for every name" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    for (std.enums.values(ui.Icon)) |name| {
        const node = ui.icon(.{ .name = name, .size = 18 });
        try std.testing.expectEqual(@as(f32, 18), node.element.drawing.?.width);
    }
}

test "dropdown menu, segmented and dismiss layer" {
    var state = State{};
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const menu = ui.dropdownMenu(.{ .items = &.{
        .{ .label = "New", .icon = .file, .extra = "⌘N", .on_click = ui.action(&state, State.click) },
        ui.MenuItem.divider,
        ui.MenuItem.group("View"),
        .{ .label = "Disabled", .disabled = true },
    } });
    try std.testing.expectEqual(@as(usize, 4), menu.element.children.len);
    const seg = ui.segmented(.{ .options = &.{ .{ .label = "A" }, .{ .label = "B" } }, .selected = 1 });
    // sliding thumb + 2 options
    try std.testing.expectEqual(@as(usize, 3), seg.element.children.len);
    const layer = ui.dismissLayer(.{ .on_dismiss = ui.action(&state, State.click) });
    try std.testing.expect(layer.element.layout.absolute);
    try std.testing.expectEqual(zant.node.ButtonType.plain, layer.element.button_type);
}

test "block_pointer containers swallow clicks on empty areas" {
    var tree = try zant.tree.Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const backdrop = try tree.addContainer(tree.root, .{ .width = 200, .height = 100 });
    tree.get(backdrop).visual.block_pointer = true;
    const plain = try tree.addContainer(tree.root, .{ .width = 200, .height = 100 });
    tree.layout(.{ .width = 300, .height = 300 });
    try std.testing.expectEqual(backdrop, tree.hitTest(10, 10).?);
    try std.testing.expect(tree.hitTest(10, tree.get(plain).bounds.y + 10) == null);
}

test "dismiss layer paints its dimming background" {
    const Dim = struct {
        fn render(_: *anyopaque) ui.Node {
            return ui.column(.{ .grow = 1, .background = 0xffffffff, .children = &.{
                ui.dismissLayer(.{ .background = 0xff000000 }),
            } });
        }
    };
    var dummy: u8 = 0;
    const runtime = try zant.ui_runtime.Runtime.init(std.testing.allocator, &dummy, Dim.render, .{ .window = .{ .width = 100, .height = 100 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(100, 100);
    defer renderer.deinit();
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 100, 100, 1.0);
    renderer.end();
    try std.testing.expectEqual(@as(u32, 0xff000000), renderer.pixelAt(50, 50).?);
}

test "modal: mask closes, card swallows clicks, shortcut list balances columns" {
    const Help = struct {
        open: bool = true,
        fn close(self: *@This()) void {
            self.open = false;
        }
        fn render(ctx: *anyopaque) ui.Node {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            if (!self.open) return ui.column(.{ .grow = 1, .children = &.{} });
            return ui.column(.{ .grow = 1, .children = &.{ui.modal(.{
                .key = "help",
                .title = "Keys",
                .width = 300,
                .on_close = ui.action(self, close),
                .content = ui.shortcutList(.{ .key = "keys", .column_width = 120, .groups = &.{
                    .{ .title = "A", .items = &.{ .{ .label = "one", .keys = "⌘1" }, .{ .label = "two", .keys = "⌘2" } } },
                    .{ .title = "B", .items = &.{.{ .label = "three", .keys = "⌘3" }} },
                } }),
            })} });
        }
    };
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var state = Help{};
    const runtime = try zant.ui_runtime.Runtime.init(std.testing.allocator, &state, Help.render, .{ .window = .{ .width = 600, .height = 400 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(600, 400);
    defer renderer.deinit();
    const frame = struct {
        fn f(r: *zant.ui_runtime.Runtime, ren: *zant.renderer.Renderer) void {
            const canvas = ren.begin(1.0).?;
            r.renderFrame(canvas, 600, 400, 1.0);
            ren.end();
        }
    }.f;
    frame(runtime, &renderer);
    var card: ?*zant.node.Node = null;
    var list: ?*zant.node.Node = null;
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.key) |k| {
            if (std.mem.eql(u8, k, "help-card")) card = node;
            if (std.mem.eql(u8, k, "keys")) list = node;
        }
    }
    // two groups → two columns
    try std.testing.expectEqual(@as(usize, 2), list.?.children.items.len);
    const c = card.?.bounds;
    try std.testing.expectApproxEqAbs(@as(f32, 300), c.width, 0.5);
    // clicking inside the card does nothing; the mask closes
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = c.x + 10, .y = c.y + c.height - 5 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = c.x + 10, .y = c.y + c.height - 5 } });
    try std.testing.expect(state.open);
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 5, .y = 5 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 5, .y = 5 } });
    try std.testing.expect(!state.open);
}

test "message is a pass-through pill with a drop-in presence" {
    ui.beginFrame();
    defer ui.endFrame();
    const node = ui.message(.{ .key = "msg", .text = "Saved", .kind = .err, .top = 60 });
    try std.testing.expect(node.element.layout.absolute);
    try std.testing.expectEqual(@as(?f32, 60), node.element.layout.top);
    try std.testing.expectEqual(zant.motion.Presence.move_up, node.element.motion.presence);
    // the full-width row itself never catches clicks
    try std.testing.expect(!node.element.visual.block_pointer);
}
