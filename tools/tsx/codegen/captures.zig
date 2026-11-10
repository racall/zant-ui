//! Event arrows capture render locals by value; named component functions cannot capture.
const std = @import("std");
const core = @import("codegen.zig");
const scope = @import("scope.zig");
const types = @import("types.zig");
const text = @import("zig_text.zig");
const events = @import("events.zig");

pub const Capture = struct { name: []const u8, field: []const u8, ty: types.Type, source: []const u8 };

/// Discover captures while generating the inline method in its lexical source context.
pub fn prepare(g: *core.Generator, c: *scope.Context, index: usize) core.Error!void {
    if (g.methods.items[index].emitted != null) return;
    const code = try core.method(g, g.methods.items[index], c, index);
    g.methods.items[index].emitted = code;
}

pub fn read(g: *core.Generator, c: *scope.Context, n: []const u8) core.Error!?types.Value {
    const source = c.capture_source orelse return null;
    const binding = scope.local(source, n) orelse return null;
    const index = c.capture_method.?;
    for (g.methods.items[index].captures.items) |capture| if (text.eq(capture.name, n)) {
        return .{ .ty = capture.ty, .code = try text.fmt(g, "cap.{s}", .{try text.id(g, capture.field)}) };
    };
    var field = n;
    if (text.eq(n, "self")) field = "_captured_self";
    var suffix: usize = 2;
    while (captureFieldTaken(g.methods.items[index], field)) : (suffix += 1) {
        field = try text.fmt(g, "{s}_{d}", .{ n, suffix });
    }
    try g.methods.items[index].captures.append(g.a, .{
        .name = n,
        .field = field,
        .ty = binding.ty,
        .source = binding.zig,
    });
    return .{ .ty = binding.ty, .code = try text.fmt(g, "cap.{s}", .{try text.id(g, field)}) };
}

/// One capture struct is shared by all instances of an arrow (including map rows).
pub fn declarations(g: *core.Generator) core.Error![]const u8 {
    var code: std.ArrayList(u8) = .empty;
    for (g.methods.items) |m| {
        if (m.captures.items.len == 0) continue;
        try code.appendSlice(g.a, try text.fmt(g, "const {s} = struct {{\nself: *{s},\n", .{
            m.capture_name.?, try text.id(g, g.name),
        }));
        for (m.captures.items) |capture| try code.appendSlice(g.a, try text.fmt(g, "{s}: {s},\n", .{
            try text.id(g, capture.field), capture.ty.zig(g),
        }));
        try code.appendSlice(g.a, "};\n\n");
    }
    return code.items;
}

pub fn value(g: *core.Generator, m: events.Method) core.Error![]const u8 {
    var fields: std.ArrayList(u8) = .empty;
    try fields.appendSlice(g.a, ".self = self,\n");
    for (m.captures.items) |capture| try fields.appendSlice(g.a, try text.fmt(g, ".{s} = {s},\n", .{
        try text.id(g, capture.field), capture.source,
    }));
    return text.fmt(g, "tsx.capture({s}{{\n{s}}})", .{ m.capture_name.?, fields.items });
}

fn captureFieldTaken(method: events.Method, field: []const u8) bool {
    if (text.eq(field, "self")) return true;
    for (method.captures.items) |entry| if (text.eq(entry.field, field)) return true;
    return false;
}
