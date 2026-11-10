//! Intern compound types per compilation; named declarations retain their source identity.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const text = @import("zig_text.zig");
const Type = types.Type;
const Generator = core.Generator;
const Error = core.Error;

pub const Field = struct {
    name: []const u8,
    ty: Type,
    optional: bool = false,
    /// The Zig field when it differs from the TSX name (`colorPrimary` →
    /// `color_primary` on the theme).
    zig: ?[]const u8 = null,
};

/// The Zig spelling of field `name` of `ty` (its TSX name unless aliased).
pub fn fieldZig(g: *Generator, ty: Type, name: []const u8) ?[]const u8 {
    const entry = info(g, ty) orelse return null;
    for (entry.fields) |f| if (text.eq(f.name, name)) return f.zig;
    return null;
}

/// `zant.tokens.Theme` as an object type with camelCase fields (`useTheme()`).
pub fn themeType(g: *Generator) Error!Type {
    for (g.compounds.items, 0..) |entry, i| if (text.eq(entry.zig, "zant.tokens.Theme")) {
        return @fromBackingInt(@intCast(types.compound_base + i));
    };
    var fields: std.ArrayList(Field) = .empty;
    for (@import("../tags.zig").theme_fields) |f| {
        const t: Type = if (f.kind == .number) .f32 else .u32;
        try fields.append(g.a, .{ .name = f.name, .ty = t, .zig = f.field });
    }
    return add(g, .{ .kind = .object, .zig = "zant.tokens.Theme", .fields = fields.items });
}
pub const Info = struct {
    kind: enum { array, object, optional, enumeration, callback },
    zig: []const u8,
    child: Type = .void_,
    fields: []const Field = &.{},
    members: []const []const u8 = &.{},
    declaration: ?[]const u8 = null,
    /// `.callback`: the parameter types (callbacks return nothing).
    params: []const Type = &.{},
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
        .enumeration, .callback => false,
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
        .func => |f| @import("components.zig").callbackType(g, ref.loc, f),
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
    if (closestField(entry.fields, name)) |near| {
        return core.fail(g, loc, "{s} has no field `{s}`; did you mean `{s}`?", .{ ty.zig(g), name, near });
    }
    return core.fail(g, loc, "{s} has no field `{s}`", .{ ty.zig(g), name });
}

/// The field whose name is within a couple of edits of `name` (typos).
fn closestField(fields_: []const Field, name: []const u8) ?[]const u8 {
    var best: ?[]const u8 = null;
    var best_distance: usize = 3;
    for (fields_) |f| {
        const d = editDistance(f.name, name);
        if (d < best_distance) {
            best = f.name;
            best_distance = d;
        }
    }
    return best;
}

/// Levenshtein distance for short identifiers (longer ones count as far).
fn editDistance(a: []const u8, b: []const u8) usize {
    if (a.len > 64 or b.len > 64) return std.math.maxInt(usize);
    var row: [65]usize = undefined;
    for (0..b.len + 1) |j| row[j] = j;
    for (a, 1..) |ca, i| {
        var diagonal = row[0];
        row[0] = i;
        for (b, 1..) |cb, j| {
            const above = row[j];
            row[j] = @min(@min(row[j] + 1, row[j - 1] + 1), diagonal + @intFromBool(ca != cb));
            diagonal = above;
        }
    }
    return row[b.len];
}
