//! Plain top-level functions (not components): a lowercase name, typed
//! parameters and a return type, like React's convention that only
//! capitalized functions are components.
//!
//!     export function bytes(n: number): string { … }
//!
//! becomes a file-level `pub fn bytes(n: f64) []const u8`, callable from
//! components, from other functions (also recursively) and from other files
//! (`import { bytes } from "./format"` → `_format.bytes(…)`). They have no
//! state and no hooks; stores, other functions and Zig imports are fine.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const stmt = @import("stmt.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

pub const Function = struct {
    /// The name in this file.
    name: []const u8,
    /// Zig callee: `bytes` or `_format.bytes`.
    zig: []const u8,
    func: *ast.Function,
    params: []const Type,
    ret: Type,
    external: bool = false,
};

/// Components start with a capital letter; everything else is a function.
pub fn isFunction(f: *const ast.Function) bool {
    return f.name.len > 0 and !std.ascii.isUpper(f.name[0]);
}

/// Resolves the signature of `f` (types in the current type scope).
pub fn signature(g: *Generator, f: *ast.Function, zig: []const u8, external: bool) Error!Function {
    var params: std.ArrayList(Type) = .empty;
    for (f.params) |p| {
        if (p.destructure.len > 0) return core.fail(g, p.name.loc, "function parameters are plain names", .{});
        const ref = p.ty orelse return core.fail(
            g,
            p.name.loc,
            "parameter `{s}` of `{s}` needs a type",
            .{ p.name.name, f.name },
        );
        var ty = try types.typeRef(g, ref, false);
        if (p.optional) ty = try @import("model.zig").optional(g, ty);
        try params.append(g.a, ty);
    }
    const ret: Type = if (f.ret) |r| try types.typeRef(g, r, true) else .void_;
    if (ret == .node or ret == .optional_node) return core.fail(
        g,
        f.loc,
        "`{s}` returns elements: components start with a capital letter",
        .{f.name},
    );
    return .{ .name = f.name, .zig = zig, .func = f, .params = params.items, .ret = ret, .external = external };
}

/// Registers this file's functions (before anything is emitted, so they can
/// call each other in any order).
pub fn collect(g: *Generator, module: ast.Module) Error!void {
    for (module.functions) |top| {
        if (!isFunction(top.func)) continue;
        if (top.default) return core.fail(g, top.func.loc, "the default export is a component (capitalize it)", .{});
        for (g.functions.items) |other| if (text.eq(other.name, top.func.name)) {
            return core.fail(g, top.func.loc, "duplicate function `{s}`", .{top.func.name});
        };
        try g.functions.append(g.a, try signature(g, top.func, try text.id(g, top.func.name), false));
    }
}

pub fn find(g: *Generator, name: []const u8) ?Function {
    for (g.functions.items) |f| if (text.eq(f.name, name)) return f;
    return null;
}

/// `bytes(n)` where `bytes` is a function of this file or an imported one.
pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    if (call.callee.data != .ident) return null;
    const name = call.callee.data.ident;
    if (scope.local(c, name) != null) return null;
    const f = find(g, name) orelse return null;
    if (call.type_args.len != 0) return core.fail(g, loc, "`{s}` takes no type arguments", .{name});
    const required = blk: {
        var n: usize = 0;
        for (f.func.params) |p| {
            if (!p.optional) n += 1;
        }
        break :blk n;
    };
    if (call.args.len < required or call.args.len > f.params.len) {
        return core.fail(g, loc, "`{s}` takes {d} arguments", .{ name, f.params.len });
    }
    var args: std.ArrayList([]const u8) = .empty;
    for (f.params, 0..) |ty, i| {
        if (i >= call.args.len) {
            try args.append(g.a, "null");
            continue;
        }
        const v = try expr.emit(g, c, call.args[i], ty, false);
        try args.append(g.a, (try types.coerce(g, v, ty, call.args[i].loc)).code);
    }
    return .{ .ty = f.ret, .code = try text.fmt(g, "{s}({s})", .{ f.zig, try std.mem.join(g.a, ", ", args.items) }) };
}

/// The Zig declarations of this file's functions.
pub fn emitAll(g: *Generator) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    for (g.functions.items) |f| {
        if (f.external) continue;
        try out.appendSlice(g.a, try emit(g, f));
        try out.appendSlice(g.a, "\n");
    }
    return out.items;
}

fn emit(g: *Generator, f: Function) Error![]const u8 {
    var c = Context{ .g = g, .render = false, .ret = f.ret, .no_self = true };
    var params: std.ArrayList([]const u8) = .empty;
    for (f.func.params, f.params) |p, ty| {
        const b = try scope.bind(&c, p.name.name, ty, false);
        try c.params.append(g.a, b);
        try params.append(g.a, try text.fmt(g, "{s}: {s}", .{ b.zig, ty.zig(g) }));
    }
    const body = switch (f.func.body) {
        .block => |ss| blk: {
            if (f.ret != .void_ and !stmt.returnsAll(ss)) {
                return core.fail(g, f.func.loc, "`{s}` must return a value on every path", .{f.name});
            }
            break :blk try stmt.statements(g, &c, ss, false);
        },
        .expr => |e| if (f.ret == .void_)
            try stmt.expressionStatement(g, &c, e)
        else
            try text.fmt(g, "return {s};\n", .{(try expr.emit(g, &c, e, f.ret, false)).code}),
    };
    return text.fmt(g, "pub fn {s}({s}) {s} {{\n{s}}}\n", .{
        f.zig,
        try std.mem.join(g.a, ", ", params.items),
        f.ret.zig(g),
        try scope.finish(&c, body),
    });
}
