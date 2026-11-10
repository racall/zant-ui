//! Typed JS built-ins, isolated from JSX and the general expression emitter.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const aggregate = @import("aggregate.zig");
const model = @import("model.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Value = types.Value;
const Type = types.Type;
const Error = core.Error;

const math_names = "floor, ceil, round, trunc, abs, sign, sqrt, cbrt, pow, exp, log, log2, log10, " ++
    "sin, cos, tan, atan, atan2, hypot, min, max, random, PI, E";
const number_names = "isNaN, isFinite, isInteger, MAX_SAFE_INTEGER, EPSILON";
const unary_names = [_][]const u8{
    "floor", "ceil", "round", "trunc", "abs", "sign", "sqrt", "cbrt",
    "exp",   "log",  "log2",  "log10", "sin", "cos",  "tan",  "atan",
};

pub fn shadowed(g: *Generator, c: *Context, name: []const u8) bool {
    if (scope.local(c, name) != null or scope.stateIndex(g, name) != null or scope.globalName(g, name)) return true;
    if (c.capture_source) |parent| if (shadowed(g, parent, name)) return true;
    if (g.current_component) |component| {
        for (component.destructure) |prop| if (text.eq(prop.name, name)) return true;
        if (component.destructure.len == 0 and text.eq(component.param, name)) return true;
    }
    return false;
}

fn objectName(g: *Generator, c: *Context, object: *ast.Expr) ?[]const u8 {
    if (object.data != .ident or shadowed(g, c, object.data.ident)) return null;
    const name = object.data.ident;
    inline for (.{ "Math", "Number", "Date", "performance" }) |known| if (text.eq(name, known)) return name;
    return null;
}

fn unknown(g: *Generator, object: []const u8, name: ast.Name) Error {
    const supported = if (text.eq(object, "Math")) math_names else if (text.eq(object, "Number"))
        number_names
    else
        "now (full Date objects are out of scope)";
    return core.fail(g, name.loc, "unknown {s} member `{s}`; supported: {s}", .{ object, name.name, supported });
}

/// Resolve numeric constants without treating the built-in object as a runtime value.
pub fn member(g: *Generator, c: *Context, m: ast.Member) Error!?Value {
    const object = objectName(g, c, m.object) orelse return null;
    const n = m.name.name;
    const code: ?[]const u8 = if (text.eq(object, "Math")) blk: {
        if (text.eq(n, "PI")) break :blk "@as(f64, std.math.pi)";
        if (text.eq(n, "E")) break :blk "@as(f64, std.math.e)";
        break :blk null;
    } else if (text.eq(object, "Number")) blk: {
        if (text.eq(n, "MAX_SAFE_INTEGER")) break :blk "@as(f64, 9007199254740991)";
        if (text.eq(n, "EPSILON")) break :blk "@as(f64, 0x1p-52)";
        break :blk null;
    } else null;
    if (code) |value| return .{ .ty = .f64, .code = value };
    return unknown(g, object, m.name);
}

fn arity(g: *Generator, loc: ast.Loc, call: ast.Call, min: usize, max: usize) Error!void {
    if (call.type_args.len != 0) return core.fail(g, loc, "built-ins do not take type arguments", .{});
    if (call.args.len < min or call.args.len > max) {
        return core.fail(g, loc, "built-in expects {d}..{d} arguments", .{ min, max });
    }
}

fn numeric(g: *Generator, value: Value, loc: ast.Loc) Error!void {
    if (!value.ty.numeric()) return core.fail(g, loc, "built-in requires a number", .{});
}

// Typed locals guarantee receiver-first, left-to-right, exactly-once evaluation.
const Call = struct {
    g: *Generator,
    c: *Context,
    setup: std.ArrayList([]const u8) = .empty,
    args: std.ArrayList([]const u8) = .empty,

    fn add(b: *Call, v: Value, ty: Type, loc: ast.Loc) Error!void {
        const value = try types.coerce(b.g, v, ty, loc);
        const name = try aggregate.temporary(b.c, "_arg");
        try b.setup.append(b.g.a, try text.fmt(b.g, "const {s}: {s} = {s};", .{
            name, ty.zig(b.g), if (ty.float() and text.eq(value.code, "-0")) "-0.0" else value.code,
        }));
        try b.args.append(b.g.a, name);
    }

    fn finish(b: *Call, ty: Type, code: []const u8) Error!Value {
        const label = try aggregate.temporary(b.c, "_builtin");
        return .{ .ty = ty, .code = try text.fmt(b.g, "{s}: {{\n{s}\nbreak :{s} {s};\n}}", .{
            label, try std.mem.join(b.g.a, "\n", b.setup.items), label, code,
        }) };
    }

    fn joined(b: *Call) Error![]const u8 {
        return std.mem.join(b.g.a, ", ", b.args.items);
    }
};

/// Dispatch globals, built-in object calls, and numeric prototype methods.
pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    if (call.callee.data == .ident) {
        const n = call.callee.data.ident;
        if (shadowed(g, c, n)) return null;
        if (text.eq(n, "Date")) return core.fail(g, loc, "full Date objects are out of scope; use Date.now()", .{});
        inline for (.{ "Number", "String", "parseInt", "parseFloat" }) |known| {
            if (text.eq(n, known)) return try global(g, c, loc, call, n);
        }
        return null;
    }
    if (call.callee.data != .member) return null;
    const m = call.callee.data.member;
    if (objectName(g, c, m.object)) |object| {
        if (text.eq(object, "Math")) return try math(g, c, loc, call);
        if (text.eq(object, "Number")) {
            inline for (.{ "isNaN", "isFinite", "isInteger" }) |name| {
                if (text.eq(m.name.name, name)) {
                    try arity(g, loc, call, 1, 1);
                    var b = Call{ .g = g, .c = c };
                    const v = try expr.emit(g, c, call.args[0], null, false);
                    try b.add(v, v.ty.concrete(), call.args[0].loc);
                    return try b.finish(.boolean, try text.fmt(g, "tsx.builtins.predicate(.{s}, {s})", .{
                        name, b.args.items[0],
                    }));
                }
            }
        } else if (text.eq(m.name.name, "now")) {
            try arity(g, loc, call, 0, 0);
            if (c.constant) return core.fail(g, loc, "clocks cannot run in a top-level constant", .{});
            return .{ .ty = .f64, .code = if (text.eq(object, "Date"))
                "tsx.builtins.dateNow()"
            else
                "tsx.builtins.performanceNow()" };
        }
        return unknown(g, object, m.name);
    }
    if (!text.eq(m.name.name, "toFixed") and !text.eq(m.name.name, "toString")) return null;
    const value = try expr.emit(g, c, m.object, null, false);
    if (model.optionalChild(g, value.ty) != null) {
        return core.fail(g, loc, "optional value needs ?? or ! before a numeric method", .{});
    }
    try numeric(g, value, m.object.loc);
    if (m.optional) return core.fail(g, loc, "optional numeric calls are not supported; use ?? or !", .{});
    const fixed = text.eq(m.name.name, "toFixed");
    try arity(g, loc, call, 0, if (fixed) 1 else 0);
    if (c.constant) return core.fail(g, loc, "numeric strings cannot run in a top-level constant", .{});
    var b = Call{ .g = g, .c = c };
    try b.add(value, if (fixed) .f64 else value.ty.concrete(), m.object.loc);
    if (fixed) {
        if (call.args.len == 1) {
            const arg = call.args[0];
            const digits = try expr.emit(g, c, arg, .f64, false);
            try b.add(digits, .f64, arg.loc);
        } else try b.args.append(g.a, "0");
    }
    return try b.finish(.string, try text.fmt(g, "tsx.builtins.{s}({s})", .{
        if (fixed) "toFixed" else "string", try b.joined(),
    }));
}

fn global(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call, name: []const u8) Error!Value {
    try arity(g, loc, call, 1, if (text.eq(name, "parseInt")) 2 else 1);
    var b = Call{ .g = g, .c = c };
    const v = try expr.emit(g, c, call.args[0], null, false);
    const conversion = text.eq(name, "Number") or text.eq(name, "String");
    if (conversion) {
        if (!v.ty.numeric() and v.ty != .string and v.ty != .boolean) {
            return core.fail(g, call.args[0].loc, "conversion requires a number, string, or boolean", .{});
        }
    }
    if (text.eq(name, "String") and c.constant) {
        return core.fail(g, loc, "String cannot run in a top-level constant", .{});
    }
    try b.add(v, if (conversion) v.ty.concrete() else .string, call.args[0].loc);
    if (call.args.len == 2) {
        const arg = call.args[1];
        try b.add(try expr.emit(g, c, arg, .f64, false), .f64, arg.loc);
    } else if (text.eq(name, "parseInt")) try b.args.append(g.a, "0");
    const helper = if (text.eq(name, "Number")) "number" else if (text.eq(name, "String")) "string" else name;
    return b.finish(if (text.eq(name, "String")) .string else .f64, try text.fmt(g, "tsx.builtins.{s}({s})", .{
        helper, try b.joined(),
    }));
}

fn math(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!Value {
    const m = call.callee.data.member;
    const name = m.name.name;
    const unary = for (unary_names) |n| {
        if (text.eq(name, n)) break true;
    } else false;
    const extrema = text.eq(name, "min") or text.eq(name, "max");
    const variadic = extrema or text.eq(name, "hypot");
    const random = text.eq(name, "random");
    const binary = text.eq(name, "pow") or text.eq(name, "atan2");
    if (!unary and !variadic and !random and !binary) return unknown(g, "Math", m.name);
    const min: usize = if (unary) 1 else if (binary) 2 else 0;
    try arity(g, loc, call, min, if (variadic) std.math.maxInt(usize) else min);
    if (random) {
        if (c.constant) return core.fail(g, loc, "random cannot run in a top-level constant", .{});
        return .{ .ty = .f64, .code = "tsx.builtins.random()" };
    }
    var values: std.ArrayList(Value) = .empty;
    var peer: Type = .int_literal;
    for (call.args) |arg| {
        const v = try expr.emit(g, c, arg, null, false);
        try numeric(g, v, arg.loc);
        peer = Type.numericPeer(peer, v.ty).?;
        try values.append(g.a, v);
    }
    const preserve = extrema or text.eq(name, "floor") or text.eq(name, "ceil") or
        text.eq(name, "round") or text.eq(name, "trunc") or text.eq(name, "abs");
    const ty: Type = if (preserve and peer.integer()) peer else .f64;
    var b = Call{ .g = g, .c = c };
    for (values.items, call.args) |v, arg| {
        try types.checkLiteral(g, arg, ty);
        try b.add(v, ty, arg.loc);
    }
    const args = try b.joined();
    const code = if (unary)
        try text.fmt(g, "tsx.builtins.unary(.{s}, {s})", .{ name, args })
    else if (extrema)
        try text.fmt(g, "tsx.builtins.extrema({s}, {s}, &.{{{s}}})", .{
            ty.zig(g), if (text.eq(name, "min")) "true" else "false", args,
        })
    else if (variadic)
        try text.fmt(g, "tsx.builtins.hypot(&.{{{s}}})", .{args})
    else
        try text.fmt(g, "tsx.builtins.{s}({s})", .{ name, args });
    return b.finish(ty, code);
}
