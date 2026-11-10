//! Inline array callbacks become labeled value blocks inside ordinary Zig loops.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const expr = @import("expr.zig");
const scope = @import("scope.zig");
const stmt = @import("stmt.zig");
const aggregate = @import("aggregate.zig");
const text = @import("zig_text.zig");

/// Parameters bind as scoped locals so unused parameters discard beside their declarations.
pub fn emit(
    g: *core.Generator,
    c: *scope.Context,
    callback: *ast.Expr,
    arguments: []const types.Value,
    expected: ?types.Type,
) core.Error!types.Value {
    if (callback.data != .arrow) return core.fail(g, callback.loc, "array callback must be an inline arrow", .{});
    const function = callback.data.arrow;
    if (function.params.len > arguments.len) {
        return core.fail(g, callback.loc, "array callback expects at most {d} parameters", .{arguments.len});
    }
    const depth = c.active.items.len;
    // elements built here repeat once per item (`nesting.zig`: keys)
    g.loop_depth += 1;
    defer g.loop_depth -= 1;
    const previous_break = c.break_label;
    const previous_continue = c.continue_label;
    c.break_label = null;
    c.continue_label = null;
    const previous_ret = c.ret;
    const previous_label = c.return_label;
    const previous_values = c.return_values;
    defer {
        c.active.shrinkRetainingCapacity(depth);
        c.break_label = previous_break;
        c.continue_label = previous_continue;
        c.ret = previous_ret;
        c.return_label = previous_label;
        c.return_values = previous_values;
    }
    var prefix: std.ArrayList(u8) = .empty;
    for (function.params, arguments[0..function.params.len]) |param, argument| {
        if (param.optional or param.destructure.len > 0) {
            return core.fail(g, param.name.loc, "array callback parameters must be simple names", .{});
        }
        if (param.ty) |ref| {
            if (try types.typeRef(g, ref, false) != argument.ty) {
                return core.fail(g, ref.loc, "callback parameter must have type {s}", .{argument.ty.zig(g)});
            }
        }
        const binding = try scope.bind(c, param.name.name, argument.ty, false);
        binding.init = argument.code;
        try prefix.appendSlice(g.a, try text.fmt(g, "\x01{d}\x02", .{binding.serial}));
    }
    const annotation = if (function.ret) |ref| try types.typeRef(g, ref, false) else expected;
    var ty: types.Type = undefined;
    var code: []const u8 = undefined;
    const label = try aggregate.temporary(c, "_callback");
    if (function.body == .expr) {
        var value = try expr.emit(g, c, function.body.expr, annotation, false);
        ty = annotation orelse value.ty.concrete();
        value = try types.coerce(g, value, ty, callback.loc);
        code = try text.fmt(g, "break :{s} {s};\n", .{ label, value.code });
    } else {
        if (!stmt.returnsAll(function.body.block)) {
            return core.fail(g, callback.loc, "array callback must return on every path", .{});
        }
        c.ret = annotation orelse .void_;
        c.return_label = label;
        var values: std.ArrayList(scope.ReturnValue) = .empty;
        c.return_values = if (annotation == null) &values else null;
        code = try stmt.statements(g, c, function.body.block, false);
        ty = c.ret;
        if (annotation == null) {
            if (values.items.len == 0) return core.fail(g, callback.loc, "callback must return a value", .{});
            ty = values.items[0].value.ty;
            for (values.items[1..]) |returned| ty = try types.unify(g, ty, returned.value.ty, returned.loc);
            ty = ty.concrete();
            for (values.items, 0..) |returned, index| {
                const value = try types.coerce(g, returned.value, ty, returned.loc);
                code = try std.mem.replaceOwned(u8, g.a, code, try text.fmt(g, "\x05{d}\x06", .{index}), value.code);
            }
        }
    }
    const value = types.Value{ .ty = ty, .code = try text.fmt(g, "{s}: {{\n{s}{s}}}", .{
        label, prefix.items, code,
    }) };
    return if (expected) |target| types.coerce(g, value, target, callback.loc) else value;
}
