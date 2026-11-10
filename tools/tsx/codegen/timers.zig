//! `setTimeout` / `clearTimeout` (docs/tsx.md, "定时器"; runtime in
//! `src/tsx_timers.zig`):
//!
//!     const id = setTimeout(() => expire(row), 8000);   // in a handler or effect
//!     clearTimeout(id);
//!
//! The callback is registered as a method (like an event arrow) while the
//! component is scanned. It runs after the frame that set it is gone, so its
//! captures are copied into the timer: numbers, booleans and enums only, and
//! only the root component or a window's (components without props, which
//! live as long as the app) can set timers.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const events = @import("events.zig");
const captures = @import("captures.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Value = types.Value;
const Error = core.Error;

fn isCall(e: *const ast.Expr, name: []const u8) bool {
    return e.data == .call and e.data.call.callee.data == .ident and text.eq(e.data.call.callee.data.ident, name);
}

/// While scanning: registers the callback of `setTimeout(cb, ms)`.
pub fn scan(g: *Generator, e: *ast.Expr) Error!void {
    if (!isCall(e, "setTimeout")) return;
    const call = e.data.call;
    if (call.args.len != 2) return core.fail(g, e.loc, "setTimeout(() => …, ms)", .{});
    const cb = call.args[0];
    const arrow = switch (cb.data) {
        .arrow => cb,
        .ident => try events.functionArrow(g, cb, .action),
        else => return core.fail(g, cb.loc, "the callback is an arrow or a function name", .{}),
    };
    if (arrow.data.arrow.params.len != 0) return core.fail(g, arrow.loc, "a timer callback takes no parameters", .{});
    g.counter += 1;
    try g.desktop_callbacks.append(g.a, .{ .expr = cb, .method = g.methods.items.len });
    try g.methods.append(g.a, .{
        .name = try scope.uniqueMethod(g, try text.fmt(g, "_timeout{d}", .{g.counter})),
        .func = arrow.data.arrow,
        .action = .action,
        .capture_name = try scope.uniqueMethod(g, try text.fmt(g, "_TimeoutCapture{d}", .{g.counter})),
    });
}

/// `setTimeout(…)` → `tsx.timers.set(ms, ctx, Comp._timeoutN)`; `clearTimeout(id)`.
pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    if (call.callee.data != .ident) return null;
    const name = call.callee.data.ident;
    const set = text.eq(name, "setTimeout");
    if (!set and !text.eq(name, "clearTimeout")) return null;
    if (@import("builtins.zig").shadowed(g, c, name)) return null;
    if (c.render or c.constant) {
        return core.fail(g, loc, "{s} runs in event handlers and effects, not while rendering", .{name});
    }
    if (!set) {
        if (call.args.len != 1) return core.fail(g, loc, "clearTimeout(id)", .{});
        const id = try expr.emit(g, c, call.args[0], .f64, false);
        return .{ .ty = .void_, .code = try text.fmt(g, "tsx.timers.clear({s})", .{
            (try types.coerce(g, id, .f64, call.args[0].loc)).code,
        }) };
    }
    if (c.no_self) return core.fail(g, loc, "setTimeout runs in a component's handlers (a plain function has none)", .{});
    if (g.current_component) |comp| if (comp.props != null) return core.fail(
        g,
        loc,
        "timers belong to the root component or a window's (components without props): they outlive other " ++
            "components; set it there and pass the result down",
        .{},
    );
    const ms = try expr.emit(g, c, call.args[1], .f64, false);
    const info = for (g.desktop_callbacks.items) |item| {
        if (item.expr == call.args[0]) break item;
    } else unreachable; // registered by `scan`
    try captures.prepare(g, c, info.method);
    const m = g.methods.items[info.method];
    const target = try text.fmt(g, "{s}.{s}", .{ try text.id(g, g.name), try text.id(g, m.name) });
    var ctx: []const u8 = "self";
    if (m.captures.items.len > 0) {
        var fields: std.ArrayList(u8) = .empty;
        try fields.appendSlice(g.a, ".self = self, ");
        for (m.captures.items) |capture| {
            if (!plain(g, capture.ty)) return core.fail(
                g,
                call.args[0].loc,
                "a timer callback can capture numbers, booleans and enums (`{s}` is not one); keep other values in " ++
                    "state and read them when it runs",
                .{capture.name},
            );
            try fields.appendSlice(g.a, try text.fmt(g, ".{s} = {s}, ", .{ try text.id(g, capture.field), capture.source }));
        }
        ctx = try text.fmt(g, "{s}{{ {s}}}", .{ m.capture_name.?, fields.items });
    }
    return .{ .ty = .f64, .code = try text.fmt(g, "tsx.timers.set({s}, {s}, {s})", .{
        (try types.coerce(g, ms, .f64, call.args[1].loc)).code, ctx, target,
    }) };
}

/// Values a timer can keep after the frame is gone.
fn plain(g: *Generator, ty: types.Type) bool {
    if (ty == .boolean or ty.numeric()) return true;
    if (model.optionalChild(g, ty)) |child| return plain(g, child);
    const entry = model.info(g, ty) orelse return false;
    return entry.kind == .enumeration;
}
