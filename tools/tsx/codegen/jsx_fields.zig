//! Merge option fields and edges in source order, then emit stable builder literals.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Error = core.Error;

/// Each field has one entry. Edges merge independently per side in attribute
/// order; ui.inset is used only when all sides share the same source expression.
pub const Fields = struct {
    g: *Generator,
    list: std.ArrayList(*Entry) = .empty,
    const Edge = struct { code: []const u8, source: *ast.Expr };
    const Entry = struct { field: []const u8, code: ?[]const u8 = null, edges: [4]?Edge = .{ null, null, null, null } };
    pub fn entry(f: *Fields, field: []const u8) Error!*Entry {
        for (f.list.items) |item| if (zig_text.eq(item.field, field)) return item;
        const item = try f.g.a.create(Entry);
        item.* = .{ .field = field };
        try f.list.append(f.g.a, item);
        return item;
    }
    pub fn remove(f: *Fields, field: []const u8) void {
        for (f.list.items, 0..) |item, i| if (zig_text.eq(item.field, field)) {
            _ = f.list.orderedRemove(i);
            return;
        };
    }
    pub fn has(f: *Fields, field: []const u8) bool {
        for (f.list.items) |item| if (zig_text.eq(item.field, field)) return true;
        return false;
    }
    pub fn put(f: *Fields, field: []const u8, value: []const u8) Error!void {
        (try f.entry(field)).code = value;
    }
    /// Content follows key, with all other option fields retaining their order.
    pub fn contentFirst(f: *Fields, field: []const u8) void {
        var content: ?*Entry = null;
        for (f.list.items, 0..) |item, i| {
            if (zig_text.eq(item.field, field)) {
                content = f.list.orderedRemove(i);
                break;
            }
        }
        const value = content orelse return;
        var position: usize = 0;
        for (f.list.items, 0..) |item, i| {
            if (zig_text.eq(item.field, "key")) {
                const key = f.list.orderedRemove(i);
                f.list.insertAssumeCapacity(0, key);
                position = 1;
                break;
            }
        }
        // Removing one entry left enough capacity to insert without allocating.
        f.list.insertAssumeCapacity(position, value);
    }

    pub fn emit(f: *Fields) Error![]const u8 {
        var out: std.ArrayList(u8) = .empty;
        try out.appendSlice(f.g.a, ".{\n");
        for (f.list.items) |item| {
            const code = item.code orelse try f.edgesValue(item.edges);
            try out.appendSlice(
                f.g.a,
                try zig_text.fmt(f.g, ".{s} = {s},\n", .{ try zig_text.id(f.g, item.field), code }),
            );
        }
        try out.appendSlice(f.g.a, "}");
        return out.items;
    }

    /// Use inset only when every side shares the same original expression.
    fn edgesValue(f: *Fields, edges: [4]?Edge) Error![]const u8 {
        const top = if (edges[0]) |edge| edge.code else "0";
        const right = if (edges[1]) |edge| edge.code else "0";
        const bottom = if (edges[2]) |edge| edge.code else "0";
        const left = if (edges[3]) |edge| edge.code else "0";
        const all_sides = edges[0] != null and edges[1] != null and edges[2] != null and edges[3] != null;
        if (all_sides) {
            const source = edges[0].?.source;
            const same_source = source == edges[1].?.source and source == edges[2].?.source and
                source == edges[3].?.source;
            if (same_source) return zig_text.fmt(f.g, "ui.inset({s})", .{top});
        }
        return zig_text.fmt(
            f.g,
            ".{{ .top = {s}, .right = {s}, .bottom = {s}, .left = {s}, }}",
            .{ top, right, bottom, left },
        );
    }
};
