//! JSX spread attributes: `<Button {...common} onClick={save} />`. Before a
//! tag or component reads its attributes, every spread becomes plain
//! attributes in source order, so a later attribute wins as in React:
//!
//! - an object literal `{...{ size: "small" }}` spreads its properties;
//! - a typed object (`common: ButtonStyle`) spreads `common.field` for each
//!   field of its type; it must be a name or a member path (it is read once
//!   per field), and optional fields can't be spread (the prop would be
//!   "maybe absent").
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const model = @import("model.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Error = core.Error;

/// Whether `attrs` has a spread.
pub fn any(attrs: []const ast.Attr) bool {
    for (attrs) |a| if (a.spread) return true;
    return false;
}

/// Object-literal spreads expanded, typed ones left in place (handler
/// scanning: arrows inside literals register like attributes).
pub fn literals(g: *Generator, attrs: []const ast.Attr) Error![]const ast.Attr {
    if (!any(attrs)) return attrs;
    var out: std.ArrayList(ast.Attr) = .empty;
    for (attrs) |a| {
        if (a.spread and a.value.?.data == .object) {
            try objectLiteral(g, a.value.?, &out, false, null);
        } else try out.append(g.a, a);
    }
    return out.items;
}

/// Every spread expanded; a later attribute replaces an earlier one.
pub fn flatten(g: *Generator, c: *Context, attrs: []const ast.Attr) Error![]const ast.Attr {
    if (!any(attrs)) return attrs;
    var out: std.ArrayList(ast.Attr) = .empty;
    for (attrs) |a| {
        if (!a.spread) {
            try put(g, &out, a);
            continue;
        }
        const value = a.value.?;
        if (value.data == .object) {
            try objectLiteral(g, value, &out, true, c);
            continue;
        }
        try typedObject(g, c, value, &out);
    }
    return out.items;
}

fn put(g: *Generator, out: *std.ArrayList(ast.Attr), a: ast.Attr) Error!void {
    for (out.items, 0..) |existing, i| if (text.eq(existing.name.name, a.name.name)) {
        _ = out.orderedRemove(i);
        break;
    };
    try out.append(g.a, a);
}

fn objectLiteral(g: *Generator, e: *ast.Expr, out: *std.ArrayList(ast.Attr), replace: bool, c: ?*Context) Error!void {
    for (e.data.object) |p| {
        if (p.spread) {
            if (c) |ctx| {
                if (p.value.data == .object) try objectLiteral(g, p.value, out, replace, c) else try typedObject(g, ctx, p.value, out);
            } else if (p.value.data == .object) try objectLiteral(g, p.value, out, replace, null);
            continue;
        }
        const a = ast.Attr{ .name = p.key, .value = p.value };
        if (replace) try put(g, out, a) else try out.append(g.a, a);
    }
}

fn typedObject(g: *Generator, c: *Context, e: *ast.Expr, out: *std.ArrayList(ast.Attr)) Error!void {
    if (!path(e)) return core.fail(
        g,
        e.loc,
        "spread a name or a member path (`{{...style}}`, `{{...props.common}}`); store other values in a const first",
        .{},
    );
    const value = try expr.emit(g, c, e, null, false);
    const info = model.info(g, value.ty) orelse return core.fail(g, e.loc, "only objects can be spread", .{});
    if (info.kind != .object) return core.fail(g, e.loc, "only objects can be spread", .{});
    for (info.fields) |f| {
        if (f.optional) return core.fail(
            g,
            e.loc,
            "optional field `{s}` can't be spread (the prop might be absent); pass it explicitly",
            .{f.name},
        );
        const member = try g.a.create(ast.Expr);
        member.* = .{ .loc = e.loc, .data = .{ .member = .{ .object = e, .name = .{ .name = f.name, .loc = e.loc } } } };
        try put(g, out, .{ .name = .{ .name = f.name, .loc = e.loc }, .value = member });
    }
}

/// `a`, `a.b.c`: cheap to read once per field.
fn path(e: *const ast.Expr) bool {
    return switch (e.data) {
        .ident => true,
        .member => |m| !m.optional and path(m.object),
        else => false,
    };
}
