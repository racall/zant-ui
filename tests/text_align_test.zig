//! Centred / right-aligned text that does not wrap stays inside its box
//! (it used to be centred within a 1e9px layout width, i.e. off screen).
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;

const Scene = struct {
    fn render(_: *anyopaque) ui.Node {
        return ui.column(.{ .padding = ui.inset(16), .gap = 8, .children = &.{
            ui.row(.{ .gap = 8, .children = &.{
                ui.avatar(.{ .key = "avatar", .text = "Z", .size = 32 }),
                ui.avatar(.{ .key = "avatar-solid", .text = "UI", .size = 40, .background = 0xff1677ff }),
            } }),
            ui.styled(ui.text("Right", .{ .key = "right", .size = 16, .text_align = .right, .color = 0xff000000 }), .{ .width = 160 }),
        } });
    }
};

fn find(tree: *zant.tree.Tree, key: []const u8) *zant.node.Node {
    for (tree.nodes.items) |node| {
        if (node.alive) if (node.key) |k| if (std.mem.eql(u8, k, key)) return node;
    }
    unreachable;
}

/// Lowest red channel inside a rect: text pixels are darker than the fills.
fn minRed(renderer: *const Renderer, b: zant.geometry.Rect) u32 {
    var min: u32 = 255;
    var y: usize = @intFromFloat(b.y);
    while (y < @as(usize, @intFromFloat(b.y + b.height))) : (y += 1) {
        var x: usize = @intFromFloat(b.x);
        while (x < @as(usize, @intFromFloat(b.x + b.width))) : (x += 1) {
            if (renderer.pixelAt(x, y)) |px| min = @min(min, (px >> 16) & 0xff);
        }
    }
    return min;
}

/// Highest red channel inside a rect (white text on a dark fill).
fn maxRed(renderer: *const Renderer, b: zant.geometry.Rect) u32 {
    var max: u32 = 0;
    var y: usize = @intFromFloat(b.y + b.height * 0.25);
    while (y < @as(usize, @intFromFloat(b.y + b.height * 0.75))) : (y += 1) {
        var x: usize = @intFromFloat(b.x + b.width * 0.25);
        while (x < @as(usize, @intFromFloat(b.x + b.width * 0.75))) : (x += 1) {
            if (renderer.pixelAt(x, y)) |px| max = @max(max, (px >> 16) & 0xff);
        }
    }
    return max;
}

test "unwrapped centred and right-aligned text is painted inside its box" {
    var dummy: u8 = 0;
    const runtime = try Runtime.init(std.testing.allocator, &dummy, Scene.render, .{ .window = .{ .width = 240, .height = 120 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(240, 120);
    defer renderer.deinit();
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 240, 120, 1.0);
    renderer.end();

    // primary (0x16) "Z" on the light primary tint (0xe6)
    try std.testing.expect(minRed(&renderer, find(runtime.tree, "avatar").bounds) < 0x80);
    // white "UI" on the primary fill (red 0x16)
    try std.testing.expect(maxRed(&renderer, find(runtime.tree, "avatar-solid").bounds) > 0xc0);
    // right-aligned black text inside its 160px box
    try std.testing.expect(minRed(&renderer, find(runtime.tree, "right").bounds) < 0x40);
}
