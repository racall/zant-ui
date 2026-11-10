//! Optional values as conditions, and narrowing them (TS control flow):
//!
//!     const name = peer ? peer.name : "none";      // peer: Peer | null
//!     if (label) { label.length }                  // label: string | null
//!     {peer && <Text>{peer.name}</Text>}
//!     if (!entry) return <Empty/>;  entry.text      // the rest of the block
//!     s.prompt !== null && s.prompt.row == row      // member paths too
//!
//! A condition yields facts "this name is non-null when the condition is
//! true / false"; while a branch is emitted, reading the name gives the
//! child type and `x.?` (only evaluated where the condition holds). Only
//! names that can't change in between narrow: const locals, parameters,
//! props, state (a snapshot in handlers), constants.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Value = types.Value;
const Type = types.Type;
const Error = core.Error;

pub const Narrow = struct {
    /// The name, or a member path (`s.prompt`).
    name: []const u8,
    /// The local it narrows (null: a prop, state or constant), so an inner
    /// local with the same name is not narrowed.
    binding: ?*scope.Binding,
    ty: Type,
    code: []const u8,
};

/// The narrowed value of `n`, if a condition around this point narrowed it.
pub fn find(c: *Context, n: []const u8) ?Value {
    return findPath(c, n, n);
}

/// The narrowed value of the member path `path` (`s.prompt`) whose base
/// name is `base`.
pub fn findPath(c: *Context, member_path: []const u8, base_name: []const u8) ?Value {
    if (c.narrowed.items.len == 0) return null;
    const n = member_path;
    const b = scope.local(c, base_name);
    var i = c.narrowed.items.len;
    while (i > 0) {
        i -= 1;
        const item = c.narrowed.items[i];
        if (text.eq(item.name, n) and item.binding == b) return .{ .ty = item.ty, .code = item.code };
    }
    return null;
}

/// Narrows what `cond` proves when it is `positive`; `restore(c, mark)` ends it.
pub fn push(g: *Generator, c: *Context, cond: *const ast.Expr, positive: bool) Error!usize {
    const mark = c.narrowed.items.len;
    try facts(g, c, cond, positive);
    return mark;
}

pub fn restore(c: *Context, mark: usize) void {
    c.narrowed.shrinkRetainingCapacity(mark);
}

fn facts(g: *Generator, c: *Context, e: *const ast.Expr, positive: bool) Error!void {
    switch (e.data) {
        .ident, .member => if (positive) try candidate(g, c, e),
        .unary => |u| if (text.eq(u.op, "!")) try facts(g, c, u.operand, !positive),
        .binary => |b| {
            if (text.eq(b.op, "&&") and positive or text.eq(b.op, "||") and !positive) {
                try facts(g, c, b.lhs, positive);
                try facts(g, c, b.rhs, positive);
                return;
            }
            const not_equal = text.eq(b.op, "!=") or text.eq(b.op, "!==");
            const equal = text.eq(b.op, "==") or text.eq(b.op, "===");
            if (!(not_equal and positive) and !(equal and !positive)) return;
            const other = if (isNull(b.rhs)) b.lhs else if (isNull(b.lhs)) b.rhs else return;
            try candidate(g, c, other);
        },
        else => {},
    }
}

fn isNull(e: *const ast.Expr) bool {
    return e.data == .null_lit or (e.data == .ident and text.eq(e.data.ident, "undefined"));
}

/// `peer` → "peer", `s.prompt` → "s.prompt" (plain member chains only).
pub fn path(g: *Generator, e: *const ast.Expr) Error!?[]const u8 {
    return switch (e.data) {
        .ident => |n| n,
        .member => |m| blk: {
            if (m.optional) break :blk null;
            const object = (try path(g, m.object)) orelse break :blk null;
            break :blk try text.fmt(g, "{s}.{s}", .{ object, m.name.name });
        },
        else => null,
    };
}

pub fn base(e: *const ast.Expr) []const u8 {
    return switch (e.data) {
        .member => |m| base(m.object),
        .ident => |n| n,
        else => "",
    };
}

fn candidate(g: *Generator, c: *Context, e: *const ast.Expr) Error!void {
    const n = (try path(g, e)) orelse return;
    const b = scope.local(c, base(e));
    if (b) |binding| if (binding.mutable) return;
    const value = expr.emit(g, c, @constCast(e), null, false) catch |err| switch (err) {
        // not a name this condition can narrow (an error is reported where it is read)
        error.Tsx => {
            g.diag.* = null;
            return;
        },
        else => return err,
    };
    const child = model.optionalChild(g, value.ty) orelse return;
    try c.narrowed.append(g.a, .{
        .name = n,
        .binding = b,
        .ty = child,
        .code = try text.fmt(g, "{s}.?", .{try text.operand(g, value, .suffix, false)}),
    });
}

/// JS truthiness of an optional: present, and for strings / numbers /
/// booleans also non-empty / non-zero / true.
pub fn truthy(g: *Generator, v: Value) Error!Value {
    const child = model.optionalChild(g, v.ty).?;
    if (child == .boolean) return .{
        .ty = .boolean,
        .code = try text.fmt(g, "{s} orelse false", .{try text.operand(g, v, .logical_or, true)}),
        .precedence = .logical_or,
    };
    if (child != .string and !child.numeric()) return .{
        .ty = .boolean,
        .code = try text.fmt(g, "{s} != null", .{try text.operand(g, v, .comparison, true)}),
        .precedence = .comparison,
    };
    g.counter += 1;
    const name = try text.fmt(g, "_present{d}", .{g.counter});
    const test_ = if (child == .string) try text.fmt(g, "{s}.len > 0", .{name}) else try text.fmt(g, "{s} != 0", .{name});
    return .{
        .ty = .boolean,
        .code = try text.fmt(g, "if ({s}) |{s}| {s} else false", .{ v.code, name, test_ }),
        .precedence = .conditional,
    };
}
