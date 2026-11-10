//! Event arrows capture render locals by value. Named component functions that
//! read render locals are emitted like arrows where they are passed, and a
//! call to one from such a handler gets its own copy (`nested`).
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
    // a local where the arrow is made, or what that place captured itself
    // (an arrow inside an arrow: `useEffect(() => setTimeout(() => hide(id), 5))`)
    var outer: ?types.Value = null;
    const binding = scope.local(source, n);
    if (binding == null) {
        if (scope.local(c, n) != null) return null;
        outer = (try read(g, source, n)) orelse return null;
    }
    // narrowed where the arrow is made (`peer && <Button onClick={() => use(peer.name)}/>`)
    const narrowed = @import("narrowing.zig").find(source, n);
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
    const ty = if (narrowed) |v| v.ty else if (outer) |v| v.ty else binding.?.ty;
    try g.methods.items[index].captures.append(g.a, .{
        .name = n,
        .field = field,
        .ty = ty,
        .source = if (narrowed) |v| v.code else if (outer) |v| v.code else binding.?.zig,
    });
    return .{ .ty = ty, .code = try text.fmt(g, "cap.{s}", .{try text.id(g, field)}) };
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

pub const Nested = struct { receiver: []const u8, target: []const u8 };

/// A call to component function `index` (it reads render locals) from a
/// handler emitted at a render site: a copy of the function capturing from
/// that render, its captured values read through the calling handler.
pub fn nested(g: *core.Generator, c: *scope.Context, loc: @import("../ast.zig").Loc, index: usize) core.Error!Nested {
    const source = c.capture_source orelse return core.fail(
        g,
        loc,
        "`{s}` reads render locals; only handlers made while rendering (events, effects) can call it",
        .{g.methods.items[index].name},
    );
    const m = g.methods.items[index];
    g.counter += 1;
    try g.methods.append(g.a, .{
        .name = try scope.uniqueMethod(g, try text.fmt(g, "_{s}{d}", .{ m.name, g.counter })),
        .func = m.func,
        .ret = m.ret,
        .capture_name = try scope.uniqueMethod(g, try text.fmt(g, "_Capture{d}", .{g.counter})),
    });
    const copy = g.methods.items.len - 1;
    try prepare(g, source, copy);
    const inner = g.methods.items[copy];
    const target = try text.fmt(g, "{s}.{s}", .{ try text.id(g, g.name), try text.id(g, inner.name) });
    if (inner.captures.items.len == 0) return .{ .receiver = "self", .target = target };
    var fields: std.ArrayList(u8) = .empty;
    try fields.appendSlice(g.a, ".self = self,\n");
    for (inner.captures.items) |capture| {
        const v = (try read(g, c, capture.name)) orelse unreachable; // a local of the same render
        try fields.appendSlice(g.a, try text.fmt(g, ".{s} = {s},\n", .{ try text.id(g, capture.field), v.code }));
    }
    const receiver = try text.fmt(g, "tsx.capture({s}{{\n{s}}})", .{ inner.capture_name.?, fields.items });
    return .{ .receiver = receiver, .target = target };
}
