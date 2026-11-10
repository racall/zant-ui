//! Desktop and platform functions imported from "zant" (docs/tsx.md, "桌面函数"):
//!
//!     import { clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos } from "zant";
//!     clipboard.setText(text);  app.quit();  mainWindow.show();
//!     const hint = keyLabel("mod+f");        // "⌘F" / "Ctrl+F"
//!
//! One table maps each TS function to its wrapper in `src/tsx_platform.zig`
//! (`tsx.platform.*`). Functions that change something (the clipboard, the
//! window, the app) run in handlers and effects, not while rendering.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const model = @import("model.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Value = types.Value;
const Type = types.Type;
const Error = core.Error;

const Kind = enum { void_, string, boolean, number, bytes, opt_string, opt_bytes };

const Function = struct {
    name: []const u8,
    zig: []const u8,
    params: []const Kind = &.{},
    ret: Kind = .void_,
    /// Changes the clipboard, a window or the app: not while rendering.
    effect: bool = false,
};

const Namespace = struct {
    name: []const u8,
    zig: []const u8,
    functions: []const Function,
};

const namespaces = [_]Namespace{
    .{ .name = "clipboard", .zig = "tsx.platform.clipboard", .functions = &.{
        .{ .name = "text", .zig = "text", .ret = .string },
        .{ .name = "setText", .zig = "setText", .params = &.{.string}, .ret = .boolean, .effect = true },
        .{ .name = "image", .zig = "image", .ret = .opt_bytes },
        .{ .name = "setImage", .zig = "setImage", .params = &.{.bytes}, .ret = .boolean, .effect = true },
        .{ .name = "files", .zig = "files", .ret = .opt_string },
        .{ .name = "setFiles", .zig = "setFiles", .params = &.{.string}, .ret = .boolean, .effect = true },
        .{ .name = "changeCount", .zig = "changeCount", .ret = .number },
    } },
    .{ .name = "app", .zig = "tsx.platform.app", .functions = &.{
        .{ .name = "quit", .zig = "quit", .effect = true },
        .{ .name = "launchAtLogin", .zig = "launchAtLogin", .ret = .string },
        .{
            .name = "setLaunchAtLogin",
            .zig = "setLaunchAtLogin",
            .params = &.{.boolean},
            .ret = .boolean,
            .effect = true,
        },
        .{ .name = "launchedAtLogin", .zig = "launchedAtLogin", .ret = .boolean },
        .{ .name = "deviceName", .zig = "deviceName", .ret = .string },
        .{ .name = "downloadsDir", .zig = "downloadsDir", .ret = .opt_string },
        .{ .name = "dataDir", .zig = "dataDir", .ret = .opt_string },
    } },
    .{ .name = "mainWindow", .zig = "tsx.platform.main_window", .functions = &.{
        .{ .name = "show", .zig = "show", .effect = true },
        .{ .name = "hide", .zig = "hide", .effect = true },
        .{ .name = "activate", .zig = "activate", .effect = true },
        .{ .name = "focus", .zig = "focus", .params = &.{.string}, .effect = true },
        .{ .name = "visible", .zig = "visible", .ret = .boolean },
        .{ .name = "active", .zig = "active", .ret = .boolean },
        .{ .name = "minimized", .zig = "minimized", .ret = .boolean },
    } },
};

/// Functions and constants imported by name.
const functions = [_]Function{
    .{ .name = "wakeAfter", .zig = "tsx.platform.wakeAfter", .params = &.{.number} },
    .{ .name = "shortcutLabel", .zig = "tsx.platform.shortcutLabel", .params = &.{.string}, .ret = .string },
};

const constants = [_][2][]const u8{
    .{ "isMacos", "tsx.platform.is_macos" },
    .{ "isWindows", "tsx.platform.is_windows" },
};

/// The names "zant" exports from this table (for the import check).
pub fn exports(name: []const u8) bool {
    for (namespaces) |ns| if (text.eq(ns.name, name)) return true;
    for (functions) |f| if (text.eq(f.name, name)) return true;
    for (constants) |constant| if (text.eq(constant[0], name)) return true;
    return text.eq(name, "keyLabel") or text.eq(name, "titlebarInsets");
}

/// `name` is imported from "zant" in this file and not shadowed.
fn visible(g: *Generator, c: *Context, name: []const u8) bool {
    if (@import("builtins.zig").shadowed(g, c, name)) return false;
    for (g.react_imports) |imp| if (text.eq(imp.source, "zant")) {
        for (imp.names) |n| if (!n.type_only and !imp.type_only and text.eq(n.local.name, name)) return true;
    };
    return false;
}

/// `isMacos` / `isWindows`; a namespace used as a value is an error.
pub fn read(g: *Generator, c: *Context, name: []const u8, loc: ast.Loc) Error!?Value {
    if (!visible(g, c, name)) return null;
    for (constants) |constant| if (text.eq(constant[0], name)) return .{ .ty = .boolean, .code = constant[1] };
    for (namespaces) |ns| if (text.eq(ns.name, name)) {
        return core.fail(g, loc, "`{s}` is a namespace; call its functions: {s}", .{ name, try names(g, ns) });
    };
    return core.fail(g, loc, "`{s}` is a function; call it", .{name});
}

/// `clipboard.text()`, `app.quit()`, `wakeAfter(ms)`, `keyLabel("mod+f")`, …
pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    switch (call.callee.data) {
        .ident => |name| {
            if (!visible(g, c, name)) return null;
            if (text.eq(name, "keyLabel")) return try keyLabel(g, loc, call);
            if (text.eq(name, "titlebarInsets")) return try titlebarInsets(g, c, loc, call);
            for (functions) |f| if (text.eq(f.name, name)) return try emitCall(g, c, loc, call, f, f.zig);
            return null;
        },
        .member => |m| {
            if (m.object.data != .ident or !visible(g, c, m.object.data.ident)) return null;
            const ns = for (namespaces) |ns| {
                if (text.eq(ns.name, m.object.data.ident)) break ns;
            } else return null;
            const f = for (ns.functions) |f| {
                if (text.eq(f.name, m.name.name)) break f;
            } else return core.fail(g, m.name.loc, "unknown function `{s}.{s}`; {s} has: {s}", .{
                ns.name, m.name.name, ns.name, try names(g, ns),
            });
            return try emitCall(g, c, loc, call, f, try text.fmt(g, "{s}.{s}", .{ ns.zig, f.zig }));
        },
        else => return null,
    }
}

fn emitCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call, f: Function, zig: []const u8) Error!Value {
    if (call.type_args.len != 0) return core.fail(g, loc, "`{s}` takes no type arguments", .{f.name});
    if (call.args.len != f.params.len) {
        return core.fail(g, loc, "`{s}` takes {d} argument(s), got {d}", .{ f.name, f.params.len, call.args.len });
    }
    if (c.constant) return core.fail(g, loc, "`{s}` cannot run in a top-level constant", .{f.name});
    if (f.effect and c.render) return core.fail(
        g,
        loc,
        "`{s}` changes the desktop; call it in an event handler or an effect, not while rendering",
        .{f.name},
    );
    var args: std.ArrayList([]const u8) = .empty;
    for (call.args, f.params) |arg, kind| {
        const ty = try typeOf(g, kind);
        const v = try types.coerce(g, try expr.emit(g, c, arg, ty, false), ty, arg.loc);
        try args.append(g.a, v.code);
    }
    return .{ .ty = try typeOf(g, f.ret), .code = try text.fmt(g, "{s}({s})", .{
        zig, try std.mem.join(g.a, ", ", args.items),
    }) };
}

fn typeOf(g: *Generator, kind: Kind) Error!Type {
    return switch (kind) {
        .void_ => .void_,
        .string => .string,
        .boolean => .boolean,
        .number => .f64,
        .bytes => try model.array(g, .u8),
        .opt_string => try model.optional(g, .string),
        .opt_bytes => try model.optional(g, try model.array(g, .u8)),
    };
}

/// `keyLabel("mod+shift+s")`: the platform's label, known at compile time.
fn keyLabel(g: *Generator, loc: ast.Loc, call: ast.Call) Error!Value {
    if (call.args.len != 1 or call.args[0].data != .string) {
        return core.fail(g, loc, "keyLabel takes a string literal: keyLabel(\"mod+f\")", .{});
    }
    return .{ .ty = .string, .code = try text.fmt(g, "zant.keys.label({s})", .{
        try text.quote(g, call.args[0].data.string),
    }) };
}

/// `titlebarInsets()`: `{ left, right }`, the space the system's window
/// buttons take in a hidden title bar.
fn titlebarInsets(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!Value {
    if (call.args.len != 0) return core.fail(g, loc, "titlebarInsets() takes no arguments", .{});
    if (!c.render) return core.fail(g, loc, "titlebarInsets() is read while rendering", .{});
    const zig = "@TypeOf(ui.titlebarInsets())";
    const ty = for (g.compounds.items, 0..) |entry, i| {
        if (text.eq(entry.zig, zig)) break @as(Type, @fromBackingInt(@intCast(types.compound_base + i)));
    } else try model.add(g, .{ .kind = .object, .zig = zig, .fields = try g.a.dupe(model.Field, &.{
        .{ .name = "left", .ty = .f32 },
        .{ .name = "right", .ty = .f32 },
    }) });
    return .{ .ty = ty, .code = "ui.titlebarInsets()" };
}

fn names(g: *Generator, ns: Namespace) Error![]const u8 {
    var list: std.ArrayList([]const u8) = .empty;
    for (ns.functions) |f| try list.append(g.a, f.name);
    return std.mem.join(g.a, ", ", list.items);
}
