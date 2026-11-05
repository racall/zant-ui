const std = @import("std");
const zant = @import("zant");
const Tree = zant.tree.Tree;
const Node = zant.node.Node;
const NodeId = zant.node.NodeId;
const FlexStyle = zant.node.FlexStyle;
const Size = zant.geometry.Size;
const Edges = zant.geometry.Edges;

fn box(tree: *Tree, parent: NodeId, style: FlexStyle) !NodeId {
    return tree.addContainer(parent, style);
}

fn rootRow(tree: *Tree, size: Size) void {
    const root = tree.get(tree.root);
    root.style.direction = .row;
    root.style.align_items = .start;
    tree.layout(size);
}

fn expectRect(node: *Node, x: f32, y: f32, w: f32, h: f32) !void {
    try std.testing.expectApproxEqAbs(x, node.bounds.x, 0.01);
    try std.testing.expectApproxEqAbs(y, node.bounds.y, 0.01);
    try std.testing.expectApproxEqAbs(w, node.bounds.width, 0.01);
    try std.testing.expectApproxEqAbs(h, node.bounds.height, 0.01);
}

test "horizontal layout with fixed widths and gap" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    _ = try box(&tree, tree.root, .{ .width = 100, .height = 50 });
    _ = try box(&tree, tree.root, .{ .width = 80, .height = 50 });
    _ = try box(&tree, tree.root, .{ .width = 60, .height = 50 });
    rootRow(&tree, .{ .width = 500, .height = 200 });
    try expectRect(tree.get(1), 0, 0, 100, 50);
    try expectRect(tree.get(2), 100, 0, 80, 50);
    try expectRect(tree.get(3), 180, 0, 60, 50);
}

test "column layout and auto height" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .column;
    _ = try box(&tree, tree.root, .{ .width = 100, .height = 40 });
    _ = try box(&tree, tree.root, .{ .width = 100, .height = 60 });
    tree.layout(.{ .width = 300, .height = 200 });
    try expectRect(tree.get(1), 0, 0, 100, 40);
    try expectRect(tree.get(2), 0, 40, 100, 60);
}

test "padding and margin" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .column;
    root.style.padding = .{ .left = 20, .top = 10, .right = 20, .bottom = 10 };
    const row = try tree.addContainer(tree.root, .{
        .direction = .row,
        .padding = .all(10),
    });
    _ = try box(&tree, row, .{
        .width = 50,
        .height = 20,
        .margin = .{ .left = 5, .right = 7, .top = 3, .bottom = 4 },
    });
    _ = try box(&tree, row, .{ .width = 40, .height = 20 });
    tree.layout(.{ .width = 300, .height = 200 });
    try expectRect(tree.get(row), 20, 10, 122, 47);
    try expectRect(tree.get(2), 35, 23, 50, 20);
    try expectRect(tree.get(3), 87, 20, 40, 20);
}

test "gap between row items" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    root.style.gap = 12;
    _ = try box(&tree, tree.root, .{ .width = 30, .height = 20 });
    _ = try box(&tree, tree.root, .{ .width = 30, .height = 20 });
    _ = try box(&tree, tree.root, .{ .width = 30, .height = 20 });
    tree.layout(.{ .width = 300, .height = 100 });
    try expectRect(tree.get(2), 42, 0, 30, 20);
    try expectRect(tree.get(3), 84, 0, 30, 20);
}

test "grow distributes free space" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    root.style.padding = .all(10);
    _ = try box(&tree, tree.root, .{ .width = 100, .height = 30 });
    _ = try box(&tree, tree.root, .{ .basis = 100, .grow = 1, .height = 30 });
    tree.layout(.{ .width = 400, .height = 100 });
    try expectRect(tree.get(1), 10, 10, 100, 30);
    try expectRect(tree.get(2), 110, 10, 280, 30);
}

test "shrink reduces overflowing items" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    _ = try box(&tree, tree.root, .{ .width = 200, .height = 30 });
    _ = try box(&tree, tree.root, .{ .width = 100, .height = 30 });
    tree.layout(.{ .width = 200, .height = 100 });
    try std.testing.expect(tree.get(1).bounds.width < 200);
    try std.testing.expect(tree.get(2).bounds.width < 100);
    try std.testing.expectApproxEqAbs(@as(f32, 200), tree.get(1).bounds.width + tree.get(2).bounds.width, 0.1);
}

test "min and max size constraints" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    _ = try box(&tree, tree.root, .{ .basis = 50, .grow = 1, .max_width = 80, .height = 20 });
    _ = try box(&tree, tree.root, .{ .basis = 50, .grow = 1, .min_width = 90, .height = 20 });
    tree.layout(.{ .width = 400, .height = 100 });
    try std.testing.expectApproxEqAbs(@as(f32, 80), tree.get(1).bounds.width, 0.1);
    try std.testing.expect(tree.get(2).bounds.width >= 90);
}

test "nested flex and window resize" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .column;
    root.style.padding = .all(10);
    const row = try tree.addContainer(tree.root, .{ .direction = .row, .gap = 10 });
    _ = try box(&tree, row, .{ .width = 100, .height = 40 });
    _ = try box(&tree, row, .{ .width = 100, .height = 40 });
    tree.layout(.{ .width = 400, .height = 200 });
    try expectRect(tree.get(row), 10, 10, 210, 40);

    tree.layout(.{ .width = 300, .height = 200 });
    try expectRect(tree.get(row), 10, 10, 210, 40);
    try expectRect(tree.get(2), 10, 10, 100, 40);
    try expectRect(tree.get(3), 120, 10, 100, 40);
}

test "justify space-between" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    root.style.justify = .space_between;
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20 });
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20 });
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20 });
    tree.layout(.{ .width = 250, .height = 100 });
    try expectRect(tree.get(1), 0, 0, 50, 20);
    try expectRect(tree.get(2), 100, 0, 50, 20);
    try expectRect(tree.get(3), 200, 0, 50, 20);
}

test "align center and end" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    root.style.align_items = .center;
    root.style.height = 100;
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20 });
    tree.layout(.{ .width = 200, .height = 100 });
    try std.testing.expectApproxEqAbs(@as(f32, 40), tree.get(1).bounds.y, 0.1);

    root.style.align_items = .end;
    tree.layout(.{ .width = 200, .height = 100 });
    try std.testing.expectApproxEqAbs(@as(f32, 80), tree.get(1).bounds.y, 0.1);
}

test "row wrap creates multiple lines" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    root.style.wrap = .wrap;
    _ = try box(&tree, tree.root, .{ .width = 60, .height = 20 });
    _ = try box(&tree, tree.root, .{ .width = 60, .height = 20 });
    _ = try box(&tree, tree.root, .{ .width = 30, .height = 20 });
    tree.layout(.{ .width = 100, .height = 200 });
    try expectRect(tree.get(1), 0, 0, 60, 20);
    try expectRect(tree.get(2), 0, 20, 60, 20);
    try expectRect(tree.get(3), 60, 20, 30, 20);
}

test "percentage width resolves against parent" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    _ = try box(&tree, tree.root, .{ .width_percent = 50, .height = 20 });
    tree.layout(.{ .width = 200, .height = 100 });
    try expectRect(tree.get(1), 0, 0, 100, 20);
}

test "order reorders flex items without changing tree ids" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20, .order = 2 });
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20, .order = 1 });
    tree.layout(.{ .width = 200, .height = 100 });
    try expectRect(tree.get(2), 0, 0, 50, 20);
    try expectRect(tree.get(1), 50, 0, 50, 20);
}

test "align-self centers one child" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .row;
    _ = try box(&tree, tree.root, .{ .width = 20, .height = 20, .align_self = .center });
    tree.layout(.{ .width = 200, .height = 100 });
    try expectRect(tree.get(1), 0, 40, 20, 20);
}

test "absolute child is positioned without taking flow space" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const root = tree.get(tree.root);
    root.style.direction = .column;
    _ = try box(&tree, tree.root, .{ .width = 50, .height = 20 });
    const absolute = try tree.addContainer(tree.root, .{
        .absolute = true,
        .right = 20,
        .top = 10,
        .width = 100,
        .height = 40,
    });
    tree.layout(.{ .width = 200, .height = 100 });
    try expectRect(tree.get(1), 0, 0, 50, 20);
    try expectRect(tree.get(absolute), 80, 10, 100, 40);
}

test "rich text lays out and wraps" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const spans = [_]zant.node.RichSpan{
        .{ .text = "Hello ", .font_size = 15, .color = 0xff292d2a },
        .{ .text = "world this is a long richer text run", .font_size = 15, .weight = .semibold, .color = 0xff607d6b },
    };
    const id = try tree.addRichText(tree.root, &spans);
    tree.get(id).style.width_percent = 100;
    tree.layout(.{ .width = 120, .height = 200 });
    try std.testing.expect(tree.get(id).bounds.height >= 20);
    try std.testing.expect(tree.get(id).bounds.width <= 120.1);
}

test "absolute button participates in hit testing" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const id = try tree.addButton(tree.root, "menu", .text, .middle, null);
    const node = tree.get(id);
    node.style.absolute = true;
    node.style.top = 10;
    node.style.right = 10;
    node.style.width = 80;
    node.style.height = 30;
    tree.layout(.{ .width = 200, .height = 100 });
    try expectRect(node, 110, 10, 80, 30);
    try std.testing.expectEqual(@as(?zant.node.NodeId, id), tree.hitTest(140, 20));
}

test "absolute child resolves percentage height (full-size overlays)" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const overlay = try box(&tree, tree.root, .{ .absolute = true, .left = 0, .top = 0, .width_percent = 100, .height_percent = 100 });
    rootRow(&tree, .{ .width = 400, .height = 300 });
    try expectRect(tree.get(overlay), 0, 0, 400, 300);
}

test "stretch fills an auto-height row and absolute children do not size parents" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const row = try box(&tree, tree.root, .{ .direction = .row, .align_items = .stretch });
    const bar = try box(&tree, row, .{ .width = 3 });
    _ = try box(&tree, row, .{ .width = 50, .height = 40 });
    const pill = try box(&tree, tree.root, .{ .direction = .row });
    _ = try box(&tree, pill, .{ .absolute = true, .width_percent = 100, .height = 10 });
    _ = try box(&tree, pill, .{ .width = 30, .height = 10 });
    const root = tree.get(tree.root);
    root.style.direction = .column;
    root.style.align_items = .start;
    tree.layout(.{ .width = 500, .height = 300 });
    try std.testing.expectApproxEqAbs(@as(f32, 40), tree.get(bar).bounds.height, 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 30), tree.get(pill).bounds.width, 0.01);
}

test "absolute percentage height wins over a builder's fixed height" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const item = try box(&tree, tree.root, .{ .direction = .row, .height = 24, .width = 80 });
    // like ui.styled(ui.button(..) /* height 32 */, .{ .height_percent = 100 })
    const hit = try box(&tree, item, .{ .absolute = true, .left = 0, .top = 0, .width_percent = 100, .height = 32, .height_percent = 100 });
    rootRow(&tree, .{ .width = 400, .height = 300 });
    try expectRect(tree.get(hit), 0, 0, 80, 24);
}

test "absolute children use the padding box (CSS containing block)" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const item = try box(&tree, tree.root, .{ .direction = .row, .height = 24, .padding = .{ .left = 11, .right = 11 } });
    const hit = try box(&tree, item, .{ .absolute = true, .left = 0, .top = 0, .width_percent = 100, .height_percent = 100 });
    _ = try box(&tree, item, .{ .width = 40, .height = 24 });
    rootRow(&tree, .{ .width = 400, .height = 300 });
    try expectRect(tree.get(item), 0, 0, 62, 24);
    try expectRect(tree.get(hit), 0, 0, 62, 24);
}

test "anchored absolute child takes the target sibling's box" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const track = try box(&tree, tree.root, .{ .direction = .row, .padding = Edges.all(2), .gap = 2, .align_items = .center });
    const thumb = try box(&tree, track, .{ .absolute = true, .anchor = 1 });
    const ink = try box(&tree, track, .{ .absolute = true, .anchor = 2, .bottom = 0, .height = 2 });
    _ = try box(&tree, track, .{ .width = 40, .height = 24 });
    const second = try box(&tree, track, .{ .width = 60, .height = 24 });
    const third = try box(&tree, track, .{ .width = 30, .height = 24 });
    rootRow(&tree, .{ .width = 500, .height = 200 });
    const target = tree.get(second).bounds;
    try expectRect(tree.get(thumb), target.x, target.y, target.width, target.height);
    // own vertical placement keeps bottom / height
    const t3 = tree.get(third).bounds;
    const tb = tree.get(track).bounds;
    try expectRect(tree.get(ink), t3.x, tb.y + tb.height - 2, t3.width, 2);
}

test "single-line text with max_lines shrinks and ends in an ellipsis" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const row = try box(&tree, tree.root, .{ .direction = .row, .width = 120 });
    const label = try tree.addText(row, "A very long document name that cannot fit.md", .{ .font_size = 13, .wrap = false, .max_lines = 1 });
    tree.get(label).style.min_width = 0;
    rootRow(&tree, .{ .width = 500, .height = 200 });
    const node = tree.get(label);
    try std.testing.expect(node.bounds.width <= 120.5);
    const layout = try node.data.text.ensureLayout(node.data.text.layoutWidth(node.bounds.width));
    try std.testing.expect(layout.metrics.longest_line <= 121);
    try std.testing.expectApproxEqAbs(@as(f32, 13 * 1.2), node.bounds.height, 6);
}

test "absolute child with left and right stretches between the insets" {
    var tree = try Tree.init(std.testing.allocator, zant.tokens.antLight());
    defer tree.deinit();
    const bar = try box(&tree, tree.root, .{ .width = 400, .height = 50 });
    const title = try box(&tree, bar, .{ .absolute = true, .left = 120, .right = 120, .top = 0, .height = 50 });
    rootRow(&tree, .{ .width = 500, .height = 200 });
    try expectRect(tree.get(title), 120, 0, 160, 50);
}
