const std = @import("std");
const geometry = @import("geometry.zig");
const node_mod = @import("node.zig");
const flex = @import("../layout/flex.zig");
const tokens = @import("../theme/tokens.zig");
const button = @import("../components/button.zig");
const input = @import("../components/input.zig");
const toggle = @import("../components/toggle.zig");
const image_mod = @import("../components/image.zig");
const select_mod = @import("../components/select.zig");

const Node = node_mod.Node;
const NodeId = node_mod.NodeId;
const Theme = tokens.Theme;

fn clampDim(node: *const Node, value: f32, width: bool) f32 {
    if (width) {
        const lo = node.style.min_width orelse 0;
        const hi = node.style.max_width orelse std.math.inf(f32);
        return @max(lo, @min(value, hi));
    }
    const lo = node.style.min_height orelse 0;
    const hi = node.style.max_height orelse std.math.inf(f32);
    return @max(lo, @min(value, hi));
}

fn finite(v: f32, fallback: f32) f32 {
    if (std.math.isFinite(v) and v > 0) return v;
    return fallback;
}

fn visibleChildren(children: []const NodeId, nodes: []*Node, out: *std.ArrayListUnmanaged(NodeId), allocator: std.mem.Allocator) !void {
    out.clearRetainingCapacity();
    for (children) |id| {
        const child = nodes[id];
        if (!child.visible) continue;
        try out.append(allocator, id);
    }
}

pub fn intrinsicWidth(nodes: []*Node, node: *Node, max_avail: f32, theme: Theme) f32 {
    if (node.style.width) |w| return clampDim(node, w, true);
    const avail = finite(max_avail, 1.0e6);
    const value: f32 = switch (node.data) {
        .root, .container => blk: {
            var total: f32 = 0;
            var widest: f32 = 0;
            const direction = node.style.direction;
            var index: usize = 0;
            for (node.children.items) |id| {
                const child = nodes[id];
                if (!child.visible) continue;
                const child_width = intrinsicWidth(nodes, child, avail, theme) +
                    child.style.margin.left + child.style.margin.right;
                if (direction == .row) {
                    total += child_width;
                    if (index > 0) total += node.style.gap;
                } else {
                    widest = @max(widest, child_width);
                }
                index += 1;
            }
            break :blk (if (direction == .row) total else widest) + node.style.padding.horizontal();
        },
        .text => |*data| blk: {
            const max_width = if (data.style.wrap) avail else 1.0e6;
            const layout = data.ensureLayout(max_width) catch break :blk 0;
            break :blk layout.metrics.longest_line;
        },
        .button => button.measureWidth(node, theme, avail),
        .input => input.measureWidth(node),
        .toggle => toggle.measureWidth(node, theme, avail),
        .image => image_mod.measureWidth(node),
        .select => select_mod.measureWidth(node),
    };
    return clampDim(node, value, true);
}

pub fn intrinsicHeight(nodes: []*Node, node: *Node, width: f32, theme: Theme) f32 {
    if (node.style.height) |h| return clampDim(node, h, false);
    const value: f32 = switch (node.data) {
        .root, .container => blk: {
            const inner_width = @max(0, width - node.style.padding.horizontal());
            var total: f32 = 0;
            var tallest: f32 = 0;
            if (node.style.direction == .row) {
                for (node.children.items) |id| {
                    const child = nodes[id];
                    if (!child.visible) continue;
                    const child_width = child.style.width orelse intrinsicWidth(nodes, child, inner_width, theme);
                    const h = intrinsicHeight(nodes, child, child_width, theme) +
                        child.style.margin.top + child.style.margin.bottom;
                    tallest = @max(tallest, h);
                }
                break :blk tallest + node.style.padding.vertical();
            } else {
                var index: usize = 0;
                for (node.children.items) |id| {
                    const child = nodes[id];
                    if (!child.visible) continue;
                    const child_width = if (node.style.align_items == .stretch and child.style.width == null)
                        @max(0, inner_width - child.style.margin.horizontal())
                    else
                        child.style.width orelse intrinsicWidth(nodes, child, inner_width, theme);
                    total += intrinsicHeight(nodes, child, child_width, theme) +
                        child.style.margin.top + child.style.margin.bottom;
                    if (index > 0) total += node.style.gap;
                    index += 1;
                }
                break :blk total + node.style.padding.vertical();
            }
        },
        .text => |*data| blk: {
            const width_for_layout = if (data.style.wrap) width else 1.0e6;
            const layout = data.ensureLayout(width_for_layout) catch break :blk data.style.font_size * 1.2;
            break :blk layout.metrics.height;
        },
        .button => button.measureHeight(node),
        .input => input.measureHeight(node),
        .toggle => toggle.measureHeight(node),
        .image => image_mod.measureHeight(node, width),
        .select => select_mod.measureHeight(node),
    };
    return clampDim(node, value, false);
}

pub fn layoutTree(nodes: []*Node, root_id: NodeId, viewport: geometry.Size, theme: Theme) void {
    _ = layoutNode(nodes, nodes[root_id], 0, 0, viewport.width, viewport.height, viewport.width, viewport.height, theme, true);
    for (nodes) |node| {
        node.dirty.layout = false;
        node.dirty.paint = true;
    }
}

fn layoutNode(
    nodes: []*Node,
    node: *Node,
    x: f32,
    y: f32,
    assigned_width: ?f32,
    assigned_height: ?f32,
    available_width: f32,
    available_height: f32,
    theme: Theme,
    is_root: bool,
) geometry.Size {
    _ = available_height;
    _ = is_root;
    var width = assigned_width orelse node.style.width orelse intrinsicWidth(nodes, node, available_width, theme);
    width = clampDim(node, @max(0, width), true);
    node.bounds.x = x;
    node.bounds.y = y;
    node.bounds.width = width;

    switch (node.data) {
        .text => |*data| {
            var height = assigned_height orelse node.style.height;
            const max_width = if (data.style.wrap) width else 1.0e6;
            if (data.ensureLayout(max_width)) |layout| {
                if (height == null) height = layout.metrics.height;
            } else |_| {
                if (height == null) height = data.style.font_size * 1.2;
            }
            node.bounds.height = clampDim(node, @max(0, height orelse 0), false);
        },
        .button => {
            const height = assigned_height orelse button.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .input => {
            const height = assigned_height orelse input.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .toggle => {
            const height = assigned_height orelse toggle.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .image => {
            const height = assigned_height orelse node.style.height orelse image_mod.measureHeight(node, width);
            node.bounds.height = clampDim(node, height, false);
        },
        .select => {
            const height = assigned_height orelse select_mod.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .container, .root => {
            const definite_height: ?f32 = assigned_height orelse node.style.height;
            layoutContainer(nodes, node, width, definite_height, theme);
        },
    }
    return node.bounds.size();
}

fn layoutContainer(nodes: []*Node, node: *Node, width: f32, definite_height: ?f32, theme: Theme) void {
    const allocator = std.heap.c_allocator;
    var ids: std.ArrayListUnmanaged(NodeId) = .empty;
    defer ids.deinit(allocator);
    visibleChildren(node.children.items, nodes, &ids, allocator) catch return;

    const padding = node.style.padding;
    const inner_x = node.bounds.x + padding.left;
    const inner_y = node.bounds.y + padding.top;
    const inner_width = @max(0, width - padding.horizontal());
    const inner_height = if (definite_height) |h| @max(0, h - padding.vertical()) else 0;
    const n = ids.items.len;
    if (definite_height) |h| {
        node.bounds.height = clampDim(node, h, false);
    }

    if (node.style.direction == .row) {
        layoutRow(nodes, node, ids.items, inner_x, inner_y, inner_width, if (definite_height != null) inner_height else null, theme, allocator);
    } else {
        layoutColumn(nodes, node, ids.items, inner_x, inner_y, inner_width, if (definite_height != null) inner_height else null, theme, allocator);
    }

    if (definite_height) |h| {
        node.bounds.height = clampDim(node, h, false);
    } else if (n == 0) {
        node.bounds.height = clampDim(node, padding.vertical(), false);
    }
}

fn layoutRow(
    nodes: []*Node,
    node: *Node,
    ids: []const NodeId,
    inner_x: f32,
    inner_y: f32,
    inner_width: f32,
    inner_height: ?f32,
    theme: Theme,
    allocator: std.mem.Allocator,
) void {
    const n = ids.len;
    if (n == 0) {
        if (inner_height == null) node.bounds.height = node.style.padding.vertical();
        return;
    }
    var items = allocator.alloc(flex.Item, n) catch return;
    defer allocator.free(items);
    const widths = allocator.alloc(f32, n) catch return;
    defer allocator.free(widths);

    for (ids, 0..) |id, i| {
        const child = nodes[id];
        var base = child.style.basis orelse child.style.width orelse intrinsicWidth(nodes, child, inner_width, theme);
        base = clampDim(child, base, true);
        items[i] = .{
            .base = base,
            .grow = child.style.grow,
            .shrink = child.style.shrink,
            .min = child.style.min_width orelse 0,
            .max = child.style.max_width orelse std.math.inf(f32),
            .margin_start = child.style.margin.left,
            .margin_end = child.style.margin.right,
        };
    }

    var free = flex.resolve(items, inner_width, node.style.gap, widths);
    if (free < 0) free = 0;
    const distribution = flex.justifyDistribution(node.style.justify, free, n);
    var cursor = inner_x + distribution.start_offset;
    var tallest: f32 = 0;
    for (ids, 0..) |id, i| {
        const child = nodes[id];
        const child_width = widths[i];
        const left = cursor + child.style.margin.left;
        const cross_assigned: ?f32 = if (inner_height) |h| blk: {
            if (child.style.height) |explicit| break :blk explicit;
            if (node.style.align_items == .stretch) break :blk @max(0, h - child.style.margin.vertical());
            break :blk null;
        } else child.style.height;
        const size = layoutNode(
            nodes,
            child,
            left,
            inner_y + child.style.margin.top,
            child_width,
            cross_assigned,
            child_width,
            inner_height orelse 1.0e6,
            theme,
            false,
        );
        tallest = @max(tallest, size.height + child.style.margin.vertical());
        cursor += child_width + child.style.margin.right + node.style.gap + distribution.between;
    }

    if (inner_height == null) {
        node.bounds.height = clampDim(node, node.style.padding.vertical() + tallest, false);
    }
    const final_inner_height = @max(0, node.bounds.height - node.style.padding.vertical());
    for (ids) |id| {
        const child = nodes[id];
        if (node.style.align_items == .stretch and inner_height != null) continue;
        const remaining = @max(0, final_inner_height - child.bounds.height - child.style.margin.vertical());
        const offset: f32 = switch (node.style.align_items) {
            .start, .stretch => 0,
            .center => remaining * 0.5,
            .end => remaining,
        };
        const target_y = inner_y + child.style.margin.top + offset;
        translateSubtreeY(nodes, child, target_y - child.bounds.y);
    }
}

fn layoutColumn(
    nodes: []*Node,
    node: *Node,
    ids: []const NodeId,
    inner_x: f32,
    inner_y: f32,
    inner_width: f32,
    inner_height: ?f32,
    theme: Theme,
    allocator: std.mem.Allocator,
) void {
    const n = ids.len;
    if (n == 0) {
        if (inner_height == null) node.bounds.height = node.style.padding.vertical();
        return;
    }
    const widths = allocator.alloc(f32, n) catch return;
    defer allocator.free(widths);
    const heights = allocator.alloc(f32, n) catch return;
    defer allocator.free(heights);
    const items = allocator.alloc(flex.Item, n) catch return;
    defer allocator.free(items);

    for (ids, 0..) |id, i| {
        const child = nodes[id];
        var child_width = child.style.width orelse blk: {
            if (node.style.align_items == .stretch) {
                break :blk @max(0, inner_width - child.style.margin.horizontal());
            }
            break :blk intrinsicWidth(nodes, child, inner_width, theme);
        };
        child_width = clampDim(child, child_width, true);
        widths[i] = child_width;
        const base_height = child.style.basis orelse child.style.height orelse intrinsicHeight(nodes, child, child_width, theme);
        items[i] = .{
            .base = base_height,
            .grow = child.style.grow,
            .shrink = child.style.shrink,
            .min = child.style.min_height orelse 0,
            .max = child.style.max_height orelse std.math.inf(f32),
            .margin_start = child.style.margin.top,
            .margin_end = child.style.margin.bottom,
        };
    }

    var free: f32 = 0;
    if (inner_height) |h| {
        free = flex.resolve(items, h, node.style.gap, heights);
        if (free < 0) free = 0;
    } else {
        for (items, 0..) |item, i| heights[i] = clampDim(nodes[ids[i]], item.base, false);
    }
    const distribution = if (inner_height != null)
        flex.justifyDistribution(node.style.justify, free, n)
    else
        flex.Distribution{};
    var cursor = inner_y + distribution.start_offset;
    for (ids, 0..) |id, i| {
        const child = nodes[id];
        const child_height: ?f32 = if (inner_height != null) heights[i] else child.style.height;
        const size = layoutNode(
            nodes,
            child,
            inner_x + child.style.margin.left,
            cursor + child.style.margin.top,
            widths[i],
            child_height,
            widths[i],
            inner_height orelse 1.0e6,
            theme,
            false,
        );
        cursor += size.height + child.style.margin.bottom + node.style.gap + distribution.between;
    }

    if (inner_height == null) {
        var content_height: f32 = 0;
        for (ids, 0..) |id, i| {
            const child = nodes[id];
            content_height += child.bounds.height + child.style.margin.vertical();
            if (i > 0) content_height += node.style.gap;
        }
        node.bounds.height = clampDim(node, node.style.padding.vertical() + content_height, false);
    }

    const final_inner_width = @max(0, node.bounds.width - node.style.padding.horizontal());
    for (ids) |id| {
        const child = nodes[id];
        const remaining = @max(0, final_inner_width - child.bounds.width - child.style.margin.horizontal());
        const offset: f32 = switch (node.style.align_items) {
            .start, .stretch => 0,
            .center => remaining * 0.5,
            .end => remaining,
        };
        const target_x = inner_x + child.style.margin.left + offset;
        translateSubtreeX(nodes, child, target_x - child.bounds.x);
    }
}

pub fn translateSubtreeX(nodes: []*Node, node: *Node, dx: f32) void {
    if (dx == 0) return;
    node.bounds.x += dx;
    for (node.children.items) |id| {
        translateSubtreeX(nodes, nodes[id], dx);
    }
}

pub fn translateSubtreeY(nodes: []*Node, node: *Node, dy: f32) void {
    if (dy == 0) return;
    node.bounds.y += dy;
    for (node.children.items) |id| {
        translateSubtreeY(nodes, nodes[id], dy);
    }
}
