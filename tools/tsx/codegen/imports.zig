//! Imports between `.tsx` files of one project (`import { TodoRow, type Todo }
//! from "./TodoRow"`). Every file compiles to its own Zig file in one output
//! directory, so an import becomes
//!
//!     const _TodoRow = @import("TodoRow.zig");
//!
//! Imported types resolve from the other file's declarations (types they
//! reference are looked up there too) under Zig names like `_TodoRow.Todo`,
//! with no declaration in this file; imported components call
//! `_TodoRow.todoRow(...)`.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const components = @import("components.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Error = core.Error;

/// A parsed file of the project, named by its file stem (`TodoRow`).
pub const NamedModule = struct {
    name: []const u8,
    module: ast.Module,
};

/// The declarations of another file and the Zig name it is imported under.
pub const Scope = struct {
    decls: []const ast.TypeDecl,
    alias: []const u8,
};

/// A type imported by name into this file.
pub const ImportedType = struct {
    local: []const u8,
    imported: []const u8,
    scope: *const Scope,
};

pub fn isRelative(source: []const u8) bool {
    return std.mem.startsWith(u8, source, "./") or std.mem.startsWith(u8, source, "../");
}

/// `./components/TodoRow.tsx` → `TodoRow`.
fn stem(source: []const u8) []const u8 {
    const base = std.fs.path.basename(source);
    return if (std.mem.endsWith(u8, base, ".tsx")) base[0 .. base.len - 4] else base;
}

/// The project file a relative import names (`./components/TodoRow` → `TodoRow`).
pub fn findModule(project: []const NamedModule, source: []const u8) ?ast.Module {
    const name = stem(source);
    for (project) |m| if (zig_text.eq(m.name, name)) return m.module;
    return null;
}

/// Handles the relative imports of `module`: registers imported types and
/// components and returns the `@import` lines for the file header.
pub fn process(g: *Generator, module: ast.Module, project: []const NamedModule) Error![]const u8 {
    var header: std.ArrayList(u8) = .empty;
    for (module.imports) |imp| {
        if (!isRelative(imp.source)) continue;
        const name = stem(imp.source);
        const target = findModule(project, imp.source) orelse {
            return core.fail(g, imp.source_loc, "cannot find `{s}` among the project's .tsx files", .{imp.source});
        };
        const alias = try zig_text.fmt(g, "_{s}", .{name});
        const scope = try g.a.create(Scope);
        scope.* = .{ .decls = target.types, .alias = alias };
        try header.appendSlice(g.a, try zig_text.fmt(g, "const {s} = @import(\"{s}.zig\");\n", .{
            try zig_text.id(g, alias), name,
        }));
        for (imp.names) |n| {
            if (exportedType(target, n.imported.name)) {
                try g.imported_types.append(g.a, .{
                    .local = n.local.name,
                    .imported = n.imported.name,
                    .scope = scope,
                });
            } else if (exportedFunction(target, n.imported.name)) |f| {
                try importComponent(g, n, f, scope, @import("stateful.zig").stateful(project, target, f));
            } else {
                return core.fail(g, n.imported.loc, "`{s}` is not exported by `{s}`", .{ n.imported.name, imp.source });
            }
        }
    }
    return header.items;
}

fn exportedType(module: ast.Module, name: []const u8) bool {
    for (module.types) |t| if (t.exported and zig_text.eq(t.name.name, name)) return true;
    return false;
}

fn exportedFunction(module: ast.Module, name: []const u8) ?*ast.Function {
    for (module.functions) |top| if (top.exported and zig_text.eq(top.func.name, name)) return top.func;
    return null;
}

/// An imported component: its props type resolves in the declaring file.
fn importComponent(
    g: *Generator,
    n: ast.ImportName,
    f: *ast.Function,
    scope: *const Scope,
    stateful: bool,
) Error!void {
    var comp = components.Component{
        .name = n.local.name,
        .builder = try zig_text.fmt(g, "{s}.{s}", .{
            try zig_text.id(g, scope.alias),
            try components.builderName(g, f.name),
        }),
        .func = f,
        .props = null,
        .external = true,
        .stateful = stateful,
    };
    if (f.params.len == 1) {
        const ref = f.params[0].ty orelse return core.fail(g, n.imported.loc, "`{s}` has untyped props", .{f.name});
        const previous = g.type_scope;
        g.type_scope = scope;
        defer g.type_scope = previous;
        comp.props = try types.typeRef(g, ref, false);
    }
    try g.components.append(g.a, comp);
}

/// A type name used in this file (`imported`) or inside another file's
/// declarations (`g.type_scope`). Null when neither knows it.
pub fn named(g: *Generator, name: []const u8) Error!?Type {
    if (g.type_scope) |scope| return resolve(g, scope, name);
    for (g.imported_types.items) |t| if (zig_text.eq(t.local, name)) {
        return (try resolve(g, t.scope, t.imported)) orelse
            core.fail(g, 0, "`{s}` is not declared in `{s}`", .{ t.imported, t.scope.alias });
    };
    return null;
}

fn resolve(g: *Generator, scope: *const Scope, name: []const u8) Error!?Type {
    const key = try zig_text.fmt(g, "{s}.{s}", .{ scope.alias, name });
    for (g.type_aliases.items) |alias| if (zig_text.eq(alias.name, key)) return alias.ty;
    for (scope.decls) |decl| {
        if (!zig_text.eq(decl.name.name, name)) continue;
        const previous = g.type_scope;
        g.type_scope = scope;
        defer g.type_scope = previous;
        const ty = try model.resolve(g, decl.ty, name);
        for (g.type_aliases.items) |alias| if (zig_text.eq(alias.name, key)) return ty;
        try g.type_aliases.append(g.a, .{ .name = key, .ty = ty });
        return ty;
    }
    return null;
}

/// The Zig name and alias key of a named compound declared in the current
/// type scope (`_TodoRow.Todo`), or null for this file's own types.
pub fn externalName(g: *Generator, name: []const u8) Error!?[]const u8 {
    const scope = g.type_scope orelse return null;
    return try zig_text.fmt(g, "{s}.{s}", .{ try zig_text.id(g, scope.alias), try zig_text.id(g, name) });
}
