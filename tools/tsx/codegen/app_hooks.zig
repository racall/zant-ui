//! App hooks of the root component (docs/tsx.md, "应用"):
//!
//!     useCloseRequest(() => { if (dirty) { setAsk(true); return false; } return true; });
//!     useQuitRequest(() => true);
//!     useKeyDown((e: KeyEvent) => { if (e.key === "Escape") { … return true; } return false; });
//!
//! Each becomes a public method of the root struct (`onCloseRequest`,
//! `onQuitRequest`, `onKey`) that `zant.tsx.app.Shell` forwards the runtime's
//! hooks to. The callbacks run like event handlers (state snapshots,
//! setters, stores); they can't read the render body's locals.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Error = core.Error;

const Hook = struct {
    name: []const u8,
    method: []const u8,
    /// What the callback takes.
    param: enum { none, key, focused } = .none,
    returns: Type = .boolean,
    /// App events: the root component only (not windows).
    root_only: bool = false,
};

const hooks = [_]Hook{
    .{ .name = "useCloseRequest", .method = "onCloseRequest" },
    .{ .name = "useQuitRequest", .method = "onQuitRequest" },
    .{ .name = "useKeyDown", .method = "onKey", .param = .key },
    .{ .name = "useWindowFocus", .method = "onFocus", .param = .focused, .returns = .void_ },
    .{ .name = "useClipboardChange", .method = "onClipboard", .returns = .void_, .root_only = true },
    .{ .name = "useReopen", .method = "onReopen", .returns = .void_, .root_only = true },
};

fn find(name: []const u8) ?Hook {
    for (hooks) |h| if (text.eq(h.name, name)) return h;
    return null;
}

pub fn isAppHook(name: []const u8) bool {
    return find(name) != null;
}

/// `KeyEvent` (`zant.tsx.KeyEvent`): what `useKeyDown` passes.
pub fn keyEventType(g: *Generator) Error!Type {
    for (g.compounds.items, 0..) |entry, i| if (text.eq(entry.zig, "tsx.KeyEvent")) {
        return @fromBackingInt(@intCast(types.compound_base + i));
    };
    var fields: std.ArrayList(model.Field) = .empty;
    try fields.append(g.a, .{ .name = "key", .ty = .string });
    inline for (.{ "mod", "shift", "alt", "ctrl", "meta", "repeat" }) |name| {
        try fields.append(g.a, .{ .name = name, .ty = .boolean });
    }
    return model.add(g, .{ .kind = .object, .zig = "tsx.KeyEvent", .fields = fields.items });
}

/// Registers the callback of `useX(…)` (a top-level statement of a
/// component) as a method, before handlers are scanned.
pub fn collect(g: *Generator, e: *ast.Expr) Error!void {
    const call = e.data.call;
    const hook = find(call.callee.data.ident).?;
    if (hook.root_only and g.current_component != null) return core.fail(
        g,
        e.loc,
        "{s} belongs to the root component (the app); call it there",
        .{hook.name},
    );
    if (g.current_component) |comp| if (comp.props != null and model.info(g, comp.props.?).?.fields.len > 0) {
        return core.fail(
            g,
            e.loc,
            "{s} belongs to the root component or a window's component (no props); call it there",
            .{hook.name},
        );
    };
    if (call.args.len != 1 or call.args[0].data != .arrow) {
        return core.fail(g, e.loc, "{s} takes one function", .{hook.name});
    }
    const f = call.args[0].data.arrow;
    if (f.params.len > @intFromBool(hook.param != .none)) return core.fail(
        g,
        f.loc,
        "the {s} callback takes {s}",
        .{ hook.name, switch (hook.param) {
            .none => "no parameters",
            .key => "(e: KeyEvent)",
            .focused => "(focused: boolean)",
        } },
    );
    if (scope.methodIndex(g, hook.method) != null) return core.fail(
        g,
        e.loc,
        "{s} is used twice, or `{s}` is also a function of this component",
        .{ hook.name, hook.method },
    );
    const params: ?[]const Type = switch (hook.param) {
        .none => null,
        .key => try g.a.dupe(Type, &.{try keyEventType(g)}),
        .focused => try g.a.dupe(Type, &.{.boolean}),
    };
    try g.methods.append(g.a, .{
        .name = hook.method,
        .func = f,
        .ret = hook.returns,
        .callback_params = params,
    });
}

/// The render-time statement: nothing to emit; the method is emitted public.
pub fn statement(g: *Generator, c: *Context, e: *ast.Expr, top: bool) Error![]const u8 {
    const hook = find(e.data.call.callee.data.ident).?;
    if (!c.render or !top) return core.fail(g, e.loc, "hooks must be called at the top level of the component", .{});
    const index = scope.methodIndex(g, hook.method).?;
    const code = try core.method(g, g.methods.items[index], null, null);
    g.methods.items[index].emitted = try text.fmt(g, "pub {s}", .{code});
    return "";
}
