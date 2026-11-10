//! `useTray` and `useHotkey` (docs/tsx.md, "托盘与全局快捷键"):
//!
//!     useTray({ icon: trayIcon, tooltip: "Clips", items: [{ title: "Open", onSelect: () => showMain() }] });
//!     const ok = useHotkey(hotkey, () => panel.toggle());   // "ctrl+cmd+v"; "" for none
//!
//! Both run while rendering and apply only changes (`src/tsx_desktop.zig`);
//! their callbacks are handlers (captures, snapshots) registered before the
//! component is emitted.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const events = @import("events.zig");
const captures = @import("captures.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Error = core.Error;

/// A callback of a desktop hook and its method.
pub const Callback = struct {
    expr: *const ast.Expr,
    method: usize,
};

pub fn isHook(name: []const u8) bool {
    return text.eq(name, "useTray") or text.eq(name, "useHotkey");
}

/// Registers the callbacks of `useTray(…)` / `useHotkey(…)` (before handlers
/// are scanned, like event arrows).
pub fn collect(g: *Generator, e: *ast.Expr) Error!void {
    const call = e.data.call;
    const name = call.callee.data.ident;
    if (text.eq(name, "useHotkey")) {
        if (call.args.len != 2) return core.fail(g, e.loc, "useHotkey(shortcut, () => …)", .{});
        return callback(g, call.args[1]);
    }
    if (call.args.len != 1 or call.args[0].data != .object) {
        return core.fail(g, e.loc, "useTray takes its options: useTray({{ icon, tooltip, items }})", .{});
    }
    for (call.args[0].data.object) |p| {
        if (text.eq(p.key.name, "onClick")) try callback(g, p.value);
        if (!text.eq(p.key.name, "items")) continue;
        if (p.value.data != .array) return core.fail(g, p.value.loc, "tray items are an array literal", .{});
        for (p.value.data.array) |item| {
            if (item.data != .object) return core.fail(g, item.loc, "a tray item is an object literal", .{});
            for (item.data.object) |field| if (text.eq(field.key.name, "onSelect")) try callback(g, field.value);
        }
    }
}

fn callback(g: *Generator, e: *ast.Expr) Error!void {
    const arrow = switch (e.data) {
        .arrow => e,
        .ident => |n| if (core.inlineFunction(g, n)) |f| blk: {
            const arrow = try g.a.create(ast.Expr);
            arrow.* = .{ .loc = e.loc, .data = .{ .arrow = f } };
            break :blk arrow;
        } else try events.functionArrow(g, e, .action),
        else => return core.fail(g, e.loc, "a callback here is an arrow or a function name", .{}),
    };
    if (arrow.data.arrow.params.len != 0) return core.fail(g, arrow.loc, "this callback takes no parameters", .{});
    g.counter += 1;
    try g.desktop_callbacks.append(g.a, .{ .expr = e, .method = g.methods.items.len });
    try g.methods.append(g.a, .{
        .name = try scope.uniqueMethod(g, try text.fmt(g, "_desktop{d}", .{g.counter})),
        .func = arrow.data.arrow,
        .action = .action,
        .capture_name = try scope.uniqueMethod(g, try text.fmt(g, "_DesktopCapture{d}", .{g.counter})),
    });
}

/// The `ui.action(…)` of a registered callback.
fn action(g: *Generator, c: *Context, e: *const ast.Expr) Error![]const u8 {
    const info = for (g.desktop_callbacks.items) |item| {
        if (item.expr == e) break item;
    } else unreachable; // registered by `collect`
    try captures.prepare(g, c, info.method);
    const m = g.methods.items[info.method];
    const receiver = if (m.captures.items.len > 0) try captures.value(g, m) else "self";
    return text.fmt(g, "ui.action({s}, {s}.{s})", .{ receiver, try text.id(g, g.name), try text.id(g, m.name) });
}

/// The boolean value of the hook call (registered / tray shown).
pub fn value(g: *Generator, c: *Context, e: *ast.Expr, top: bool) Error!types.Value {
    if (!c.render or !top) return core.fail(g, e.loc, "hooks must be called at the top level of the component", .{});
    const call = e.data.call;
    if (text.eq(call.callee.data.ident, "useHotkey")) {
        const shortcut = try expr.emit(g, c, call.args[0], .string, false);
        g.counter += 1;
        const field = try scope.uniqueMethod(g, try text.fmt(g, "_hotkey{d}", .{g.counter}));
        try g.hook_fields.append(g.a, .{ .name = field, .zig_type = "tsx.desktop.Hotkey" });
        return .{ .ty = .boolean, .code = try text.fmt(g, "self.{s}.update({s}, {s})", .{
            try scope.stateField(g, field), shortcut.code, try action(g, c, call.args[1]),
        }) };
    }
    const options = try trayOptions(g, c, call.args[0]);
    return .{ .ty = .boolean, .code = try text.fmt(g, "tsx.desktop.tray.update({s})", .{options}) };
}

fn trayOptions(g: *Generator, c: *Context, e: *ast.Expr) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    var icon = false;
    for (e.data.object) |p| {
        const key = p.key.name;
        if (p.spread) return core.fail(g, p.value.loc, "list the tray options one by one", .{});
        const code = if (text.eq(key, "icon") or text.eq(key, "iconLight")) blk: {
            icon = icon or text.eq(key, "icon");
            break :blk (try expr.emit(g, c, p.value, try @import("model.zig").array(g, .u8), false)).code;
        } else if (text.eq(key, "template"))
            (try expr.emit(g, c, p.value, .boolean, false)).code
        else if (text.eq(key, "tooltip"))
            (try expr.emit(g, c, p.value, .string, false)).code
        else if (text.eq(key, "onClick"))
            try action(g, c, p.value)
        else if (text.eq(key, "items"))
            try trayItems(g, c, p.value)
        else
            return core.fail(g, p.key.loc, "unknown tray option `{s}`; options: icon, iconLight, template, tooltip, " ++
                "items, onClick", .{key});
        const zig = if (text.eq(key, "iconLight")) "icon_light" else if (text.eq(key, "onClick")) "on_click" else key;
        try out.appendSlice(g.a, try text.fmt(g, ".{s} = {s},\n", .{ zig, code }));
    }
    if (!icon) return core.fail(g, e.loc, "the tray needs an `icon` (PNG bytes, e.g. an imported .png)", .{});
    return text.fmt(g, ".{{\n{s}}}", .{out.items});
}

fn trayItems(g: *Generator, c: *Context, e: *ast.Expr) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    for (e.data.array) |item| {
        var fields: std.ArrayList(u8) = .empty;
        for (item.data.object) |p| {
            const key = p.key.name;
            const code = if (text.eq(key, "title"))
                (try expr.emit(g, c, p.value, .string, false)).code
            else if (text.eq(key, "checked") or text.eq(key, "disabled") or text.eq(key, "separator"))
                (try expr.emit(g, c, p.value, .boolean, false)).code
            else if (text.eq(key, "onSelect"))
                try action(g, c, p.value)
            else
                return core.fail(g, p.key.loc, "unknown tray item field `{s}`; fields: title, checked, disabled, " ++
                    "separator, onSelect", .{key});
            const zig = if (text.eq(key, "onSelect")) "on_select" else key;
            try fields.appendSlice(g.a, try text.fmt(g, ".{s} = {s}, ", .{ zig, code }));
        }
        try out.appendSlice(g.a, try text.fmt(g, ".{{ {s}}},\n", .{fields.items}));
    }
    return text.fmt(g, "&.{{\n{s}}}", .{out.items});
}
