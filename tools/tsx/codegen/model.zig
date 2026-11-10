//! Intern compound types per compilation; named declarations retain their source identity.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const text = @import("zig_text.zig");
const Type = types.Type;
const Generator = core.Generator;
const Error = core.Error;

pub const Field = struct { name: []const u8, ty: Type, optional: bool = false };
pub const Info = struct {
    kind: enum { array, object, optional, enumeration },
    zig: []const u8,
    child: Type = .void_,
    fields: []const Field = &.{},
    members: []const []const u8 = &.{},
    declaration: ?[]const u8 = null,
};

pub fn info(g: *Generator, ty: Type) ?*Info {
    const id = @backingInt(ty);
    return if (id >= types.compound_base) &g.compounds.items[id - types.compound_base] else null;
}

pub fn add(g: *Generator, value: Info) Error!Type {
    const ty: Type = @fromBackingInt(@intCast(types.compound_base + g.compounds.items.len));
    try g.compounds.append(g.a, value);
    return ty;
}

pub fn array(g: *Generator, child: Type) Error!Type {
    for (g.compounds.items, 0..) |entry, i| {
        if (entry.kind == .array and entry.child == child) return @fromBackingInt(@intCast(types.compound_base + i));
    }
    return add(g, .{ .kind = .array, .child = child, .zig = try text.fmt(g, "[]const {s}", .{child.zig(g)}) });
}

pub fn optional(g: *Generator, child: Type) Error!Type {
    if (child == .null_) return child;
    if (child == .node) return .optional_node;
    if (optionalChild(g, child) != null) return child;
    for (g.compounds.items, 0..) |entry, i| {
        if (entry.kind == .optional and entry.child == child) return @fromBackingInt(@intCast(types.compound_base + i));
    }
    return add(g, .{ .kind = .optional, .child = child, .zig = try text.fmt(g, "?{s}", .{child.zig(g)}) });
}

pub fn optionalChild(g: *Generator, ty: Type) ?Type {
    if (ty == .optional_node) return .node;
    const entry = info(g, ty) orelse return null;
    return if (entry.kind == .optional) entry.child else null;
}

pub fn element(g: *Generator, ty: Type) ?Type {
    const entry = info(g, ty) orelse return null;
    return if (entry.kind == .array) entry.child else null;
}

pub fn owned(g: *Generator, ty: Type) bool {
    const entry = info(g, ty) orelse return false;
    return switch (entry.kind) {
        .array, .object => true,
        .optional => owned(g, entry.child) or entry.child == .string,
        .enumeration => false,
    };
}

pub fn enumType(g: *Generator, ty: Type) bool {
    const entry = info(g, ty) orelse return false;
    return entry.kind == .enumeration;
}

/// Resolve nullable unions, immutable arrays, nested objects, and literal enums.
pub fn resolve(g: *Generator, ref: ast.TypeRef, name: ?[]const u8) Error!Type {
    const detail = ref.detail orelse return types.typeRef(g, ref, false);
    return switch (detail.*) {
        .array => |child| array(g, try types.typeRef(g, child, false)),
        .object => |fields| object(g, fields, name),
        .union_ => |members| unionType(g, ref.loc, members, name),
        .string_lit => |member| enumeration(g, &.{member}, name),
        .null_ => .null_,
        .func => core.fail(g, ref.loc, "function types arrive with child components in round 3", .{}),
    };
}

fn object(g: *Generator, fields: []const ast.TypeField, name: ?[]const u8) Error!Type {
    const zig = if (name) |n| try text.id(g, n) else try text.fmt(g, "_Object{d}", .{g.compounds.items.len});
    const ty = try add(g, .{ .kind = .object, .zig = zig });
    if (name) |n| try g.type_aliases.append(g.a, .{ .name = n, .ty = ty });
    var result: std.ArrayList(Field) = .empty;
    var code: std.ArrayList(u8) = .empty;
    for (fields) |field| {
        for (result.items) |previous| if (text.eq(previous.name, field.name.name)) {
            return core.fail(g, field.name.loc, "duplicate field `{s}`", .{field.name.name});
        };
        var ft = try types.typeRef(g, field.ty, false);
        if (field.optional) ft = try optional(g, ft);
        try result.append(g.a, .{ .name = field.name.name, .ty = ft, .optional = field.optional });
        try code.appendSlice(g.a, try text.fmt(g, "{s}: {s}{s},\n", .{
            try text.id(g, field.name.name),
            ft.zig(g),
            if (field.optional) " = null" else "",
        }));
    }
    const declaration = try text.fmt(g, "pub const {s} = struct {{\n{s}}};\n\n", .{ zig, code.items });
    info(g, ty).?.fields = result.items;
    info(g, ty).?.declaration = declaration;
    return ty;
}

fn unionType(g: *Generator, loc: ast.Loc, members: []const ast.TypeRef, name: ?[]const u8) Error!Type {
    var nullable = false;
    var nonnull: std.ArrayList(ast.TypeRef) = .empty;
    for (members) |member| {
        if (member.detail != null and member.detail.?.* == .null_) {
            nullable = true;
        } else try nonnull.append(g.a, member);
    }
    if (nonnull.items.len == 0) return .null_;
    var ty: Type = undefined;
    if (nonnull.items.len == 1) {
        ty = if (nonnull.items[0].detail != null)
            try resolve(g, nonnull.items[0], name)
        else
            try types.typeRef(g, nonnull.items[0], false);
    } else {
        var names: std.ArrayList([]const u8) = .empty;
        for (nonnull.items) |member| {
            if (member.detail == null or member.detail.?.* != .string_lit) {
                return core.fail(g, loc, "unions must contain string literals or one type and null", .{});
            }
            try names.append(g.a, member.detail.?.string_lit);
        }
        ty = try enumeration(g, names.items, name);
    }
    return if (nullable) optional(g, ty) else ty;
}

fn enumeration(g: *Generator, members: []const []const u8, name: ?[]const u8) Error!Type {
    const zig = if (name) |n| try text.id(g, n) else try text.fmt(g, "_Enum{d}", .{g.compounds.items.len});
    var names: std.ArrayList([]const u8) = .empty;
    for (members) |member| {
        var duplicate = false;
        for (names.items) |previous| if (text.eq(previous, member)) {
            duplicate = true;
        };
        if (!duplicate) try names.append(g.a, member);
    }
    var fields: std.ArrayList([]const u8) = .empty;
    for (names.items) |member| try fields.append(g.a, try text.id(g, member));
    return add(g, .{
        .kind = .enumeration,
        .zig = zig,
        .members = names.items,
        .declaration = try text.fmt(g, "pub const {s} = enum {{ {s}, }};\n\n", .{
            zig, try std.mem.join(g.a, ", ", fields.items),
        }),
    });
}

/// Lazily resolve named declarations, permitting references to later declarations.
pub fn named(g: *Generator, name: []const u8) Error!?Type {
    for (g.type_aliases.items) |alias| if (text.eq(name, alias.name)) return alias.ty;
    for (g.type_decls, 0..) |decl, i| {
        if (!text.eq(name, decl.name.name)) continue;
        if (g.resolving[i]) return core.fail(g, decl.name.loc, "recursive types are not supported yet", .{});
        g.resolving[i] = true;
        defer g.resolving[i] = false;
        const ty = try resolve(g, decl.ty, name);
        if (decl.ty.detail) |detail| try g.resolved_types.append(g.a, .{ .detail = detail, .ty = ty });
        for (g.type_aliases.items) |alias| if (text.eq(alias.name, name)) return ty;
        try g.type_aliases.append(g.a, .{ .name = name, .ty = ty });
        return ty;
    }
    return null;
}

pub fn fieldType(g: *Generator, ty: Type, name: []const u8, loc: ast.Loc) Error!Type {
    const entry = info(g, ty) orelse return core.fail(g, loc, "{s} has no field `{s}`", .{ ty.zig(g), name });
    if (entry.kind == .object) for (entry.fields) |f| {
        if (text.eq(f.name, name)) return f.ty;
    };
    return core.fail(g, loc, "{s} has no field `{s}`", .{ ty.zig(g), name });
}
