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
