//! Text selection across the text nodes of a `selectable` container:
//! document-order collection, hit testing (nearest text when the pointer
//! is between nodes), word expansion and the copied text.
const std = @import("std");
const node_mod = @import("node.zig");
const tree_mod = @import("tree.zig");
const unicode = @import("unicode.zig");

const Tree = tree_mod.Tree;
const Node = node_mod.Node;
const NodeId = node_mod.NodeId;

pub const Point = struct { node: NodeId, byte: usize };

pub fn isText(node: *const Node) bool {
    return switch (node.data) {
        .text, .rich_text => true,
        .vector => |*d| d.copy.len > 0,
        else => false,
    };
}

/// Text and rich text nodes of `root`'s subtree, in document order.
pub fn collect(tree: *Tree, root: NodeId, allocator: std.mem.Allocator, out: *std.ArrayListUnmanaged(NodeId)) !void {
    const node = tree.get(root);
    if (!node.alive or !node.inFlow() or node.visual.unselectable) return;
    if (isText(node)) try out.append(allocator, root);
    for (node.children.items) |child| try collect(tree, child, allocator, out);
}

/// Innermost `selectable` container under (x, y).
pub fn rootAt(tree: *Tree, x: f32, y: f32) ?NodeId {
    return rootAtNode(tree, tree.get(tree.root), x, y);
}

fn rootAtNode(tree: *Tree, node: *Node, x: f32, y: f32) ?NodeId {
    if (!node.inFlow() or !node.bounds.contains(x, y)) return null;
    var i = node.children.items.len;
    while (i > 0) {
        i -= 1;
        if (rootAtNode(tree, tree.get(node.children.items[i]), x, y)) |hit| return hit;
    }
    return if (node.visual.selectable) node.id else null;
}

/// The node's text as laid out (rich text: spans joined).
pub fn nodeText(node: *const Node, allocator: std.mem.Allocator) ![]u8 {
    return switch (node.data) {
        .text => |*d| allocator.dupe(u8, d.content),
        .rich_text => |*d| blk: {
            var out: std.ArrayListUnmanaged(u8) = .empty;
            for (d.spans) |span| try out.appendSlice(allocator, span.text);
            break :blk out.toOwnedSlice(allocator);
        },
        .vector => |*d| allocator.dupe(u8, d.copy),
        else => allocator.dupe(u8, ""),
    };
}

fn byteAt(node: *Node, x: f32, y: f32) usize {
    const lx = x - node.bounds.x;
    const ly = y - node.bounds.y;
    return switch (node.data) {
        .text => |*d| if (d.ensureLayout(d.layoutWidth(node.bounds.width))) |l| @min(l.indexAt(lx, ly), d.content.len) else |_| 0,
        .rich_text => |*d| if (d.ensureLayout(node.bounds.width)) |l| l.indexAt(lx, ly) else |_| 0,
        // a drawing is one unit: before or after it
        .vector => |*d| if (lx < node.bounds.width / 2) 0 else d.copy.len,
        else => 0,
    };
}

/// Text position under (x, y): the node containing the point, else the
/// nearest node (lines first, then columns), clamped into it.
pub fn pointAt(tree: *Tree, nodes: []const NodeId, x: f32, y: f32) ?Point {
    var best: ?NodeId = null;
    var best_score = std.math.inf(f32);
    for (nodes) |id| {
        const b = tree.get(id).bounds;
        const dy = if (y < b.y) b.y - y else if (y > b.bottom()) y - b.bottom() else 0;
        const dx = if (x < b.x) b.x - x else if (x > b.right()) x - b.right() else 0;
        const score = dy * 4 + dx;
        if (score < best_score) {
            best_score = score;
            best = id;
        }
    }
    const id = best orelse return null;
    const node = tree.get(id);
    const b = node.bounds;
    // above / below the node: its start / end
    if (y < b.y) return .{ .node = id, .byte = 0 };
    if (y > b.bottom()) return .{ .node = id, .byte = textLen(node) };
    return .{ .node = id, .byte = byteAt(node, std.math.clamp(x, b.x, b.right()), y) };
}

pub fn textLen(node: *const Node) usize {
    return switch (node.data) {
        .text => |*d| d.content.len,
        .rich_text => |*d| blk: {
            var n: usize = 0;
            for (d.spans) |span| n += span.text.len;
            break :blk n;
        },
        .vector => |*d| d.copy.len,
        else => 0,
    };
}

pub fn indexOf(nodes: []const NodeId, id: NodeId) ?usize {
    return std.mem.indexOfScalar(NodeId, nodes, id);
}

/// Word around `byte` in a node (double click).
pub fn wordAt(node: *const Node, byte: usize, allocator: std.mem.Allocator) [2]usize {
    const value = nodeText(node, allocator) catch return .{ byte, byte };
    defer allocator.free(value);
    const at = @min(byte, value.len);
    const start = unicode.wordStart(value, at);
    var end = unicode.wordEnd(value, at);
    if (start == end and end < value.len) end = unicode.nextCodepoint(value, end);
    return .{ start, end };
}

/// Ordered selection (start, end) in node-list order.
pub fn ordered(nodes: []const NodeId, a: Point, b: Point) ?struct { start: Point, end: Point, si: usize, ei: usize } {
    const ai = indexOf(nodes, a.node) orelse return null;
    const bi = indexOf(nodes, b.node) orelse return null;
    if (ai < bi or (ai == bi and a.byte <= b.byte)) return .{ .start = a, .end = b, .si = ai, .ei = bi };
    return .{ .start = b, .end = a, .si = bi, .ei = ai };
}

/// Copied text of a node range: spans' `copy` text replaces their text
/// (an embed is copied whole when any of it is selected).
fn appendRange(node: *const Node, from: usize, to: usize, allocator: std.mem.Allocator, out: *std.ArrayListUnmanaged(u8)) !void {
    switch (node.data) {
        .text => |*d| {
            const s = @min(from, d.content.len);
            const e = @min(to, d.content.len);
            if (s < e) try out.appendSlice(allocator, d.content[s..e]);
        },
        .rich_text => |*d| {
            var offset: usize = 0;
            for (d.spans) |span| {
                const s0 = offset;
                const e0 = offset + span.text.len;
                offset = e0;
                const s = @max(s0, from);
                const e = @min(e0, to);
                if (s >= e) continue;
                if (span.copy) |c| {
                    try out.appendSlice(allocator, c);
                } else try out.appendSlice(allocator, span.text[s - s0 .. e - s0]);
            }
        },
        .vector => |*d| if (from < to) try out.appendSlice(allocator, d.copy),
        else => {},
    }
}

/// The selected text: nodes joined by a line break when the next starts
/// below the previous one, by a space when they sit side by side.
pub fn copyText(tree: *Tree, nodes: []const NodeId, a: Point, b: Point, allocator: std.mem.Allocator) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    errdefer out.deinit(allocator);
    const range = ordered(nodes, a, b) orelse return out.toOwnedSlice(allocator);
    var prev: ?*Node = null;
    var i = range.si;
    while (i <= range.ei) : (i += 1) {
        const node = tree.get(nodes[i]);
        const from = if (i == range.si) range.start.byte else 0;
        const to = if (i == range.ei) range.end.byte else textLen(node);
        if (prev) |p| {
            if (node.bounds.y >= p.bounds.bottom() - 2) {
                try out.append(allocator, '\n');
            } else if (out.items.len > 0 and out.items[out.items.len - 1] != ' ') try out.append(allocator, ' ');
        }
        try appendRange(node, from, to, allocator, &out);
        prev = node;
    }
    return out.toOwnedSlice(allocator);
}
