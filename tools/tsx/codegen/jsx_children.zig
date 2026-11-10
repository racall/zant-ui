//! Flatten fragments and preserve conditional-child evaluation with hidden bindings.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const jsx = @import("jsx.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Value = types.Value;
const Context = scope.Context;
const Binding = scope.Binding;
const Error = core.Error;

pub const ChildList = struct {
    g: *Generator,
    c: *Context,
    values: std.ArrayList(Value) = .empty,
    prefix: std.ArrayList(u8) = .empty,

    pub fn hidden(list: *ChildList, name: []const u8, t: Type) Error!*Binding {
        const binding = try scope.bind(list.c, name, t, false);
        binding.name = ""; // Compiler temporary, never a TS source binding.
        return binding;
    }

    pub fn append(list: *ChildList, cs: []const ast.Child, guard: ?[]const u8) Error!void {
        for (cs) |ch| switch (ch) {
            .text => |text| return core.fail(list.g, text.loc, "text must be inside <Text>", .{}),
            .element => |el| try list.element(el, guard),
            .expr => |x| if (x) |e| {
                try list.expression(e, guard);
            },
        };
    }

    /// Flatten a fragment or append its element under the inherited guard.
    fn element(list: *ChildList, el: *ast.Jsx, guard: ?[]const u8) Error!void {
        if (el.name.len == 0) return list.append(el.children, guard);
        const value = try jsx.emit(list.g, list.c, el);
        try list.node(value, guard, el.loc);
    }

    pub fn node(list: *ChildList, value: Value, guard: ?[]const u8, loc: ast.Loc) Error!void {
        if (value.ty != .node and value.ty != .optional_node and value.ty != .null_) {
            return core.fail(list.g, loc, "text must be inside <Text>", .{});
        }
        if (guard) |cond| {
            if (value.ty == .null_) return;
            try list.values.append(
                list.g.a,
                .{
                    .ty = .optional_node,
                    .code = try zig_text.fmt(list.g, "if ({s}) {s} else null", .{ cond, value.code }),
                },
            );
        } else try list.values.append(list.g.a, value);
    }

    pub fn condition(list: *ChildList, e: *ast.Expr, guard: ?[]const u8) Error![]const u8 {
        const value = try expr.condition(list.g, list.c, e);
        const binding = try list.hidden("_condition", .boolean);
        binding.init = if (guard) |parent| (try expr.infix(
            list.g,
            .{ .ty = .boolean, .code = parent },
            "and",
            value,
            .boolean,
        )).code else value.code;
        try list.prefix.appendSlice(list.g.a, try zig_text.fmt(list.g, "\x01{d}\x02", .{binding.serial}));
        return binding.zig;
    }

    pub fn expression(list: *ChildList, e: *ast.Expr, guard: ?[]const u8) Error!void {
        if (e.data == .jsx and e.data.jsx.name.len == 0) return list.append(e.data.jsx.children, guard);
        if (e.data == .binary and zig_text.eq(e.data.binary.op, "&&") and jsx.hasFragment(e.data.binary.rhs)) {
            const b = e.data.binary;
            return list.expression(b.rhs, try list.condition(b.lhs, guard));
        }
        if (e.data == .cond and (jsx.hasFragment(e.data.cond.then) or jsx.hasFragment(e.data.cond.else_))) {
            const v = e.data.cond;
            const cond = try list.condition(v.cond, guard);
            try list.expression(v.then, cond);
            const otherwise = if (guard) |parent|
                try zig_text.fmt(list.g, "{s} and !{s}", .{ parent, cond })
            else
                try zig_text.fmt(list.g, "!{s}", .{cond});
            return list.expression(v.else_, otherwise);
        }
        try list.node(try expr.emit(list.g, list.c, e, null, true), guard, e.loc);
    }

    pub fn wrap(list: *ChildList, code: []const u8) Error![]const u8 {
        if (list.prefix.items.len == 0) return code;
        const label = try list.hidden("_children", .void_);
        return zig_text.fmt(
            list.g,
            "{s}: {{\n{s}break :{s} {s};\n}}",
            .{ label.zig, list.prefix.items, label.zig, code },
        );
    }
};
