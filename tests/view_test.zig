//! ui.view: RN `View` / HTML `div` defaults, BoxOptions passthrough,
//! shadow, style override, layout and clicks.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;

const expectEqual = std.testing.expectEqual;

fn renderFrame(runtime: *Runtime, renderer: *Renderer, width: f32, height: f32) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, width, height, 1.0);
    renderer.end();
}

fn clickAt(runtime: *Runtime, rect: zant.geometry.Rect) void {
    const p = zant.events.PointerEvent{ .x = rect.x + rect.width * 0.5, .y = rect.y + rect.height * 0.5 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
}

test "view defaults to a stretching, unshrinking column container" {
    ui.beginFrame();
    defer ui.endFrame();
    const node = ui.view(.{});
    const e = node.element;
    try expectEqual(zant.node.NodeKind.container, e.kind);
    try expectEqual(zant.flex.Direction.column, e.layout.direction);
    try expectEqual(zant.flex.Align.stretch, e.layout.align_items);
    try expectEqual(@as(f32, 0), e.layout.shrink);
    try expectEqual(@as(?zant.node.PointerArea, null), e.pointer);
    try expectEqual(@as(?u32, null), e.visual.shadow_color);
}

test "view direction and the flex_direction alias" {
    ui.beginFrame();
    defer ui.endFrame();
    try expectEqual(zant.flex.Direction.row, ui.view(.{ .direction = .row }).element.layout.direction);
    try expectEqual(zant.flex.Direction.row, ui.view(.{ .flex_direction = .row }).element.layout.direction);
    try expectEqual(
        zant.flex.Direction.column,
        ui.view(.{ .direction = .row, .flex_direction = .column }).element.layout.direction,
    );
}

test "view passes every box option through" {
    ui.beginFrame();
    defer ui.endFrame();
    const child = ui.text("x", .{});
    const node = ui.view(.{
        .key = "card",
        .gap = 6,
        .padding = ui.inset(12),
        .margin = ui.insetXY(4, 2),
        .justify_content = .center,
        .align_items = .center,
        .width = 120,
        .height_percent = 50,
        .grow = 1,
        .shrink = 1,
        .background = 0xff112233,
        .border_color = 0xff445566,
        .border_width = 1,
        .radius = 8,
        .scroll = true,
        .hover_background = 0x0f000000,
        .children = &.{child},
    });
    const e = node.element;
    try std.testing.expectEqualStrings("card", e.key.?);
    try expectEqual(@as(f32, 6), e.layout.gap);
    try expectEqual(@as(f32, 12), e.layout.padding.top);
    try expectEqual(@as(f32, 4), e.layout.margin.left);
    try expectEqual(@as(f32, 2), e.layout.margin.top);
    try expectEqual(zant.flex.Justify.center, e.layout.justify);
    try expectEqual(zant.flex.Align.center, e.layout.align_items);
    try expectEqual(@as(?f32, 120), e.layout.width);
    try expectEqual(@as(?f32, 50), e.layout.height_percent);
    try expectEqual(@as(f32, 1), e.layout.grow);
    try expectEqual(@as(f32, 1), e.layout.shrink);
    try expectEqual(@as(?u32, 0xff112233), e.visual.background);
    try expectEqual(@as(?u32, 0xff445566), e.visual.border_color);
    try expectEqual(@as(f32, 1), e.visual.border_width);
    try expectEqual(@as(f32, 8), e.visual.radius);
    try std.testing.expect(e.layout.scroll);
    try std.testing.expect(e.visual.clip); // scrolling clips
    try expectEqual(@as(?u32, 0x0f000000), e.hover_visual.?.background);
    try expectEqual(@as(usize, 1), e.children.len);
}

test "view shadow and style override" {
    ui.beginFrame();
    defer ui.endFrame();
    const shadowed = ui.view(.{ .shadow_color = 0x33000000, .shadow_blur = 12, .shadow_offset_y = 4 });
    try expectEqual(@as(?u32, 0x33000000), shadowed.element.visual.shadow_color);
    try expectEqual(@as(f32, 12), shadowed.element.visual.shadow_blur);
    try expectEqual(@as(f32, 4), shadowed.element.visual.shadow_offset_y);

    // style wins over the plain options, including the shadow
    const node = ui.view(.{
        .background = 0xff000000,
        .width = 10,
        .shadow_color = 0x33000000,
        .shadow_blur = 12,
        .style = .{ .background = 0xffffffff, .width = 40, .shadow_blur = 2, .radius = 6 },
    });
    try expectEqual(@as(?u32, 0xffffffff), node.element.visual.background);
    try expectEqual(@as(?f32, 40), node.element.layout.width);
    try expectEqual(@as(f32, 6), node.element.visual.radius);
    try expectEqual(@as(?u32, 0x33000000), node.element.visual.shadow_color);
    try expectEqual(@as(f32, 2), node.element.visual.shadow_blur);
}

test "view lays children out like a div and a row view side by side" {
    const Layout = struct {
        fn render(_: *anyopaque) ui.Node {
            const a = ui.frameAllocator();
            return ui.view(.{ .key = "root", .padding = ui.inset(10), .gap = 8, .children = a.dupe(ui.Node, &.{
                ui.view(.{ .key = "block", .height = 30 }),
                ui.view(.{ .key = "bar", .direction = .row, .gap = 4, .children = a.dupe(ui.Node, &.{
                    ui.view(.{ .key = "a", .width = 50, .height = 20 }),
                    ui.view(.{ .key = "b", .grow = 1, .height = 20 }),
                }) catch unreachable }),
            }) catch unreachable });
        }
    };
    var state: u8 = 0;
    const runtime = try Runtime.init(std.testing.allocator, &state, Layout.render, .{ .window = .{ .width = 300, .height = 200 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(300, 200);
    defer renderer.deinit();
    renderFrame(runtime, &renderer, 300, 200);

    const block = runtime.tree.boundsOfKey("block").?;
    try expectEqual(@as(f32, 10), block.x);
    try expectEqual(@as(f32, 10), block.y);
    try expectEqual(@as(f32, 280), block.width); // stretched across the padding box
    try expectEqual(@as(f32, 30), block.height);

    const bar = runtime.tree.boundsOfKey("bar").?;
    try expectEqual(@as(f32, 48), bar.y);
    try expectEqual(@as(f32, 280), bar.width);
    try expectEqual(@as(f32, 20), bar.height);

    const a = runtime.tree.boundsOfKey("a").?;
    const b = runtime.tree.boundsOfKey("b").?;
    try expectEqual(@as(f32, 10), a.x);
    try expectEqual(@as(f32, 50), a.width);
    try expectEqual(@as(f32, 64), b.x);
    try expectEqual(@as(f32, 226), b.width); // grows into the rest of the row
}

test "clickable view: on_click, index_action and inner buttons keep their clicks" {
    const Clicks = struct {
        opened: u32 = 0,
        picked: ?usize = null,
        saved: u32 = 0,

        fn open(self: *@This()) void {
            self.opened += 1;
        }
        fn pick(self: *@This(), i: usize) void {
            self.picked = i;
        }
        fn save(self: *@This()) void {
            self.saved += 1;
        }
        fn render(ctx: *anyopaque) ui.Node {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            const a = ui.frameAllocator();
            return ui.view(.{ .children = a.dupe(ui.Node, &.{
                ui.view(.{ .key = "card", .height = 60, .direction = .row, .on_click = ui.action(self, open), .children = a.dupe(ui.Node, &.{
                    ui.view(.{ .grow = 1 }),
                    ui.button(.{ .key = "save", .text = "Save", .on_click = ui.action(self, save) }),
                }) catch unreachable }),
                ui.view(.{ .key = "row-3", .height = 40, .index_action = ui.indexAction(self, pick), .index = 3 }),
            }) catch unreachable });
        }
    };
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var state = Clicks{};
    const runtime = try Runtime.init(std.testing.allocator, &state, Clicks.render, .{ .window = .{ .width = 300, .height = 200 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(300, 200);
    defer renderer.deinit();

    renderFrame(runtime, &renderer, 300, 200);
    const card = runtime.tree.boundsOfKey("card").?;
    clickAt(runtime, .{ .x = card.x, .y = card.y, .width = 40, .height = card.height });
    try expectEqual(@as(u32, 1), state.opened);

    renderFrame(runtime, &renderer, 300, 200);
    clickAt(runtime, runtime.tree.boundsOfKey("save").?);
    try expectEqual(@as(u32, 1), state.saved);
    try expectEqual(@as(u32, 1), state.opened);

    renderFrame(runtime, &renderer, 300, 200);
    clickAt(runtime, runtime.tree.boundsOfKey("row-3").?);
    try expectEqual(@as(?usize, 3), state.picked);
}

test "wrapped text at its natural width stays on one line (start-aligned columns)" {
    const Wrap = struct {
        fn render(_: *anyopaque) ui.Node {
            const a = ui.frameAllocator();
            return ui.view(.{ .padding = ui.inset(24), .gap = 16, .align_items = .start, .children = a.dupe(ui.Node, &.{
                ui.text("Terms", .{ .key = "word", .size = 14, .weight = .medium, .wrap = true }),
                ui.withKey(ui.paragraph("Every component on this page is written in TSX and compiled to native Zig.", .{}), "para"),
                ui.view(.{ .width = 420, .padding = ui.inset(20), .align_items = .start, .children = a.dupe(ui.Node, &.{
                    ui.withKey(ui.alert(.{ .title = "Terms", .description = "State flows from TSX into native Zant components." }), "alert"),
                }) catch unreachable }),
            }) catch unreachable });
        }
    };
    var state: u8 = 0;
    const runtime = try Runtime.init(std.testing.allocator, &state, Wrap.render, .{ .window = .{ .width = 960, .height = 700 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(960, 700);
    defer renderer.deinit();
    renderFrame(runtime, &renderer, 960, 700);
    // one line each: a wrapped "Term / s" or a dropped last word doubles the height
    for (runtime.tree.nodes.items) |n| {
        if (!n.alive or n.kind() != .text) continue;
        const one_line = n.data.text.style.font_size * 1.6;
        if (n.bounds.height > one_line) {
            std.debug.print("text \"{s}\" wrapped: {d:.1}x{d:.1}\n", .{ n.data.text.content, n.bounds.width, n.bounds.height });
            return error.TextWrapped;
        }
    }
}
