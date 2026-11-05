const std = @import("std");
const zant = @import("zant");
const App = zant.app.App;
const Renderer = zant.renderer.Renderer;
const events = zant.events;

fn draw(app: *App, renderer: *Renderer, width: u32, height: u32) void {
    const canvas = renderer.begin(1.0) orelse return;
    app.draw(canvas, @floatFromInt(width), @floatFromInt(height), 1.0);
    renderer.end();
}

fn pointFor(node: *const zant.node.Node) events.PointerEvent {
    return .{
        .x = node.bounds.x + node.bounds.width * 0.5,
        .y = node.bounds.y + node.bounds.height * 0.5,
    };
}

fn click(app: *App, id: zant.node.NodeId) void {
    const p = pointFor(app.tree.get(id));
    _ = app.handle(.{ .pointer_move = p });
    _ = app.handle(.{ .pointer_down = p });
    _ = app.handle(.{ .pointer_up = p });
}

fn focusedInput(app: *App) *zant.node.Node {
    const id = app.tree.focused orelse @panic("expected a focused node");
    const node = app.tree.get(id);
    if (std.meta.activeTag(node.data) != .input) @panic("expected focused input");
    return node;
}

test "gallery button click, input editing, composition and focus" {
    const app = try App.create(std.testing.allocator);
    defer app.destroy();
    var renderer = try Renderer.createRaster(900, 640);
    defer renderer.deinit();
    draw(app, &renderer, 900, 640);

    try std.testing.expectEqual(@as(u32, 0), app.gallery.counter);
    click(app, app.gallery.increment_button);
    try std.testing.expectEqual(@as(u32, 1), app.gallery.counter);

    const increment = app.tree.get(app.gallery.increment_button);
    const inc_point = pointFor(increment);
    _ = app.handle(.{ .pointer_down = inc_point });
    _ = app.handle(.{ .pointer_up = .{ .x = 1, .y = 1 } });
    try std.testing.expectEqual(@as(u32, 1), app.gallery.counter);

    click(app, app.gallery.default_input);
    try std.testing.expectEqual(app.gallery.default_input, app.tree.focused.?);
    try std.testing.expect(app.tree.get(app.tree.focused.?).data.input.focused);

    _ = app.handle(.{ .text_input = "!" });
    var input = focusedInput(app);
    try std.testing.expect(std.mem.endsWith(u8, input.data.input.text.items, "Zant UI!"));

    _ = app.handle(.{ .key_down = .{ .key = .backspace } });
    input = focusedInput(app);
    try std.testing.expect(std.mem.endsWith(u8, input.data.input.text.items, "Zant UI"));

    _ = app.handle(.{ .composition = .{
        .text = "zhong",
        .selection_start = 5,
        .selection_end = 5,
    } });
    input = focusedInput(app);
    try std.testing.expect(input.data.input.has_composition);

    _ = app.handle(.{ .text_input = "中" });
    input = focusedInput(app);
    try std.testing.expect(!input.data.input.has_composition);
    try std.testing.expect(std.mem.endsWith(u8, input.data.input.text.items, "中"));

    _ = app.handle(.{ .key_down = .{ .key = .backspace } });
    input = focusedInput(app);
    try std.testing.expect(std.mem.endsWith(u8, input.data.input.text.items, "Zant UI"));

    _ = app.handle(.{ .key_down = .{
        .key = .left,
        .modifiers = .{ .shift = true },
    } });
    input = focusedInput(app);
    try std.testing.expect(input.data.input.cursor < input.data.input.text.items.len);
    try std.testing.expect(input.data.input.cursor != input.data.input.anchor);
    _ = app.handle(.{ .text_input = "X" });
    input = focusedInput(app);
    try std.testing.expect(std.mem.indexOf(u8, input.data.input.text.items, "X") != null);

    _ = app.handle(.{ .key_down = .{ .key = .tab } });
    try std.testing.expect(app.tree.focused == null or app.tree.focused.? != app.gallery.default_input);

    draw(app, &renderer, 760, 560);
    try std.testing.expect(app.tree.get(app.tree.root).bounds.width == 760);

    const png_path = "zig-out/test-gallery.png";
    try std.testing.expect(renderer.writePng(png_path));
}

test "paint-only text update does not mark layout dirty" {
    var tree = try zant.tree.Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const label = try tree.addText(tree.root, "before", .{ .wrap = false });
    tree.layout(.{ .width = 200, .height = 100 });
    tree.clearPaint();
    try tree.setTextNoLayout(label, "after");
    try std.testing.expect(!tree.needsLayout());
    try std.testing.expect(tree.needsPaint());
}

test "caret keeps the text height with a tall line height" {
    var tree = try zant.tree.Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const id = try tree.addInput(tree.root, "**写作应该是轻盈的。**\nsecond line", "", null);
    const node = tree.get(id);
    node.style.width = 400;
    node.style.height = 200;
    node.data.input.multiline = true;
    node.data.input.font_size = 16;
    node.data.input.line_height = 2.0;
    node.data.input.available_width = 380;
    node.data.input.invalidateLayout();
    node.data.input.cursor = 0;
    node.data.input.anchor = 0;
    tree.layout(.{ .width = 500, .height = 300 });
    const caret = zant.input.caretRect(&node.data.input, node).?;
    try std.testing.expect(caret.height <= 16 * 1.3 + 0.01);
    try std.testing.expect(caret.height >= 16);
    // still inside the first line box (8px top inset, 32px line)
    try std.testing.expect(caret.y >= node.bounds.y + 8 - 0.5);
    try std.testing.expect(caret.y + caret.height <= node.bounds.y + 8 + 32 + 0.5);
}

fn multilineInput(tree: *zant.tree.Tree, value: []const u8, height: f32) !*zant.node.Node {
    const id = try tree.addInput(tree.root, value, "", null);
    const node = tree.get(id);
    node.style.width = 300;
    node.style.height = height;
    node.data.input.multiline = true;
    node.data.input.font_size = 16;
    node.data.input.available_width = 278;
    node.data.input.invalidateLayout();
    tree.layout(.{ .width = 400, .height = 400 });
    return node;
}

test "multiline caret moves between lines and to line edges" {
    var tree = try zant.tree.Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const node = try multilineInput(&tree, "first line\nsecond line\nthird line", 200);
    const data = &node.data.input;
    // caret in the middle of "second line"
    data.cursor = 11 + 3;
    data.anchor = data.cursor;
    tree.inputMove(node.id, .up, false);
    try std.testing.expect(data.cursor < 11);
    tree.inputMove(node.id, .down, false);
    try std.testing.expect(data.cursor >= 11 and data.cursor <= 22);
    try std.testing.expect(@abs(@as(i64, @intCast(data.cursor)) - 14) <= 1);
    tree.inputMove(node.id, .line_end, false);
    try std.testing.expectEqual(@as(usize, 22), data.cursor);
    tree.inputMove(node.id, .line_start, false);
    try std.testing.expectEqual(@as(usize, 11), data.cursor);
    tree.inputMove(node.id, .doc_end, true);
    try std.testing.expectEqual(data.text.items.len, data.cursor);
    try std.testing.expectEqual(@as(usize, 11), data.anchor);
}

test "caret moves reveal the caret, plain scrolling does not snap back" {
    var tree = try zant.tree.Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    var long: [400]u8 = undefined;
    for (&long, 0..) |*c, i| c.* = if (i % 10 == 9) '\n' else 'a';
    const node = try multilineInput(&tree, &long, 80);
    const data = &node.data.input;
    tree.inputMove(node.id, .doc_end, false);
    zant.input.ensureCaretVisible(data, data.available_width, 64);
    try std.testing.expect(data.scroll_y > 0);
    // the user scrolls back to the top: no caret move, so it must stay there
    data.scroll_y = 0;
    zant.input.ensureCaretVisible(data, data.available_width, 64);
    try std.testing.expectEqual(@as(f32, 0), data.scroll_y);
}
