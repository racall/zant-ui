//! Array operations lower to frame slices and explicit loops, with no runtime closures.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const aggregate = @import("aggregate.zig");
const callbacks = @import("callbacks.zig");
const expr = @import("expr.zig");
const scope = @import("scope.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;
const Kind = enum { map, filter, find, findIndex, some, every, reduce, indexOf, includes, slice, join, concat };
const methods = std.StaticStringMap(Kind).initComptime(.{
    .{ "map", .map },           .{ "filter", .filter }, .{ "find", .find },     .{ "findIndex", .findIndex },
    .{ "some", .some },         .{ "every", .every },   .{ "reduce", .reduce }, .{ "indexOf", .indexOf },
    .{ "includes", .includes }, .{ "slice", .slice },   .{ "join", .join },     .{ "concat", .concat },
});

pub fn emit(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call, expected: ?Type) Error!Value {
    const member = call.callee.data.member;
    const value = try expr.emit(g, c, member.object, null, false);
    if (value.ty == .string) return @import("string_methods.zig").emit(g, c, loc, call, value);
    if (model.optionalChild(g, value.ty)) |child_type| {
        const message = if (child_type == .string) "optional value needs ?? or !" else "optional array needs ?? or !";
        return core.fail(g, loc, "{s}", .{message});
    }
    const child = model.element(g, value.ty) orelse return core.fail(g, loc, "array method needs an array", .{});
    const kind = methods.get(member.name.name) orelse return core.fail(
        g,
        member.name.loc,
        "unknown array method `{s}`",
        .{member.name.name},
    );
    if (c.constant) return core.fail(g, loc, "array methods cannot run in a top-level constant", .{});
    const source = try aggregate.temporary(c, "_items");
    const label = try aggregate.temporary(c, "_array");
    const prefix = try text.fmt(g, "const {s}: {s} = {s};\n", .{ source, value.ty.zig(g), value.code });
    var result = switch (kind) {
        .map, .filter, .find, .findIndex, .some, .every, .reduce => try loop(
            g,
            c,
            loc,
            kind,
            call.args,
            source,
            child,
            expected,
        ),
        .indexOf, .includes => try search(g, c, loc, kind, call.args, source, child),
        .slice => try slice(g, c, loc, call.args, source, value.ty),
        .join => try join(g, c, loc, call.args, source, child),
        .concat => try concat(g, c, call.args, source, value.ty, child),
    };
    result.code = try text.fmt(g, "{s}: {{\n{s}break :{s} {s};\n}}", .{ label, prefix, label, result.code });
    return result;
}

fn arity(g: *Generator, loc: ast.Loc, args: []const *ast.Expr, min: usize, max: usize) Error!void {
    if (args.len < min or args.len > max) return core.fail(
        g,
        loc,
        "array method expects {d}..{d} arguments",
        .{ min, max },
    );
}

/// Callback results are assigned into typed storage, including optional find results.
fn loop(
    g: *Generator,
    c: *Context,
    loc: ast.Loc,
    kind: Kind,
    args: []const *ast.Expr,
    source: []const u8,
    child: Type,
    target: ?Type,
) Error!Value {
    try arity(g, loc, args, if (kind == .reduce) 2 else 1, if (kind == .reduce) 2 else 1);
    const item = try aggregate.temporary(c, "_item");
    const index = try aggregate.temporary(c, "_index");
    const result = try aggregate.temporary(c, "_result");
    const count = try aggregate.temporary(c, "_count");
    const label = try aggregate.temporary(c, "_loop");
    var initial: Value = .{ .ty = .void_, .code = "" };
    if (kind == .reduce) {
        initial = try expr.emit(g, c, args[1], null, false);
        initial = try types.coerce(g, initial, initial.ty.concrete(), args[1].loc);
    }
    const values = [_]Value{
        .{ .ty = child, .code = item }, .{ .ty = .usize, .code = index },
    };
    const reduce_values = [_]Value{
        .{ .ty = initial.ty, .code = result }, values[0], values[1],
    };
    const expected: ?Type = switch (kind) {
        .map => if (target) |ty| model.element(g, ty) else null,
        .reduce => initial.ty,
        else => .boolean,
    };
    const callback = try callbacks.emit(g, c, args[0], if (kind == .reduce) &reduce_values else &values, expected);
    const params = args[0].data.arrow.params.len;
    const needs_index = kind == .findIndex or params > (if (kind == .reduce) @as(usize, 2) else 1);
    const header = if (needs_index)
        try text.fmt(g, "for ({s}, 0..) |{s}, {s}|", .{ source, item, index })
    else
        try text.fmt(g, "for ({s}) |{s}|", .{ source, item });
    const uses_item = kind == .filter or kind == .find;
    const discard = if (!uses_item and (params == 0 or (kind == .reduce and params < 2)))
        try text.fmt(g, "_ = {s};\n", .{item})
    else
        "";
    const ty: Type = switch (kind) {
        .map => try model.array(g, callback.ty),
        .filter => try model.array(g, child),
        .find => try model.optional(g, child),
        .findIndex => .isize,
        .some, .every => .boolean,
        .reduce => initial.ty,
        else => unreachable,
    };
    const names = LoopNames{
        .source = source,
        .item = item,
        .index = index,
        .result = result,
        .count = count,
        .child = child,
        .ty = ty,
        .initial = initial,
        .callback = callback,
    };
    const plan = switch (kind) {
        .map => try mapPlan(g, names),
        .filter => try filterPlan(g, names),
        .reduce => try reducePlan(g, names),
        else => try predicatePlan(g, names, kind),
    };
    return .{ .ty = ty, .code = try text.fmt(g, "{s}: {{\n{s}{s} {{\n{s}{s}\n}}\nbreak :{s} {s};\n}}", .{
        label, plan.setup, header, discard, plan.body, label, plan.output,
    }) };
}

const LoopNames = struct {
    source: []const u8,
    item: []const u8,
    index: []const u8,
    result: []const u8,
    count: []const u8,
    child: Type,
    ty: Type,
    initial: Value,
    callback: Value,
};
const Plan = struct { setup: []const u8, body: []const u8, output: []const u8 };

/// Map allocates exactly one result slot per input element.
fn mapPlan(g: *Generator, n: LoopNames) Error!Plan {
    return .{
        .setup = try text.fmt(g, "const {s} = tsx.alloc({s}, {s}.len);\nvar {s}: usize = 0;\n", .{
            n.result, n.callback.ty.zig(g), n.source, n.count,
        }),
        .body = try text.fmt(g, "{s}[{s}] = {s};\n{s} += 1;", .{ n.result, n.count, n.callback.code, n.count }),
        .output = n.result,
    };
}

/// Filter retains matching elements in input order and returns the initialized prefix.
fn filterPlan(g: *Generator, n: LoopNames) Error!Plan {
    return .{
        .setup = try text.fmt(g, "const {s} = tsx.alloc({s}, {s}.len);\nvar {s}: usize = 0;\n", .{
            n.result, n.child.zig(g), n.source, n.count,
        }),
        .body = try text.fmt(g, "if ({s}) {{\n{s}[{s}] = {s};\n{s} += 1;\n}}", .{
            n.callback.code, n.result, n.count, n.item, n.count,
        }),
        .output = try text.fmt(g, "{s}[0..{s}]", .{ n.result, n.count }),
    };
}

/// Reduce carries a typed accumulator without allocating an intermediate slice.
fn reducePlan(g: *Generator, n: LoopNames) Error!Plan {
    return .{
        .setup = try text.fmt(g, "var {s}: {s} = {s};\n", .{ n.result, n.ty.zig(g), n.initial.code }),
        .body = try text.fmt(g, "{s} = {s};", .{ n.result, n.callback.code }),
        .output = n.result,
    };
}

/// Search predicates short-circuit; find stores an optional value rather than an identity.
fn predicatePlan(g: *Generator, n: LoopNames, kind: Kind) Error!Plan {
    const default = switch (kind) {
        .find => "null",
        .findIndex => "-1",
        .every => "true",
        else => "false",
    };
    const found = switch (kind) {
        .find => n.item,
        .findIndex => try text.fmt(g, "@intCast({s})", .{n.index}),
        .some => "true",
        .every => "false",
        else => unreachable,
    };
    return .{
        .setup = try text.fmt(g, "var {s}: {s} = {s};\n", .{ n.result, n.ty.zig(g), default }),
        .body = try text.fmt(g, "if ({s}{s}) {{\n{s} = {s};\nbreak;\n}}", .{
            if (kind == .every) "!" else "", n.callback.code, n.result, found,
        }),
        .output = n.result,
    };
}

fn search(
    g: *Generator,
    c: *Context,
    loc: ast.Loc,
    kind: Kind,
    args: []const *ast.Expr,
    source: []const u8,
    child: Type,
) Error!Value {
    try arity(g, loc, args, 1, 1);
    if (!child.numeric() and child != .string and child != .boolean and !model.enumType(g, child)) {
        return core.fail(g, loc, "includes/indexOf need primitive or string elements", .{});
    }
    const value = try expr.emit(g, c, args[0], child, false);
    const needle = try aggregate.temporary(c, "_needle");
    const item = try aggregate.temporary(c, "_item");
    const index = try aggregate.temporary(c, "_index");
    const label = try aggregate.temporary(c, "_search");
    const comparison = if (child == .string)
        try text.fmt(g, "std.mem.eql(u8, {s}, {s})", .{ item, needle })
    else if (kind == .includes and child.float())
        try text.fmt(g, "{s} == {s} or std.math.isNan({s}) and std.math.isNan({s})", .{
            item, needle, item, needle,
        })
    else
        try text.fmt(g, "{s} == {s}", .{ item, needle });
    const found = if (kind == .includes) "true" else try text.fmt(g, "@as(isize, @intCast({s}))", .{index});
    const header = if (kind == .includes)
        try text.fmt(g, "for ({s}) |{s}|", .{ source, item })
    else
        try text.fmt(g, "for ({s}, 0..) |{s}, {s}|", .{ source, item, index });
    const pattern = "{s}: {{\nconst {s}: {s} = {s};\n{s} {{\nif ({s}) break :{s} {s};\n}}\nbreak :{s} {s};\n}}";
    return .{
        .ty = if (kind == .includes) .boolean else .isize,
        .code = try text.fmt(g, pattern, .{
            label,
            needle,
            child.zig(g),
            value.code,
            header,
            comparison,
            label,
            found,
            label,
            if (kind == .includes) "false" else "@as(isize, -1)",
        }),
    };
}

fn slice(g: *Generator, c: *Context, loc: ast.Loc, args: []const *ast.Expr, source: []const u8, ty: Type) Error!Value {
    try arity(g, loc, args, 0, 2);
    var bounds = [_][]const u8{ "0", try text.fmt(g, "{s}.len", .{source}) };
    for (args, 0..) |arg, i| {
        const value = try expr.emit(g, c, arg, null, false);
        if (model.optionalChild(g, value.ty) != null) {
            return core.fail(g, arg.loc, "optional slice endpoint needs ?? or !", .{});
        }
        if (!value.ty.numeric()) return core.fail(g, arg.loc, "slice endpoint must be numeric", .{});
        bounds[i] = try text.fmt(g, "tsx.sliceIndex({s}, {s}.len)", .{ (try types.concrete(g, value)).code, source });
    }
    const start = try aggregate.temporary(c, "_start");
    const end = try aggregate.temporary(c, "_end");
    const label = try aggregate.temporary(c, "_slice");
    return .{ .ty = ty, .code = try text.fmt(g, "{s}: {{\nconst {s}: usize = {s};\nconst {s}: usize = {s};\n" ++
        "break :{s} {s}[{s}..@max({s}, {s})];\n}}", .{
        label, start, bounds[0], end, bounds[1], label, source, start, start, end,
    }) };
}

fn join(
    g: *Generator,
    c: *Context,
    loc: ast.Loc,
    args: []const *ast.Expr,
    source: []const u8,
    child: Type,
) Error!Value {
    try arity(g, loc, args, 0, 1);
    if (child != .string) return core.fail(g, loc, "join needs a string array", .{});
    const separator = if (args.len == 1) (try expr.emit(g, c, args[0], .string, false)).code else "\",\"";
    return .{ .ty = .string, .code = try text.fmt(g, "tsx.join({s}, {s})", .{ source, separator }) };
}

fn concat(
    g: *Generator,
    c: *Context,
    args: []const *ast.Expr,
    source: []const u8,
    ty: Type,
    child: Type,
) Error!Value {
    var parts: std.ArrayList([]const u8) = .empty;
    try parts.append(g.a, source);
    for (args) |arg| {
        const context = switch (arg.data) {
            .array => ty,
            .object => child,
            else => null,
        };
        const value = try expr.emit(g, c, arg, context, false);
        if (value.ty == ty) {
            try parts.append(g.a, value.code);
        } else {
            const item = try types.coerce(g, value, child, arg.loc);
            try parts.append(g.a, try text.fmt(g, "&[_]{s}{{ {s}, }}", .{ child.zig(g), item.code }));
        }
    }
    return .{ .ty = ty, .code = try text.fmt(g, "tsx.concat({s}, &.{{{s}}})", .{
        child.zig(g), try std.mem.join(g.a, ", ", parts.items),
    }) };
}
