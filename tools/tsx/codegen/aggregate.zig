//! Immutable array/object values, spreads, indexing, and optional member access.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const expr = @import("expr.zig");
const scope = @import("scope.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

/// General arrays copy their elements into the frame arena; initial values stay static.
pub fn array(g: *Generator, c: *Context, loc: ast.Loc, items: []const *ast.Expr, expected: ?Type) Error!Value {
    var child: ?Type = if (expected) |ty| model.element(g, unwrap(g, ty)) else null;
    var values: std.ArrayList(Value) = .empty;
    for (items) |item| {
        const spread = item.data == .spread;
        const context = if (expected != null) child else if (child) |ty|
            (if (ty.numeric() or ty == .null_ or item.data == .null_lit) null else child)
        else
            null;
        const value = try expr.emit(g, c, if (spread) item.data.spread else item, if (spread) null else context, false);
        if (spread and model.optionalChild(g, value.ty) != null) {
            return core.fail(g, item.loc, "optional array needs ?? or ! before spread", .{});
        }
        const element = if (spread)
            model.element(g, value.ty) orelse return core.fail(g, item.loc, "spread needs an array", .{})
        else
            value.ty;
        child = if (child) |ty| try types.unify(g, ty, element, item.loc) else element.concrete();
        try values.append(g.a, value);
    }
    const element = (child orelse return core.fail(g, loc, "empty array needs a type annotation", .{})).concrete();
    const ty = try model.array(g, element);
    var parts: std.ArrayList([]const u8) = .empty;
    var constants: std.ArrayList([]const u8) = .empty;
    var has_spread = false;
    for (items, values.items) |item, raw| {
        if (item.data == .spread) {
            if (model.element(g, raw.ty).? != element) {
                return core.fail(g, item.loc, "spread array element type does not match", .{});
            }
            has_spread = true;
            try parts.append(g.a, raw.code);
        } else {
            const value = try types.coerce(g, raw, element, item.loc);
            try constants.append(g.a, value.code);
            try parts.append(g.a, try text.fmt(g, "&[_]{s}{{ {s}, }}", .{ element.zig(g), value.code }));
        }
    }
    if (c.constant and !has_spread) return .{ .ty = ty, .code = try text.fmt(g, "&[_]{s}{{{s}}}", .{
        element.zig(g), try std.mem.join(g.a, ", ", constants.items),
    }) };
    return .{ .ty = ty, .code = try text.fmt(g, "{s}({s}, {s}{{{s}}})", .{
        if (c.constant) "&tsx.constantConcat" else "tsx.concat",
        element.zig(g),
        if (c.constant) "." else "&.",
        try std.mem.join(g.a, ", ", parts.items),
    }) };
}

/// Copy a spread exactly once, then apply fields in source order (last write wins).
pub fn object(g: *Generator, c: *Context, loc: ast.Loc, properties: []const ast.Property, expected: ?Type) Error!Value {
    const ty = unwrap(g, expected orelse return core.fail(g, loc, "object literal needs a type annotation", .{}));
    const entry = model.info(g, ty) orelse return core.fail(g, loc, "object literal needs a struct type", .{});
    if (entry.kind != .object) return core.fail(g, loc, "object literal needs a struct type", .{});
    const fields = entry.fields;
    var values: std.ArrayList(Value) = .empty;
    var writes: std.ArrayList([]const u8) = .empty;
    var initial: std.ArrayList([]const u8) = .empty;
    const temp = try temporary(c, "_object");
    var spread = false;
    for (properties) |property| {
        const ft = if (property.spread) ty else try model.fieldType(g, ty, property.key.name, property.key.loc);
        const value = try expr.emit(g, c, property.value, ft, false);
        try values.append(g.a, value);
        if (property.spread) {
            spread = true;
            try writes.append(g.a, try text.fmt(g, "{s} = {s};", .{ temp, value.code }));
        } else {
            try writes.append(g.a, try text.fmt(g, "{s}.{s} = {s};", .{
                temp, try text.id(g, property.key.name), value.code,
            }));
        }
    }
    if (spread) return .{ .ty = ty, .code = try text.fmt(g, "{s}: {{\nvar {s}: {s} = undefined;\n{s}\n" ++
        "break :{s} {s};\n}}", .{ temp, temp, ty.zig(g), try std.mem.join(g.a, "\n", writes.items), temp, temp }) };
    for (fields) |f| {
        var present = false;
        for (properties) |property| present = present or text.eq(property.key.name, f.name);
        if (!present and !f.optional) {
            return core.fail(g, loc, "{s} is missing field `{s}`", .{ ty.zig(g), f.name });
        }
    }
    // Source order matters when field values call component functions.
    for (properties, values.items, 0..) |property, value, index_| {
        var replaced = false;
        for (properties[index_ + 1 ..]) |later| replaced = replaced or text.eq(later.key.name, property.key.name);
        if (replaced) continue;
        try initial.append(g.a, try text.fmt(g, ".{s} = {s},", .{ try text.id(g, property.key.name), value.code }));
    }
    return .{ .ty = ty, .code = try text.fmt(g, "{s}{{\n{s}\n}}", .{
        ty.zig(g), try std.mem.join(g.a, "\n", initial.items),
    }) };
}

pub fn unwrap(g: *Generator, ty: Type) Type {
    return model.optionalChild(g, ty) orelse ty;
}

/// Compiler temporaries share the function allocator, so they cannot shadow source locals.
pub fn temporary(c: *Context, base: []const u8) Error![]const u8 {
    const binding = try scope.bind(c, base, .void_, false);
    binding.name = "";
    return binding.zig;
}

pub fn nonNull(g: *Generator, c: *Context, inner: *ast.Expr, expected: ?Type, child: bool) Error!Value {
    if (types.numericLiterals(inner)) return expr.emit(g, c, inner, expected, child);
    const value = try expr.emit(g, c, inner, null, child);
    if (model.optionalChild(g, value.ty)) |ty| return .{
        .ty = ty,
        .code = try text.fmt(g, "{s}.?", .{try text.operand(g, value, .suffix, false)}),
    };
    return if (expected) |target| types.coerce(g, value, target, inner.loc) else value;
}

pub fn coalesce(g: *Generator, c: *Context, loc: ast.Loc, binary: ast.Binary, expected: ?Type) Error!Value {
    const value = try expr.emit(g, c, binary.lhs, null, false);
    if (value.ty == .null_) {
        var fallback = try expr.emit(g, c, binary.rhs, expected, false);
        if (text.eq(value.code, "null")) return fallback;
        fallback = try types.coerce(g, fallback, expected orelse fallback.ty.concrete(), binary.rhs.loc);
        return .{ .ty = fallback.ty, .code = try text.fmt(g, "tsx.nullAs({s}, {s}) orelse {s}", .{
            fallback.ty.zig(g), value.code, try text.operand(g, fallback, .logical_or, true),
        }), .precedence = .logical_or };
    }
    const child = model.optionalChild(g, value.ty) orelse return core.fail(g, loc, "?? needs an optional value", .{});
    const fallback = try expr.emit(g, c, binary.rhs, child, false);
    return .{ .ty = child, .code = try text.fmt(g, "{s} orelse {s}", .{
        try text.operand(g, value, .logical_or, true), try text.operand(g, fallback, .logical_or, true),
    }), .precedence = .logical_or };
}

/// Optional access evaluates its receiver once and preserves null through the member.
pub fn member(g: *Generator, c: *Context, m: ast.Member) Error!Value {
    if (c.narrowed.items.len > 0 and !m.optional) {
        // `s.prompt` narrowed by `s.prompt !== null && …`
        const narrowing = @import("narrowing.zig");
        const self_expr = ast.Expr{ .loc = m.name.loc, .data = .{ .member = m } };
        if (try narrowing.path(g, &self_expr)) |p| if (narrowing.findPath(c, p, narrowing.base(&self_expr))) |v| return v;
    }
    if (try @import("builtins.zig").member(g, c, m)) |value| return value;
    if (try @import("hooks.zig").refRead(g, c, m)) |ref| return ref;
    const value = try expr.emit(g, c, m.object, null, false);
    if (m.optional and model.optionalChild(g, value.ty) != null) {
        const child = model.optionalChild(g, value.ty).?;
        const name = try temporary(c, "_value");
        const result = try memberValue(g, .{ .ty = child, .code = name }, m.name);
        return .{ .ty = try model.optional(g, result.ty), .code = try text.fmt(g, "if ({s}) |{s}| {s} else null", .{
            value.code, name, result.code,
        }), .precedence = .conditional };
    }
    if (model.optionalChild(g, value.ty) != null) {
        return core.fail(g, m.name.loc, "optional value needs ?? or ! before member access", .{});
    }
    return memberValue(g, value, m.name);
}

fn memberValue(g: *Generator, value: Value, name: ast.Name) Error!Value {
    const length = text.eq(name.name, "length") and (value.ty == .string or model.element(g, value.ty) != null);
    const ty = if (length) .usize else try model.fieldType(g, value.ty, name.name, name.loc);
    // strings count UTF-16 code units, as in TypeScript
    if (length and value.ty == .string) return .{ .ty = ty, .code = try text.fmt(g, "tsx.utf16Length({s})", .{value.code}) };
    const field = if (length) "len" else model.fieldZig(g, value.ty, name.name) orelse try text.id(g, name.name);
    return .{ .ty = ty, .code = try text.fmt(g, "{s}.{s}", .{ try text.operand(g, value, .suffix, false), field }) };
}

/// Numeric indexes are checked by tsx.index before Zig's own slice bounds check.
pub fn index(g: *Generator, c: *Context, loc: ast.Loc, ix: ast.Index) Error!Value {
    const value = try expr.emit(g, c, ix.object, null, false);
    const ty = unwrap(g, value.ty);
    if (!ix.optional and ty != value.ty) return core.fail(g, loc, "optional array needs ?? or !", .{});
    const child = model.element(g, ty) orelse return core.fail(g, loc, "indexing needs an array", .{});
    const position = try expr.emit(g, c, ix.index, null, false);
    if (model.optionalChild(g, position.ty) != null) {
        return core.fail(g, ix.index.loc, "optional array index needs ?? or !", .{});
    }
    if (!position.ty.numeric()) return core.fail(g, ix.index.loc, "array index must be numeric", .{});
    const at = try text.fmt(g, "tsx.index({s})", .{(try types.concrete(g, position)).code});
    if (ty != value.ty) {
        const name = try temporary(c, "_array");
        return .{ .ty = try model.optional(g, child), .code = try text.fmt(g, "if ({s}) |{s}| {s}[{s}] else null", .{
            value.code, name, name, at,
        }), .precedence = .conditional };
    }
    return .{ .ty = child, .code = try text.fmt(g, "{s}[{s}]", .{
        try text.operand(g, value, .suffix, false), at,
    }) };
}
