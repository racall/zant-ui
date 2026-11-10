//! Native modules: a TS spec (`native/clips.ts`) declares what Zig code
//! (`native/clips.zig`) gives the app, like a React Native TurboModule spec:
//!
//!     import type { i64 } from "zant";
//!     export interface ListEntry { row: i64; preview: string }
//!     export declare function entries(query: string): ListEntry[];
//!
//! The spec compiles to `_spec_clips.zig` (its types, and `check`, which
//! compares the implementation with the declared signatures at compile
//! time) in the `spec` module; TSX imports it as ES modules:
//!
//!     import { entries, type ListEntry } from "native/clips";
//!
//! Types become `@import("spec").clips.ListEntry` (the same types the Zig
//! side uses), functions direct calls of `@import("native").clips.entries`.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const imports = @import("imports.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Error = core.Error;

/// `native/clips` → `clips` (null for other sources).
pub fn moduleName(source: []const u8) ?[]const u8 {
    const prefix = "native/";
    if (!std.mem.startsWith(u8, source, prefix)) return null;
    const name = source[prefix.len..];
    if (name.len == 0 or std.mem.indexOfAny(u8, name, "/\\.") != null) return null;
    return name;
}

pub fn find(project: []const imports.NamedModule, name: []const u8) ?ast.Module {
    for (project) |m| if (m.native and text.eq(m.name, name)) return m.module;
    return null;
}

/// The declarations of the spec `name`, under the alias this file reaches
/// it by: `@import("spec").clips` in the app, `@import("_spec_clips.zig")`
/// in another spec.
pub fn scope(g: *Generator, name: []const u8, loc: ast.Loc, source: []const u8) Error!*imports.Scope {
    const target = find(g.project, name) orelse return core.fail(
        g,
        loc,
        "cannot find `{s}`: add native/{s}.ts (the spec) and native/{s}.zig",
        .{ source, name, name },
    );
    const alias = try text.id(g, try text.fmt(g, "_spec_{s}", .{name}));
    const line = if (g.native_spec)
        try text.fmt(g, "const {s} = @import(\"_spec_{s}.zig\");\n", .{ alias, name })
    else
        try text.fmt(g, "const {s} = @import(\"spec\").{s};\n", .{ alias, try text.id(g, name) });
    if (std.mem.indexOf(u8, g.header.items, line) == null) try g.header.appendSlice(g.a, line);
    const result = try g.a.create(imports.Scope);
    result.* = .{ .decls = target.types, .alias = alias, .imports = target.imports, .native = true, .declared = target.declared };
    return result;
}

/// `import { entries, type ListEntry } from "native/clips"`.
pub fn process(g: *Generator, imp: ast.Import, name: []const u8) Error!void {
    if (g.native_spec) return core.fail(g, imp.source_loc, "a spec imports other specs as `./{s}`", .{name});
    const spec = try scope(g, name, imp.source_loc, imp.source);
    for (imp.names) |n| {
        if (exportedType(spec.decls, n.imported.name)) {
            try g.imported_types.append(g.a, .{ .local = n.local.name, .imported = n.imported.name, .scope = spec });
        } else if (declared(spec.declared, n.imported.name)) |f| {
            if (n.type_only or imp.type_only) return core.fail(g, n.imported.loc, "`{s}` is a function", .{f.name});
            try importFunction(g, n, f, spec, name);
        } else {
            return core.fail(g, n.imported.loc, "native/{s}.ts declares no exported `{s}`", .{ name, n.imported.name });
        }
    }
}

/// A relative import inside a spec (`import type { Kind } from "./clips"`):
/// types only.
pub fn processInSpec(g: *Generator, imp: ast.Import) Error!void {
    const name = std.fs.path.stem(imp.source);
    const spec = try scope(g, name, imp.source_loc, imp.source);
    for (imp.names) |n| {
        if (!exportedType(spec.decls, n.imported.name)) return core.fail(
            g,
            n.imported.loc,
            "native/{s}.ts exports no type `{s}` (specs share types, not functions)",
            .{ name, n.imported.name },
        );
        try g.imported_types.append(g.a, .{ .local = n.local.name, .imported = n.imported.name, .scope = spec });
    }
}

fn exportedType(decls: []const ast.TypeDecl, name: []const u8) bool {
    for (decls) |t| if (t.exported and text.eq(t.name.name, name)) return true;
    return false;
}

fn declared(list: []const ast.TopFunction, name: []const u8) ?*ast.Function {
    for (list) |top| if (top.exported and text.eq(top.func.name, name)) return top.func;
    return null;
}

/// A declared function called directly in the native module.
fn importFunction(g: *Generator, n: ast.ImportName, f: *ast.Function, spec: *const imports.Scope, name: []const u8) Error!void {
    const alias = try text.id(g, try text.fmt(g, "_native_{s}", .{name}));
    const line = try text.fmt(g, "const {s} = @import(\"native\").{s};\n", .{ alias, try text.id(g, name) });
    if (std.mem.indexOf(u8, g.header.items, line) == null) try g.header.appendSlice(g.a, line);
    const previous = g.type_scope;
    g.type_scope = spec;
    defer g.type_scope = previous;
    const zig = try text.fmt(g, "{s}.{s}", .{ alias, try text.id(g, f.name) });
    var function = try @import("functions.zig").signature(g, f, zig, true);
    function.name = n.local.name;
    try g.functions.append(g.a, function);
}

/// A spec has types and declared functions only: the code is Zig's.
pub fn validate(g: *Generator, module: ast.Module, name: []const u8) Error!void {
    if (module.functions.len > 0) return core.fail(
        g,
        module.functions[0].func.loc,
        "a spec declares functions (`export declare function {s}(…): …;`); write them in native/{s}.zig",
        .{ module.functions[0].func.name, name },
    );
    if (module.consts.len > 0) return core.fail(
        g,
        module.consts[0].name.loc,
        "a spec has types and declared functions only; constants belong in native/{s}.zig",
        .{name},
    );
    for (module.declared) |top| if (!top.exported) return core.fail(
        g,
        top.func.loc,
        "export the declared function: `export declare function {s}`",
        .{top.func.name},
    );
    // Zig names every type the implementation uses: no inline objects or enums
    for (module.types) |decl| if (decl.ty.detail) |detail| switch (detail.*) {
        .object => |fields| for (fields) |field| try requireNamed(g, field.ty),
        .union_ => |members| for (members) |member| if (member.detail) |d| switch (d.*) {
            .string_lit, .null_ => {},
            else => try requireNamed(g, member),
        },
        else => try requireNamed(g, decl.ty),
    };
    for (module.declared) |top| {
        for (top.func.params) |p| if (p.ty) |ty| try requireNamed(g, ty);
        if (top.func.ret) |ty| try requireNamed(g, ty);
    }
}

/// `ty` names its object and string-union types (arrays and `| null` of named ones are fine).
fn requireNamed(g: *Generator, ty: ast.TypeRef) Error!void {
    const detail = ty.detail orelse return;
    switch (detail.*) {
        .array => |item| try requireNamed(g, item),
        .union_ => |members| {
            var others: usize = 0;
            for (members) |member| {
                if (member.detail != null and member.detail.?.* == .null_) continue;
                others += 1;
                try requireNamed(g, member);
            }
            if (others > 1) return unnamed(g, ty, "a union");
        },
        .string_lit => return unnamed(g, ty, "a union"),
        .object => return unnamed(g, ty, "an object type"),
        .func => return core.fail(g, ty.loc, "native functions take and return data, not callbacks", .{}),
        .null_ => {},
    }
}

fn unnamed(g: *Generator, ty: ast.TypeRef, what: []const u8) Error {
    return core.fail(g, ty.loc, "name {s} the Zig side uses: `export type Name = …;` / `export interface Name {{ … }}`", .{what});
}

/// `pub fn check(comptime Impl: type) void`: each declared function must be
/// a `pub fn` of the implementation with exactly the spec's signature.
pub fn check(g: *Generator, module: ast.Module, name: []const u8) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    try out.appendSlice(g.a, try text.fmt(g,
        \\/// The spec's file, for messages.
        \\pub const spec_file = "native/{s}.ts";
        \\
        \\/// Compile errors unless `Impl` (native/{s}.zig) implements every declared function.
        \\pub fn check(comptime Impl: type) void {{
        \\
    , .{ name, name }));
    if (module.declared.len == 0) try out.appendSlice(g.a, "_ = Impl;\n");
    for (module.declared) |top| {
        const f = try @import("functions.zig").signature(g, top.func, try text.id(g, top.func.name), false);
        var params: std.ArrayList([]const u8) = .empty;
        var named: std.ArrayList([]const u8) = .empty;
        for (top.func.params, f.params) |p, ty| {
            try params.append(g.a, ty.zig(g));
            try named.append(g.a, try text.fmt(g, "{s}: {s}", .{ try text.id(g, p.name.name), ty.zig(g) }));
        }
        const fn_type = try text.fmt(g, "fn ({s}) {s}", .{ try std.mem.join(g.a, ", ", params.items), f.ret.zig(g) });
        const signature = try text.fmt(g, "pub fn {s}({s}) {s}", .{
            try text.id(g, top.func.name), try std.mem.join(g.a, ", ", named.items), f.ret.zig(g),
        });
        try out.appendSlice(g.a, try text.fmt(g, "tsx.native.expect(Impl, \"{s}\", {s}, {s}, {s});\n", .{
            name, try text.quote(g, top.func.name), fn_type, try text.quote(g, signature),
        }));
    }
    try out.appendSlice(g.a, "}\n");
    return out.items;
}
