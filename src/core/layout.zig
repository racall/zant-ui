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

fn resolveWidth(node: *const Node, available: f32) ?f32 {
    if (node.style.width_percent) |percent| return @max(0, available) * percent / 100.0;
    return node.style.width;
}

fn effectiveAlign(parent: *const Node, child: *const Node) flex.Align {
    return switch (child.style.align_self) {
        .auto => parent.style.align_items,
        .start => .start,
        .center => .center,
        .end => .end,
        .stretch => .stretch,
    };
}

fn gapMain(node: *const Node) f32 {
    return if (node.style.direction == .row)
        (node.style.column_gap orelse node.style.gap)
    else
        (node.style.row_gap orelse node.style.gap);
}

fn gapCross(node: *const Node) f32 {
    return if (node.style.direction == .row)
        (node.style.row_gap orelse node.style.gap)
    else
        (node.style.column_gap orelse node.style.gap);
}

fn orderIds(nodes: []*Node, ids: []const NodeId, allocator: std.mem.Allocator) []NodeId {
    const copy = allocator.dupe(NodeId, ids) catch return @constCast(ids);
    var i: usize = 1;
    while (i < copy.len) : (i += 1) {
        const value = copy[i];
        var j = i;
        while (j > 0 and nodes[copy[j - 1]].style.order > nodes[value].style.order) : (j -= 1) {
            copy[j] = copy[j - 1];
        }
        copy[j] = value;
    }
    return copy;
}

fn finite(v: f32, fallback: f32) f32 {
    if (std.math.isFinite(v) and v > 0) return v;
    return fallback;
}

fn visibleChildren(children: []const NodeId, nodes: []*Node, out: *std.ArrayListUnmanaged(NodeId), allocator: std.mem.Allocator) !void {
    out.clearRetainingCapacity();
    for (children) |id| {
        const child = nodes[id];
        if (!child.inFlow()) continue;
        try out.append(allocator, id);
    }
}

pub fn intrinsicWidth(nodes: []*Node, node: *Node, max_avail: f32, theme: Theme) f32 {
    if (node.style.width_percent) |percent| return clampDim(node, @max(0, max_avail) * percent / 100.0, true);
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
                // absolutely positioned children do not size their parent
                if (!child.inFlow() or child.style.absolute) continue;
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
        .text => |*data| data.measureWidth(if (data.style.wrap) avail else 1.0e6),
        .button => button.measureWidth(node, theme, avail),
        .input => input.measureWidth(node),
        .toggle => toggle.measureWidth(node, theme, avail),
        .image => image_mod.measureWidth(node),
        .select => select_mod.measureWidth(node),
        .rich_text => |*data| data.naturalWidth(),
        .vector => |*data| data.drawing.width,
    };
    return clampDim(node, value, true);
}

pub fn intrinsicHeight(nodes: []*Node, node: *Node, width: f32, theme: Theme) f32 {
    if (node.style.height) |h| return clampDim(node, h, false);
    // A scroll container's natural height is its viewport, not its content.
    if (node.style.scroll) return clampDim(node, node.style.padding.vertical(), false);
    const value: f32 = switch (node.data) {
        .root, .container => blk: {
            const inner_width = @max(0, width - node.style.padding.horizontal());
            var total: f32 = 0;
            var tallest: f32 = 0;
            if (node.style.direction == .row) {
                for (node.children.items) |id| {
                    const child = nodes[id];
                    if (!child.inFlow() or child.style.absolute) continue;
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
                    if (!child.inFlow() or child.style.absolute) continue;
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
        .rich_text => |*data| if (data.ensureLayout(width)) |layout| layout.height else |_| @as(f32, @floatFromInt(data.spans.len)) * 20,
        .vector => |*data| data.drawing.height(),
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
    _ = is_root;
    var width = assigned_width orelse resolveWidth(node, available_width) orelse intrinsicWidth(nodes, node, available_width, theme);
    width = clampDim(node, @max(0, width), true);
    node.bounds.x = x;
    node.bounds.y = y;
    node.bounds.width = width;

    switch (node.data) {
        .text => |*data| {
            var height = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else node.style.height;
            const max_width = data.layoutWidth(width);
            if (data.ensureLayout(max_width)) |layout| {
                if (height == null) height = layout.metrics.height;
            } else |_| {
                if (height == null) height = data.style.font_size * 1.2;
            }
            node.bounds.height = clampDim(node, @max(0, height orelse 0), false);
        },
        .button => {
            const height = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else button.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .input => {
            const height = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else input.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .toggle => {
            const height = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else toggle.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .image => {
            const height = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else node.style.height orelse image_mod.measureHeight(node, width);
            node.bounds.height = clampDim(node, height, false);
        },
        .select => {
            const height = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else select_mod.measureHeight(node);
            node.bounds.height = clampDim(node, height, false);
        },
        .vector => |*data| {
            const height = assigned_height orelse node.style.height orelse data.drawing.height();
            node.bounds.height = clampDim(node, height, false);
        },
        .rich_text => |*data| {
            const layout_h = if (data.ensureLayout(width)) |layout| layout.height else |_| @as(f32, @floatFromInt(data.spans.len)) * 20;
            const height = assigned_height orelse layout_h;
            node.bounds.height = clampDim(node, height, false);
        },
        .container, .root => {
            const definite_height: ?f32 = assigned_height orelse if (node.style.height_percent) |percent| @max(0, available_height) * percent / 100.0 else node.style.height;
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
    const ordered = orderIds(nodes, ids.items, allocator);
    defer if (ordered.ptr != ids.items.ptr) allocator.free(ordered);

    var flow: std.ArrayListUnmanaged(NodeId) = .empty;
    defer flow.deinit(allocator);
    var absolute: std.ArrayListUnmanaged(NodeId) = .empty;
    defer absolute.deinit(allocator);
    for (ordered) |id| {
        if (nodes[id].style.absolute) {
            absolute.append(allocator, id) catch return;
        } else {
            flow.append(allocator, id) catch return;
        }
    }

    const padding = node.style.padding;
    const inner_x = node.bounds.x + padding.left;
    const inner_y = node.bounds.y + padding.top;
    const inner_width = @max(0, width - padding.horizontal());
    const inner_height = if (definite_height) |h| @max(0, h - padding.vertical()) else 0;
    const n = flow.items.len;
    if (definite_height) |h| {
        node.bounds.height = clampDim(node, h, false);
    }

    if (node.style.direction == .row) {
        if (node.style.wrap != .nowrap) {
            layoutRowWrap(nodes, node, flow.items, inner_x, inner_y, inner_width, if (definite_height != null) inner_height else null, theme, allocator);
        } else {
            layoutRow(nodes, node, flow.items, inner_x, inner_y, inner_width, if (definite_height != null) inner_height else null, theme, allocator);
        }
    } else if (node.style.scroll and definite_height != null) {
        // Scroll containers lay children out at their natural height, then
        // shift them by the retained scroll offset.
        layoutColumn(nodes, node, flow.items, inner_x, inner_y, inner_width, null, theme, allocator);
        const content = @max(0, node.bounds.height - padding.vertical());
        node.bounds.height = clampDim(node, definite_height.?, false);
        switch (node.data) {
            .container => |*data| {
                data.content_height = content + padding.vertical();
                data.scroll_offset = @max(0, @min(data.scroll_offset, data.maxScroll(node.bounds.height)));
                for (flow.items) |id| translateSubtreeY(nodes, nodes[id], -data.scroll_offset);
            },
            else => {},
        }
    } else {
        layoutColumn(nodes, node, flow.items, inner_x, inner_y, inner_width, if (definite_height != null) inner_height else null, theme, allocator);
    }

    // CSS containing block: absolute children are positioned and sized
    // against the parent's padding box (padding included), so a full-size
    // hit target / hover background also covers the parent's padding.
    const box_x = node.bounds.x;
    const box_y = node.bounds.y;
    const box_w = node.bounds.width;
    const box_h = node.bounds.height;
    for (absolute.items) |id| {
        const child = nodes[id];
        // CSS: `left` + `right` without a width stretch between the insets
        const inset_w: ?f32 = if (child.style.left != null and child.style.right != null and child.style.width == null and child.style.width_percent == null)
            @max(0, box_w - child.style.left.? - child.style.right.?)
        else
            null;
        const w = inset_w orelse resolveWidth(child, box_w) orelse intrinsicWidth(nodes, child, box_w, theme);
        // a percentage wins over a builder's fixed height (as in flow layout),
        // so `ui.styled(button, .{ .height_percent = 100 })` fills the parent
        const h = if (child.style.height_percent) |percent|
            box_h * percent / 100.0
        else if (child.style.height) |explicit|
            explicit
        else
            intrinsicHeight(nodes, child, w, theme);
        var x = box_x;
        var y = box_y;
        if (child.style.left) |left| x = box_x + left else if (child.style.right) |right| x = box_x + box_w - w - right;
        if (child.style.top) |top| y = box_y + top else if (child.style.bottom) |bottom| y = box_y + box_h - h - bottom;
        if (child.style.anchor) |n_anchor| {
            if (n_anchor < flow.items.len) {
                // follow the n-th in-flow sibling (FLIP animates the moves)
                const target = nodes[flow.items[n_anchor]].bounds;
                const s = child.style;
                const own_vertical = s.top != null or s.bottom != null or s.height != null or s.height_percent != null;
                _ = layoutNode(nodes, child, target.x, if (own_vertical) y else target.y, target.width, if (own_vertical) h else target.height, target.width, box_h, theme, false);
                continue;
            }
        }
        _ = layoutNode(nodes, child, x, y, w, h, w, box_h, theme, false);
    }

    if (definite_height) |h| {
        node.bounds.height = clampDim(node, h, false);
    } else if (n == 0) {
        node.bounds.height = clampDim(node, padding.vertical(), false);
    }
}

const WrapLine = struct { start: usize, end: usize };

fn layoutRowWrap(
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
    const items = allocator.alloc(flex.Item, n) catch return;
    defer allocator.free(items);
    const widths = allocator.alloc(f32, n) catch return;
    defer allocator.free(widths);
    const heights = allocator.alloc(f32, n) catch return;
    defer allocator.free(heights);
    const aligns = allocator.alloc(flex.Align, n) catch return;
    defer allocator.free(aligns);

    const main_gap = gapMain(node);
    for (ids, 0..) |id, i| {
        const child = nodes[id];
        var base = child.style.basis orelse resolveWidth(child, inner_width) orelse intrinsicWidth(nodes, child, inner_width, theme);
        base = clampDim(child, base, true);
        items[i] = .{ .base = base, .grow = child.style.grow, .shrink = child.style.shrink, .min = child.style.min_width orelse 0, .max = child.style.max_width orelse std.math.inf(f32), .margin_start = child.style.margin.left, .margin_end = child.style.margin.right };
    }

    var lines: std.ArrayListUnmanaged(WrapLine) = .empty;
    defer lines.deinit(allocator);
    var start: usize = 0;
    var used: f32 = 0;
    for (ids, 0..) |id, i| {
        const child = nodes[id];
        const outer = items[i].base + child.style.margin.horizontal();
        if (i > start and used + main_gap + outer > inner_width + 0.01) {
            lines.append(allocator, .{ .start = start, .end = i }) catch return;
            start = i;
            used = outer;
        } else {
            if (i > start) used += main_gap;
            used += outer;
        }
    }
    lines.append(allocator, .{ .start = start, .end = n }) catch return;

    for (lines.items) |line| {
        var line_height: f32 = 0;
        for (ids[line.start..line.end]) |id| {
            const child = nodes[id];
            line_height = @max(line_height, (child.style.height orelse intrinsicHeight(nodes, child, items[line.start].base, theme)) + child.style.margin.vertical());
        }
        if (line_height <= 0) line_height = 1;
        heights[line.start] = line_height;
    }

    var total: f32 = 0;
    for (lines.items, 0..) |line, index| {
        if (index > 0) total += gapCross(node);
        total += heights[line.start];
    }
    const final_height = inner_height orelse total;
    if (inner_height == null) node.bounds.height = clampDim(node, node.style.padding.vertical() + total, false);

    const free_cross = @max(0, final_height - total);
    var offset: f32 = 0;
    var between: f32 = 0;
    const line_count = lines.items.len;
    switch (node.style.align_content) {
        .start, .stretch => {},
        .end => offset = free_cross,
        .center => offset = free_cross * 0.5,
        .space_between => {
            if (line_count > 1) between = free_cross / @as(f32, @floatFromInt(line_count - 1));
        },
        .space_around => if (line_count > 0) {
            offset = free_cross / @as(f32, @floatFromInt(line_count * 2));
            between = free_cross / @as(f32, @floatFromInt(line_count));
        },
    }

    var line_y = inner_y + offset;
    for (lines.items, 0..) |line, index| {
        const line_height = heights[line.start];
        const count = line.end - line.start;
        var free = flex.resolve(items[line.start..line.end], inner_width, main_gap, widths[line.start..line.end]);
        if (free < 0) free = 0;
        const distribution = flex.justifyDistribution(node.style.justify, free, count);
        var cursor = inner_x + distribution.start_offset;
        for (ids[line.start..line.end], 0..) |id, offset_in_line| {
            const i = line.start + offset_in_line;
            const child = nodes[id];
            const alignment = effectiveAlign(node, child);
            aligns[i] = alignment;
            const intrinsic_h = child.style.height orelse intrinsicHeight(nodes, child, widths[i], theme);
            const assigned_h: ?f32 = if (alignment == .stretch)
                @max(0, line_height - child.style.margin.vertical())
            else
                child.style.height;
            const remaining = @max(0, line_height - child.style.margin.vertical() - intrinsic_h);
            const cross = switch (alignment) {
                .start, .stretch => 0,
                .center => remaining * 0.5,
                .end => remaining,
            };
            _ = layoutNode(nodes, child, cursor + child.style.margin.left, line_y + child.style.margin.top + cross, widths[i], assigned_h, widths[i], line_height, theme, false);
            cursor += widths[i] + child.style.margin.right + main_gap + distribution.between;
        }
        line_y += line_height;
        if (index + 1 < line_count) line_y += gapCross(node) + between;
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
        var base = child.style.basis orelse resolveWidth(child, inner_width) orelse intrinsicWidth(nodes, child, inner_width, theme);
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

    var free = flex.resolve(items, inner_width, gapMain(node), widths);
    if (free < 0) free = 0;
    const distribution = flex.justifyDistribution(node.style.justify, free, n);
    var cursor = inner_x + distribution.start_offset;
    var tallest: f32 = 0;
    for (ids, 0..) |id, i| {
        const child = nodes[id];
        const child_width = widths[i];
        const left = cursor + child.style.margin.left;
        const alignment = effectiveAlign(node, child);
        const cross_assigned: ?f32 = if (inner_height) |h| blk: {
            if (child.style.height) |explicit| break :blk explicit;
            if (alignment == .stretch) break :blk @max(0, h - child.style.margin.vertical());
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
        cursor += child_width + child.style.margin.right + gapMain(node) + distribution.between;
    }

    if (inner_height == null) {
        node.bounds.height = clampDim(node, node.style.padding.vertical() + tallest, false);
    }
    const final_inner_height = @max(0, node.bounds.height - node.style.padding.vertical());
    if (inner_height == null) {
        // CSS: stretch items fill the line's cross size even when the row's
        // height comes from its content.
        for (ids) |id| {
            const child = nodes[id];
            if (effectiveAlign(node, child) != .stretch or child.style.height != null) continue;
            const target = @max(0, final_inner_height - child.style.margin.vertical());
            if (@abs(child.bounds.height - target) < 0.25) continue;
            _ = layoutNode(nodes, child, child.bounds.x, inner_y + child.style.margin.top, child.bounds.width, target, child.bounds.width, final_inner_height, theme, false);
        }
    }
    for (ids) |id| {
        const child = nodes[id];
        const alignment = effectiveAlign(node, child);
        if (alignment == .stretch and child.style.height == null) continue;
        const remaining = @max(0, final_inner_height - child.bounds.height - child.style.margin.vertical());
        const offset: f32 = switch (alignment) {
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
        var child_width = resolveWidth(child, inner_width) orelse blk: {
            if (effectiveAlign(node, child) == .stretch) {
                break :blk @max(0, inner_width - child.style.margin.horizontal());
            }
            break :blk intrinsicWidth(nodes, child, inner_width, theme);
        };
        child_width = clampDim(child, child_width, true);
        widths[i] = child_width;
        // flex bases only matter with a definite height (grow / shrink);
        // otherwise heights come from laying the children out below, and
        // measuring here would lay out the whole subtree a second time at
        // estimated widths (re-shaping every paragraph of a long chat)
        const base_height: f32 = if (inner_height == null) 0 else child.style.basis orelse
            (if (child.style.height_percent) |percent| @max(0, inner_height.?) * percent / 100.0 else child.style.height) orelse
            intrinsicHeight(nodes, child, child_width, theme);
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
        free = flex.resolve(items, h, gapMain(node), heights);
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
        cursor += size.height + child.style.margin.bottom + gapMain(node) + distribution.between;
    }

    if (inner_height == null) {
        var content_height: f32 = 0;
        for (ids, 0..) |id, i| {
            const child = nodes[id];
            content_height += child.bounds.height + child.style.margin.vertical();
            if (i > 0) content_height += gapMain(node);
        }
        node.bounds.height = clampDim(node, node.style.padding.vertical() + content_height, false);
    }

    const final_inner_width = @max(0, node.bounds.width - node.style.padding.horizontal());
    for (ids) |id| {
        const child = nodes[id];
        const alignment = effectiveAlign(node, child);
        const remaining = @max(0, final_inner_width - child.bounds.width - child.style.margin.horizontal());
        const offset: f32 = switch (alignment) {
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
