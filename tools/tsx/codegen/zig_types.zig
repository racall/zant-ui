//! Types of hand-written Zig files used from TSX (`import type { Peer } from
//! "./sync.zig"`): a `pub const X = struct { … }` becomes an object type and
//! a `pub const K = enum { … }` a string-literal union, both under their Zig
//! names (`_sync.Peer`) with no declaration in the generated file. Function
//! signatures of the same file may use them, also inside slices and
//! optionals (`pub fn peers() []const Peer`).
//!
//! Fields of types TSX can't represent (pointers, mutexes, …) are left out:
//! TSX reads the others; building such a struct in TSX is a Zig error.
const std = @import("std");
const Ast = std.zig.Ast;
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Error = core.Error;

/// A Zig file and the alias the generated file imports it under (`_sync`).
pub const File = struct {
    tree: *const Ast,
    alias: []const u8,
};

/// A type imported by name into this file.
pub const Imported = struct {
    local: []const u8,
    ty: Type,
};

/// `local` names an imported Zig type of this file.
pub fn named(g: *Generator, local: []const u8) ?Type {
    for (g.zig_types.items) |t| if (text.eq(t.local, local)) return t.ty;
    return null;
}

/// The node of `pub const <name> = struct / enum { … }`, if any.
fn declaration(tree: *const Ast, name: []const u8) ?Ast.Node.Index {
    for (tree.rootDecls()) |decl| {
        const v = tree.fullVarDecl(decl) orelse continue;
        if (v.visib_token == null) continue;
        if (!text.eq(tree.tokenSlice(v.ast.mut_token + 1), name)) continue;
        const init = v.ast.init_node.unwrap() orelse return null;
        var buffer: [2]Ast.Node.Index = undefined;
        if (tree.fullContainerDecl(&buffer, init) == null) return null;
        return init;
    }
    return null;
}

/// The TSX type of the Zig type `name` declared in `file`, or null when it
/// isn't a pub struct / enum there.
pub fn resolve(g: *Generator, file: File, name: []const u8) Error!?Type {
    const zig = try text.fmt(g, "{s}.{s}", .{ file.alias, try text.id(g, name) });
    for (g.type_aliases.items) |alias| if (text.eq(alias.name, zig)) return alias.ty;
    const node = declaration(file.tree, name) orelse return null;
    var buffer: [2]Ast.Node.Index = undefined;
    const container = file.tree.fullContainerDecl(&buffer, node).?;
    const keyword = file.tree.tokenTag(container.ast.main_token);
    if (keyword == .keyword_enum) return try enumeration(g, file, container, zig);
    if (keyword != .keyword_struct) return null;
    // registered before its fields, so a field may refer back to it
    const ty = try model.add(g, .{ .kind = .object, .zig = zig });
    try g.type_aliases.append(g.a, .{ .name = zig, .ty = ty });
    var fields: std.ArrayList(model.Field) = .empty;
    for (container.ast.members) |member| {
        const field = file.tree.fullContainerField(member) orelse continue;
        const type_node = field.ast.type_expr.unwrap() orelse continue;
        const field_ty = (try mapIn(g, file.tree, file, type_node, false)) orelse continue;
        try fields.append(g.a, .{ .name = file.tree.tokenSlice(field.ast.main_token), .ty = field_ty });
    }
    model.info(g, ty).?.fields = fields.items;
    return ty;
}

fn enumeration(g: *Generator, file: File, container: Ast.full.ContainerDecl, zig: []const u8) Error!Type {
    var members: std.ArrayList([]const u8) = .empty;
    for (container.ast.members) |member| {
        const field = file.tree.fullContainerField(member) orelse continue;
        try members.append(g.a, file.tree.tokenSlice(field.ast.main_token));
    }
    const ty = try model.add(g, .{ .kind = .enumeration, .zig = zig, .members = members.items });
    try g.type_aliases.append(g.a, .{ .name = zig, .ty = ty });
    return ty;
}

/// A Zig type expression as a TSX type: bool, numbers, `[]const u8`
/// (string), `tsx.Bytes`, pub structs / enums of `file` (when given), and
/// `?T` / `[]const T` of those. Null for anything else.
pub fn mapIn(g: *Generator, tree: *const Ast, file: ?File, node: Ast.Node.Index, allow_void: bool) Error!?Type {
    switch (tree.nodeTag(node)) {
        .identifier => {
            const name = tree.tokenSlice(tree.nodeMainToken(node));
            if (text.eq(name, "bool")) return .boolean;
            if (allow_void and text.eq(name, "void")) return .void_;
            if (types.numericName(name)) return std.meta.stringToEnum(Type, name).?;
            return if (file) |f| try resolve(g, f, name) else null;
        },
        .optional_type => {
            const child = (try mapIn(g, tree, file, tree.nodeData(node).node, false)) orelse return null;
            if (model.optionalChild(g, child) != null) return null;
            return try model.optional(g, child);
        },
        .field_access => {
            // `tsx.Bytes` / `zant.tsx.Bytes`: a Uint8Array
            const source = tree.getNodeSource(node);
            if (text.eq(source, "tsx.Bytes") or text.eq(source, "zant.tsx.Bytes")) return try model.array(g, .u8);
            if (text.eq(source, "zant.image.Image") or text.eq(source, "image.Image")) {
                return try @import("jsx.zig").imageType(g);
            }
            return null;
        },
        else => {},
    }
    const ptr = tree.fullPtrType(node) orelse return null;
    if (ptr.size != .slice or ptr.const_token == null or ptr.allowzero_token != null or
        ptr.volatile_token != null or ptr.ast.sentinel != .none or ptr.ast.align_node != .none or
        ptr.ast.addrspace_node != .none) return null;
    const child = (try mapIn(g, tree, file, ptr.ast.child_type, false)) orelse return null;
    if (child == .u8) return .string;
    return try model.array(g, child);
}
