//! Other windows of the app (docs/tsx.md, "多窗口"):
//!
//!     export const panel = createWindow(Panel, { title: "Clips", style: "panel", width: 380, height: 480 });
//!     panel.toggle();  panel.place("trayCorner", 12);  panel.focus("search");
//!
//! A window is a file-level `pub var panel: tsx.Window(Panel)` (src/tsx_window.zig)
//! that opens on first use. `Panel` is a component without props; it keeps
//! its state while the window exists, and its app hooks (`useKeyDown`,
//! `useCloseRequest`, `useWindowFocus`) are the window's. Other files import
//! the window like a store.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const model = @import("model.zig");
const components = @import("components.zig");
const imports = @import("imports.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Value = types.Value;
const Error = core.Error;

pub const Window = struct {
    name: []const u8,
    /// Zig path: `panel` or `_App.panel`.
    zig: []const u8,
};

pub fn isCreate(e: *const ast.Expr) bool {
    return e.data == .call and e.data.call.callee.data == .ident and
        text.eq(e.data.call.callee.data.ident, "createWindow");
}

const Option = struct {
    ts: []const u8,
    zig: []const u8,
    kind: enum { number, boolean, color, style },
};

/// `createWindow` options → `zant.WindowOptions` fields (`title` is apart).
const options = [_]Option{
    .{ .ts = "style", .zig = "style", .kind = .style },
    .{ .ts = "width", .zig = "width", .kind = .number },
    .{ .ts = "height", .zig = "height", .kind = .number },
    .{ .ts = "minWidth", .zig = "min_width", .kind = .number },
    .{ .ts = "minHeight", .zig = "min_height", .kind = .number },
    .{ .ts = "background", .zig = "background", .kind = .color },
    .{ .ts = "resizable", .zig = "resizable", .kind = .boolean },
    .{ .ts = "alwaysOnTop", .zig = "always_on_top", .kind = .boolean },
    .{ .ts = "movableByBackground", .zig = "movable_by_background", .kind = .boolean },
    .{ .ts = "center", .zig = "center", .kind = .boolean },
    .{ .ts = "titlebarHeight", .zig = "titlebar_height", .kind = .number },
};

const styles = [_][2][]const u8{
    .{ "native", "native" },         .{ "hidden-titlebar", "hidden_titlebar" }, .{ "hiddenTitlebar", "hidden_titlebar" },
    .{ "borderless", "borderless" }, .{ "panel", "panel" },
};

const places = [_][2][]const u8{
    .{ "center", "center" },          .{ "topLeft", "top_left" },         .{ "topRight", "top_right" },
    .{ "bottomLeft", "bottom_left" }, .{ "bottomRight", "bottom_right" }, .{ "trayCorner", "tray_corner" },
};

/// `createWindow(Component, { … })` constants → `pub var` declarations.
pub fn collect(g: *Generator, constants: []const ast.TopConst) Error![]const u8 {
    var code: std.ArrayList(u8) = .empty;
    for (constants) |constant| {
        if (!isCreate(constant.value)) continue;
        const call = constant.value.data.call;
        if (call.args.len < 1 or call.args.len > 2 or call.args[0].data != .ident) return core.fail(
            g,
            constant.value.loc,
            "createWindow takes a component and options: createWindow(Panel, {{ title: \"…\", width: 380 }})",
            .{},
        );
        if (scope.globalName(g, constant.name.name)) {
            return core.fail(g, constant.name.loc, "duplicate or reserved name `{s}`", .{constant.name.name});
        }
        const comp = components.find(g, call.args[0].data.ident) orelse return core.fail(
            g,
            call.args[0].loc,
            "`{s}` is not a component of this file or imported",
            .{call.args[0].data.ident},
        );
        if (comp.props) |props| for (model.info(g, props).?.fields) |f| if (!f.optional) {
            return core.fail(g, call.args[0].loc, "a window's component takes no props (`{s}` needs `{s}`)", .{
                comp.name, f.name,
            });
        };
        const struct_name = comp.builder[0 .. comp.builder.len - ".build".len];
        const opts = if (call.args.len == 2) try optionsValue(g, call.args[1]) else ".{}";
        const name = try text.id(g, constant.name.name);
        try code.appendSlice(g.a, try text.fmt(g, "{s}var {s}: tsx.Window({s}) = .{{ .options = {s} }};\n\n", .{
            if (constant.exported) "pub " else "", name, struct_name, opts,
        }));
        try g.windows.append(g.a, .{ .name = constant.name.name, .zig = name });
    }
    return code.items;
}

fn optionsValue(g: *Generator, e: *ast.Expr) Error![]const u8 {
    if (e.data != .object) return core.fail(g, e.loc, "window options are an object literal", .{});
    var c = Context{ .g = g, .render = true, .ret = .void_, .constant = true };
    var title: []const u8 = "\"\"";
    var fields: std.ArrayList(u8) = .empty;
    for (e.data.object) |p| {
        if (p.spread) return core.fail(g, p.value.loc, "list the window options one by one", .{});
        if (text.eq(p.key.name, "title")) {
            if (p.value.data != .string) return core.fail(g, p.value.loc, "the title is a string literal", .{});
            title = try text.quote(g, p.value.data.string);
            continue;
        }
        const option = for (options) |o| {
            if (text.eq(o.ts, p.key.name)) break o;
        } else return core.fail(g, p.key.loc, "unknown window option `{s}`; options: title, {s}", .{
            p.key.name, try optionNames(g),
        });
        const code = switch (option.kind) {
            .number => (try expr.emit(g, &c, p.value, .f32, false)).code,
            .boolean => (try expr.emit(g, &c, p.value, .boolean, false)).code,
            .color => try text.color(g, &c, p.value),
            .style => try literal(g, p.value, &styles, "style"),
        };
        try fields.appendSlice(g.a, try text.fmt(g, ".{s} = {s}, ", .{ option.zig, code }));
    }
    return text.fmt(g, ".{{ .title = {s}, .window = .{{ {s}}} }}", .{ title, fields.items });
}

fn optionNames(g: *Generator) Error![]const u8 {
    var names: std.ArrayList([]const u8) = .empty;
    for (options) |o| try names.append(g.a, o.ts);
    return std.mem.join(g.a, ", ", names.items);
}

fn literal(g: *Generator, e: *ast.Expr, table: []const [2][]const u8, what: []const u8) Error![]const u8 {
    if (e.data == .string) for (table) |entry| if (text.eq(entry[0], e.data.string)) {
        return text.fmt(g, ".{s}", .{entry[1]});
    };
    var names: std.ArrayList([]const u8) = .empty;
    for (table) |entry| try names.append(g.a, entry[0]);
    return core.fail(g, e.loc, "{s} is one of: {s}", .{ what, try std.mem.join(g.a, ", ", names.items) });
}

/// The window a file exports as `name`, if any.
pub fn exported(module: ast.Module, name: []const u8) ?ast.TopConst {
    for (module.consts) |constant| {
        if (constant.exported and isCreate(constant.value) and text.eq(constant.name.name, name)) return constant;
    }
    return null;
}

/// `import { panel } from "./App"`.
pub fn import(g: *Generator, n: ast.ImportName, from: *const imports.Scope) Error!void {
    try g.windows.append(g.a, .{
        .name = n.local.name,
        .zig = try text.fmt(g, "{s}.{s}", .{ try text.id(g, from.alias), try text.id(g, n.imported.name) }),
    });
}

fn find(g: *Generator, c: *Context, e: *const ast.Expr) ?Window {
    if (e.data != .ident or scope.local(c, e.data.ident) != null) return null;
    for (g.windows.items) |w| if (text.eq(w.name, e.data.ident)) return w;
    return null;
}

/// `panel.show()`, `hide`, `activate`, `toggle`, `visible`, `place`, `focus`.
pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    if (call.callee.data != .member) return null;
    const m = call.callee.data.member;
    const w = find(g, c, m.object) orelse return null;
    const method = m.name.name;
    inline for (.{ "show", "hide", "activate", "toggle" }) |name| if (text.eq(method, name)) {
        if (call.args.len != 0) return core.fail(g, loc, "{s}() takes no arguments", .{name});
        return .{ .ty = .void_, .code = try text.fmt(g, "{s}.{s}()", .{ w.zig, name }) };
    };
    inline for (.{ "visible", "active", "minimized" }) |name| if (text.eq(method, name)) {
        if (call.args.len != 0) return core.fail(g, loc, "{s}() takes no arguments", .{name});
        return .{ .ty = .boolean, .code = try text.fmt(g, "{s}.{s}()", .{ w.zig, name }) };
    };
    if (text.eq(method, "place")) {
        if (call.args.len < 1 or call.args.len > 2) return core.fail(g, loc, "place(where, margin?)", .{});
        const where = try literal(g, call.args[0], &places, "the place");
        const margin = if (call.args.len == 2) (try expr.emit(g, c, call.args[1], .f32, false)).code else "0";
        return .{ .ty = .void_, .code = try text.fmt(g, "{s}.place({s}, {s})", .{ w.zig, where, margin }) };
    }
    if (text.eq(method, "focus")) {
        if (call.args.len != 1) return core.fail(g, loc, "focus(key) takes the key of a node", .{});
        const key = try expr.emit(g, c, call.args[0], .string, false);
        return .{ .ty = .void_, .code = try text.fmt(g, "{s}.focus({s})", .{ w.zig, key.code }) };
    }
    return core.fail(g, m.name.loc, "a window has show, hide, activate, toggle, visible, active, minimized, place " ++
        "and focus", .{});
}
