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

test "modal is a focus scope: Tab cycles inside the dialog only" {
    const App = struct {
        open: bool = true,
        fn renderOpaque(ctx: *anyopaque) ui.Node {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            const behind = ui.row(.{ .gap = 8, .children = &.{ ui.button(.{ .key = "behind-a", .text = "A" }), ui.button(.{ .key = "behind-b", .text = "B" }) } });
            if (!self.open) return ui.column(.{ .grow = 1, .children = &.{behind} });
            return ui.column(.{ .grow = 1, .children = &.{
                behind,
                ui.modal(.{ .key = "dlg", .title = "T", .content = ui.input(.{ .key = "dlg-input" }), .footer = ui.button(.{ .key = "dlg-ok", .text = "OK" }) }),
            } });
        }
    };
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = App{};
    const runtime = try zant.ui_runtime.Runtime.init(std.testing.allocator, &app, App.renderOpaque, .{ .window = .{ .width = 600, .height = 400 } });
    defer runtime.deinit();
    runtime.rebuildForTest();
    var seen: [8][]const u8 = undefined;
    for (&seen) |*s| {
        _ = runtime.dispatch(.{ .key_down = .{ .key = .tab } });
        const id = runtime.tree.focused.?;
        s.* = runtime.tree.get(id).key orelse "-";
        try std.testing.expect(!std.mem.startsWith(u8, s.*, "behind"));
    }
    // Shift+Tab also stays inside
    _ = runtime.dispatch(.{ .key_down = .{ .key = .tab, .modifiers = .{ .shift = true } } });
    try std.testing.expect(!std.mem.startsWith(u8, runtime.tree.get(runtime.tree.focused.?).key orelse "-", "behind"));
}

test "clickable rows and the hotkey recorder" {
    const Rows = struct {
        picked: ?usize = null,
        deleted: usize = 0,
        recording: bool = false,
        hotkey: ?zant.desktop.Shortcut = null,

        fn pick(self: *@This(), i: usize) void {
            self.picked = i;
        }
        fn delete(self: *@This()) void {
            self.deleted += 1;
        }
        fn start(self: *@This()) void {
            self.recording = true;
        }
        fn record(self: *@This(), s: zant.desktop.Shortcut) void {
            self.hotkey = s;
            self.recording = false;
        }
        fn render(ctx: *anyopaque) ui.Node {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            var rows: [2]ui.Node = undefined;
            for (&rows, 0..) |*row, i| row.* = ui.clickable(.{
                .box = .{ .key = if (i == 0) "row-0" else "row-1", .height = 40, .flex_direction = .row, .children = ui.frameAllocator().dupe(ui.Node, &.{
                    ui.row(.{ .grow = 1 }),
                    ui.iconButton(.{ .key = if (i == 0) "del-0" else "del-1", .icon = .trash, .on_click = ui.action(self, delete) }),
                }) catch unreachable },
                .index_action = ui.indexAction(self, pick),
                .index = i,
            });
            return ui.column(.{ .align_items = .stretch, .children = ui.frameAllocator().dupe(ui.Node, &.{
                rows[0],
                rows[1],
                ui.hotkeyInput(.{ .key = "hk", .value = self.hotkey, .recording = self.recording, .on_start = ui.action(self, start), .on_record = ui.hotkeyAction(self, record) }),
            }) catch unreachable });
        }
    };
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var state = Rows{};
    const runtime = try zant.ui_runtime.Runtime.init(std.testing.allocator, &state, Rows.render, .{ .window = .{ .width = 300, .height = 200 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(300, 200);
    defer renderer.deinit();
    const frame = struct {
        fn f(r: *zant.ui_runtime.Runtime, ren: *zant.renderer.Renderer) void {
            const canvas = ren.begin(1.0).?;
            r.renderFrame(canvas, 300, 200, 1.0);
            ren.end();
        }
    }.f;
    frame(runtime, &renderer);
    const row1 = runtime.tree.boundsOfKey("row-1").?;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = row1.x + 10, .y = row1.y + 10 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = row1.x + 10, .y = row1.y + 10 } });
    try std.testing.expectEqual(@as(?usize, 1), state.picked);
    // a button inside the row takes its own click
    frame(runtime, &renderer);
    const del = runtime.tree.boundsOfKey("del-0").?;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = del.x + 5, .y = del.y + 5 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = del.x + 5, .y = del.y + 5 } });
    try std.testing.expectEqual(@as(usize, 1), state.deleted);
    try std.testing.expectEqual(@as(?usize, 1), state.picked);
    // click the recorder, press ⌃⌥K
    frame(runtime, &renderer);
    const hk = runtime.tree.boundsOfKey("hk").?;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = hk.x + 5, .y = hk.y + 5 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = hk.x + 5, .y = hk.y + 5 } });
    try std.testing.expect(state.recording);
    frame(runtime, &renderer);
    _ = runtime.dispatch(.{ .key_down = .{ .key = .{ .character = 'k' }, .modifiers = .{ .control = true, .alt = true } } });
    try std.testing.expect(!state.recording);
    try std.testing.expect(state.hotkey.?.eql(.{ .key = .{ .character = 'k' }, .modifiers = .{ .control = true, .alt = true } }));
    frame(runtime, &renderer);
}

test "a switch after a growing wrapped-text column stays inside the row" {
    const Row = struct {
        fn render(_: *anyopaque) ui.Node {
            const a = ui.frameAllocator();
            return ui.column(.{ .align_items = .stretch, .children = a.dupe(ui.Node, &.{
                ui.row(.{ .key = "row", .padding = ui.insetXY(16, 12), .gap = 16, .align_items = .center, .children = a.dupe(ui.Node, &.{
                    ui.column(.{ .key = "text-col", .grow = 1, .shrink = 1, .basis = 0, .gap = 3, .align_items = .stretch, .children = a.dupe(ui.Node, &.{
                        ui.text("自动同步", .{ .size = 14 }),
                        ui.text("同一局域网内打开 ClipHop 的设备会互相发现，并同步最新复制的一条文字；历史记录不会同步。", .{ .size = 12, .wrap = true }),
                    }) catch unreachable }),
                    ui.@"switch"(.{ .key = "sw", .checked = true }),
                }) catch unreachable }),
            }) catch unreachable });
        }
    };
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var dummy: u8 = 0;
    const runtime = try zant.ui_runtime.Runtime.init(std.testing.allocator, &dummy, Row.render, .{ .window = .{ .width = 392, .height = 200 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(392, 200);
    defer renderer.deinit();
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 392, 200, 1.0);
    renderer.end();
    const row = runtime.tree.boundsOfKey("row").?;
    const col = runtime.tree.boundsOfKey("text-col").?;
    const sw = runtime.tree.boundsOfKey("sw").?;
    try std.testing.expectApproxEqAbs(@as(f32, 40), sw.width, 0.5);
    try std.testing.expect(sw.x + sw.width <= row.x + row.width - 16 + 0.5);
    // the text column takes the rest of the row
    try std.testing.expectApproxEqAbs(row.width - 32 - 16 - 40, col.width, 0.5);
}

test "image thumbnail and PNG round trip" {
    // 40×20 opaque red, encoded through Skia, thumbnailed to 8 px wide
    var px: [40 * 20 * 4]u8 = undefined;
    for (0..40 * 20) |i| px[i * 4 ..][0..4].* = if (zant.image.native_order == .rgba) .{ 255, 0, 0, 255 } else .{ 0, 0, 255, 255 };
    const img = zant.image.Image.fromPixels(&px, 40, 20, 40 * 4).?;
    defer img.deinit();
    const png_bytes = try img.encodePng(std.testing.allocator);
    defer std.testing.allocator.free(png_bytes);
    try std.testing.expect(std.mem.startsWith(u8, png_bytes, "\x89PNG"));
    const thumb = zant.image.Image.thumbnail(png_bytes, 8).?;
    defer thumb.deinit();
    try std.testing.expectEqual(@as(u32, 8), thumb.width);
    try std.testing.expectEqual(@as(u32, 4), thumb.height);
    // never enlarged
    const same = zant.image.Image.thumbnail(png_bytes, 100).?;
    defer same.deinit();
    try std.testing.expectEqual(@as(u32, 40), same.width);
}
