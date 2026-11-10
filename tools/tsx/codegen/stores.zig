//! Shared state across components and files (Zustand style, `zant.store`
//! in docs/tsx.md):
//!
//!     export const settings = createStore<Settings>({ dark: false, size: 14 });
//!     const dark = useStore(settings, (s) => s.dark);   // in a component
//!     settings.update({ dark: true });                  // in a handler
//!
//! A store is a file-level `pub var settings: tsx.Store(Settings)`; other
//! files import it like a component (`_store.settings`). Changing it rebuilds
//! every window.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const callbacks = @import("callbacks.zig");
const imports = @import("imports.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

pub const Store = struct {
    /// The TSX name in this file.
    name: []const u8,
    /// Zig path: `settings` or `_store.settings`.
    zig: []const u8,
    ty: Type,
};

fn isCreate(e: *const ast.Expr) bool {
    return e.data == .call and e.data.call.callee.data == .ident and
        text.eq(e.data.call.callee.data.ident, "createStore");
}

/// The file-level constants that are not stores (for `globals.collect`).
pub fn plainConstants(g: *Generator, constants: []const ast.TopConst) Error![]const ast.TopConst {
    var plain: std.ArrayList(ast.TopConst) = .empty;
    for (constants) |constant| {
        if (isCreate(constant.value) or @import("windows.zig").isCreate(constant.value)) continue;
        try plain.append(g.a, constant);
    }
    return plain.items;
}

/// `createStore<T>(initial)` constants → `pub var` declarations.
pub fn collect(g: *Generator, constants: []const ast.TopConst) Error![]const u8 {
    var code: std.ArrayList(u8) = .empty;
    for (constants) |constant| {
        if (!isCreate(constant.value)) continue;
        const call = constant.value.data.call;
        if (call.type_args.len != 1 or call.args.len != 1) return core.fail(
            g,
            constant.value.loc,
            "createStore takes the state type and its initial value: createStore<Settings>({{ … }})",
            .{},
        );
        if (scope.globalName(g, constant.name.name)) {
            return core.fail(g, constant.name.loc, "duplicate or reserved name `{s}`", .{constant.name.name});
        }
        const ty = try types.typeRef(g, call.type_args[0], false);
        var c = Context{ .g = g, .render = true, .ret = .void_, .constant = true };
        var initial = try expr.emit(g, &c, call.args[0], ty, false);
        initial = try types.coerce(g, initial, ty, call.args[0].loc);
        const name = try text.id(g, constant.name.name);
        const declaration = "{s}var {s}: tsx.Store({s}) = .{{ .state = .{{ .initial = {s} }} }};\n\n";
        try code.appendSlice(g.a, try text.fmt(g, declaration, .{
            if (constant.exported) "pub " else "", name, ty.zig(g), initial.code,
        }));
        try g.stores.append(g.a, .{ .name = constant.name.name, .zig = name, .ty = ty });
    }
    return code.items;
}

/// The store a file exports as `name`, if any.
pub fn exported(module: ast.Module, name: []const u8) ?ast.TopConst {
    for (module.consts) |constant| {
        if (constant.exported and isCreate(constant.value) and text.eq(constant.name.name, name)) return constant;
    }
    return null;
}

/// `import { settings } from "./store"`: its type resolves in that file.
pub fn import(g: *Generator, n: ast.ImportName, constant: ast.TopConst, from: *const imports.Scope) Error!void {
    const call = constant.value.data.call;
    if (call.type_args.len != 1) return core.fail(g, n.imported.loc, "`{s}` needs a type argument", .{n.imported.name});
    const previous = g.type_scope;
    g.type_scope = from;
    defer g.type_scope = previous;
    const ty = try types.typeRef(g, call.type_args[0], false);
    try g.stores.append(g.a, .{
        .name = n.local.name,
        .zig = try text.fmt(g, "{s}.{s}", .{ try text.id(g, from.alias), try text.id(g, n.imported.name) }),
        .ty = ty,
    });
}

fn find(g: *Generator, c: *Context, e: *const ast.Expr) ?Store {
    if (e.data != .ident or scope.local(c, e.data.ident) != null) return null;
    for (g.stores.items) |s| if (text.eq(s.name, e.data.ident)) return s;
    return null;
}

/// `const dark = useStore(settings, (s) => s.dark)` as a render local: the
/// store's value (or the selected part) at render time, like a state value.
pub fn use(g: *Generator, c: *Context, loc: ast.Loc, d: ast.Decl, top: bool) Error![]const u8 {
    const call = d.init.?.data.call;
    if (!c.render or !top) return core.fail(g, loc, "hooks must be called at the top level of the component", .{});
    if (d.pattern != .ident) return core.fail(g, loc, "const value = useStore(store)", .{});
    if (call.args.len < 1 or call.args.len > 2) {
        return core.fail(g, loc, "useStore takes a store and an optional selector: useStore(store, (s) => s.x)", .{});
    }
    const store = find(g, c, call.args[0]) orelse {
        return core.fail(g, call.args[0].loc, "useStore needs a store made with createStore", .{});
    };
    // retired values from earlier changes are freed once a new frame reads it
    const current = try text.fmt(g, "{s}.read()", .{store.zig});
    const value: Value = if (call.args.len == 2)
        try callbacks.emit(g, c, call.args[1], &.{.{ .ty = store.ty, .code = current }}, null)
    else
        .{ .ty = store.ty, .code = current };
    const b = try scope.bind(c, d.pattern.ident.name, value.ty, false);
    b.init = value.code;
    return text.fmt(g, "\x01{d}\x02", .{b.serial});
}

/// `settings.get()`, `settings.set(v)`, `settings.update({ … })`; null when the
/// callee is not a store method.
pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    if (call.callee.data != .member) return null;
    const m = call.callee.data.member;
    const store = find(g, c, m.object) orelse return null;
    const method = m.name.name;
    if (text.eq(method, "get")) {
        if (call.args.len != 0) return core.fail(g, loc, "get() takes no arguments", .{});
        return .{ .ty = store.ty, .code = try text.fmt(g, "{s}.get()", .{store.zig}) };
    }
    const changes = text.eq(method, "set") or text.eq(method, "update");
    if (!changes) return core.fail(g, m.name.loc, "a store has get(), set(value) and update({{ … }})", .{});
    if (c.render) return core.fail(g, loc, "stores change in event handlers and effects, not while rendering", .{});
    if (call.args.len != 1) return core.fail(g, loc, "{s}() takes one argument", .{method});
    if (text.eq(method, "set")) {
        const v = try expr.emit(g, c, call.args[0], store.ty, false);
        return .{ .ty = .void_, .code = try text.fmt(g, "{s}.set({s})", .{ store.zig, v.code }) };
    }
    return .{ .ty = .void_, .code = try update(g, c, store, call.args[0]) };
}

/// `update({ dark: true })`: a shallow merge of the listed fields.
fn update(g: *Generator, c: *Context, store: Store, arg: *ast.Expr) Error![]const u8 {
    if (arg.data != .object) {
        return core.fail(g, arg.loc, "update takes an object literal with the fields to change", .{});
    }
    var fields: std.ArrayList(u8) = .empty;
    for (arg.data.object) |p| {
        if (p.spread) return core.fail(g, p.value.loc, "list the fields to change (no spread)", .{});
        const ty = try model.fieldType(g, store.ty, p.key.name, p.key.loc);
        const v = try expr.emit(g, c, p.value, ty, false);
        const field = model.fieldZig(g, store.ty, p.key.name) orelse try text.id(g, p.key.name);
        // the patch is generic: a literal (`cond ? "a" : "b"`) needs its type spelled out
        const code = if (v.inference == .known) v.code else try text.fmt(g, "@as({s}, {s})", .{ ty.zig(g), v.code });
        try fields.appendSlice(g.a, try text.fmt(g, ".{s} = {s},\n", .{ field, code }));
    }
    return text.fmt(g, "{s}.update(.{{\n{s}}})", .{ store.zig, fields.items });
}
