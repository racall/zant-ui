//! Typed expressions, numeric operators, conditionals, strings, calls, and console.log.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @This();
const aggregate = @import("aggregate.zig");
const model = @import("model.zig");
const jsx = @import("jsx.zig");
const events = @import("events.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Value = types.Value;
const Context = scope.Context;
const Format = zig_text.Format;
const Error = core.Error;

const boolean_condition_message =
    "conditions must be boolean; compare the value explicitly (for example, count > 0)";

pub fn condition(g: *Generator, c: *Context, e: *ast.Expr) Error!Value {
    const v = try expr.emit(g, c, e, null, false);
    if (model.optionalChild(g, v.ty) != null) return core.fail(g, e.loc, "optional condition needs ?? or !", .{});
    if (v.ty != .boolean) return core.fail(g, e.loc, boolean_condition_message, .{});
    return v;
}

pub fn emit(g: *Generator, c: *Context, e: *ast.Expr, expected: ?Type, child: bool) Error!Value {
    if (expected) |t| try types.checkLiteral(g, e, t);
    const v: Value = switch (e.data) {
        .number => |n| number(n),
        .string => |s| .{ .ty = .string, .code = try zig_text.quote(g, s), .text = s },
        .boolean => |b| .{ .ty = .boolean, .code = if (b) "true" else "false" },
        .null_lit => .{ .ty = .null_, .code = "null" },
        .ident => |n| try scope.read(g, c, n, e.loc),
        .template => |t| try expr.template(g, c, t),
        .unary => |u| try unary(g, c, e.loc, u, expected),
        .binary => |b| try expr.binary(g, c, e.loc, b, expected, child),
        .cond => |cond| try conditional(g, c, e.loc, cond, expected, child),
        .member => |m| try member(g, c, m),
        .call => |call| try expr.callValue(g, c, e.loc, call, expected),
        .jsx => |j| try jsx.emit(g, c, j),
        .arrow => return unsupportedArrow(g, e.loc),
        .object => |properties| try aggregate.object(g, c, e.loc, properties, expected),
        .assign, .update => return core.fail(g, e.loc, "assignments and updates are only supported as statements", .{}),
        .non_null => |inner| try aggregate.nonNull(g, c, inner, expected, child),
        .as => |cast| try explicitCast(g, c, e.loc, cast, child),
        .array => |items| try aggregate.array(g, c, e.loc, items, expected),
        .index => |ix| try aggregate.index(g, c, e.loc, ix),
        .spread => return core.fail(g, e.loc, "this expression is not supported yet", .{}),
    };
    return if (expected) |t| types.coerce(g, v, t, e.loc) else v;
}

/// Keep a numeric literal contextual until a consumer determines its Zig type.
fn number(n: []const u8) Value {
    return .{
        .ty = if (types.floatLiteral(n)) .float_literal else .int_literal,
        .code = n,
        .inference = .literal,
    };
}

/// Reject arrows outside the supported handler, function, and update positions.
fn unsupportedArrow(g: *Generator, loc: ast.Loc) Error {
    return core.fail(
        g,
        loc,
        "arrow functions are only supported as component functions, event handlers, or functional updates",
        .{},
    );
}

/// Check boolean negation or numeric signs without changing the operand type.
pub fn unary(g: *Generator, c: *Context, loc: ast.Loc, u: ast.Unary, expected: ?Type) Error!Value {
    if (zig_text.eq(u.op, "!")) {
        const x = try expr.condition(g, c, u.operand);
        return .{
            .ty = .boolean,
            .code = try zig_text.fmt(g, "!{s}", .{try zig_text.operand(g, x, .unary, true)}),
            .precedence = .unary,
        };
    }
    const operand_context = if (u.operand.data != .number and types.numericLiterals(u.operand)) expected else null;
    var x = try expr.emit(g, c, u.operand, operand_context, false);
    if (x.inference == .result_location) x = try types.concrete(g, x);
    if (model.optionalChild(g, x.ty) != null) return core.fail(g, loc, "optional operand needs ?? or !", .{});
    if (!x.ty.numeric()) return core.fail(g, loc, "unary - requires a number", .{});
    if (zig_text.eq(u.op, "-") and x.ty.integer() and x.ty.zig(g)[0] == 'u') return core.fail(
        g,
        loc,
        "cannot negate an unsigned value; use a signed type",
        .{},
    );
    if (zig_text.eq(u.op, "+")) return x;
    return .{
        .ty = x.ty,
        .code = try zig_text.fmt(g, "-{s}", .{try zig_text.operand(g, x, .unary, true)}),
        .precedence = .unary,
        .inference = x.inference,
    };
}

/// Unify branches and retain whether the result needs a concrete Zig type.
pub fn conditional(g: *Generator, c: *Context, loc: ast.Loc, cond: ast.Cond, expected: ?Type, child: bool) Error!Value {
    const test_ = try expr.condition(g, c, cond.cond);
    var a = try expr.emit(g, c, cond.then, expected, child);
    var b = try expr.emit(g, c, cond.else_, expected, child);
    const t = expected orelse (try types.unify(g, a.ty, b.ty, loc)).concrete();
    a = try types.coerce(g, a, t, cond.then.loc);
    b = try types.coerce(g, b, t, cond.else_.loc);
    const literal_branches = a.inference == .literal and b.inference == .literal;
    const needs_type = a.inference == .result_location or b.inference == .result_location or literal_branches;
    return .{
        .ty = t,
        .code = try zig_text.fmt(g, "if ({s}) {s} else {s}", .{ test_.code, a.code, b.code }),
        .precedence = .conditional,
        .inference = if (needs_type) .result_location else .known,
    };
}

/// Lower the supported string length member while respecting suffix precedence.
pub fn member(g: *Generator, c: *Context, m: ast.Member) Error!Value {
    return aggregate.member(g, c, m);
}

/// Unify numeric peers, reject truthiness, and lower strings and optional JSX children.
pub fn binary(g: *Generator, c: *Context, loc: ast.Loc, b: ast.Binary, expected: ?Type, child: bool) Error!Value {
    if (zig_text.eq(b.op, "??")) return aggregate.coalesce(g, c, loc, b, expected);
    if (zig_text.eq(b.op, "**")) return core.fail(g, loc, "** is not supported yet", .{});
    if (zig_text.eq(b.op, "&&") or zig_text.eq(b.op, "||")) {
        const a = try expr.condition(g, c, b.lhs);
        const rhs = try expr.emit(g, c, b.rhs, null, child);
        if (child and zig_text.eq(b.op, "&&") and (rhs.ty == .node or rhs.ty == .optional_node)) return .{
            .ty = .optional_node,
            .code = try zig_text.fmt(g, "if ({s}) {s} else null", .{ a.code, rhs.code }),
            .precedence = .conditional,
        };
        if (child and zig_text.eq(b.op, "&&") and model.element(g, rhs.ty) != null) return .{
            .ty = rhs.ty,
            .code = try zig_text.fmt(g, "if ({s}) {s} else &.{{}}", .{ a.code, rhs.code }),
            .precedence = .conditional,
        };
        if (rhs.ty != .boolean) return core.fail(g, b.rhs.loc, boolean_condition_message, .{});
        return infix(g, a, if (zig_text.eq(b.op, "&&")) "and" else "or", rhs, .boolean);
    }
    const numeric_context = if (expected) |t| (if (t.numeric()) expected else null) else null;
    const lhs_context = if (types.numericLiterals(b.lhs)) types.numericContext(
        g,
        c,
        b.lhs,
        types.numericHint(g, c, b.rhs) orelse numeric_context,
    ) else null;
    var a = try expr.emit(g, c, b.lhs, lhs_context, false);
    const rhs_context = if (types.numericLiterals(b.rhs)) types.numericContext(
        g,
        c,
        b.rhs,
        if (a.ty.numeric() and !a.ty.literal()) a.ty else numeric_context,
    ) else null;
    var z = try expr.emit(g, c, b.rhs, rhs_context, false);
    if (equalityOperator(b.op)) {
        if (model.enumType(g, a.ty) and z.ty == .string) z = try types.coerce(g, z, a.ty, b.rhs.loc);
        if (model.enumType(g, z.ty) and a.ty == .string) a = try types.coerce(g, a, z.ty, b.lhs.loc);
        if (a.ty == .null_ or z.ty == .null_) {
            const other = if (a.ty == .null_) z else a;
            if (other.ty != .null_ and model.optionalChild(g, other.ty) == null) {
                return core.fail(g, loc, "null comparison needs an optional value", .{});
            }
            const negated = zig_text.eq(b.op, "!==") or zig_text.eq(b.op, "!=");
            const typed_null = (a.ty == .null_ and !zig_text.eq(a.code, "null")) or
                (z.ty == .null_ and !zig_text.eq(z.code, "null"));
            if (typed_null) return .{
                .ty = .boolean,
                .code = try zig_text.fmt(g, "{s}tsx.nullEqual({s}, {s}, {s})", .{
                    if (negated) "!" else "", if (a.ty == .null_) "true" else "false", a.code, z.code,
                }),
                .precedence = if (negated) .unary else .primary,
            };
            return infix(g, a, if (negated) "!=" else "==", z, .boolean);
        }
        if (model.optionalChild(g, a.ty) != null or model.optionalChild(g, z.ty) != null) {
            return core.fail(g, loc, "optional operand needs ?? or !", .{});
        }
        if (model.enumType(g, a.ty) and a.ty == z.ty) {
            return infix(g, a, if (zig_text.eq(b.op, "!==") or zig_text.eq(b.op, "!=")) "!=" else "==", z, .boolean);
        }
        if (model.info(g, a.ty) != null or model.info(g, z.ty) != null) {
            return core.fail(g, loc, "equality on structs and arrays is not supported; compare their fields", .{});
        }
    }
    if (zig_text.eq(b.op, "+") and (a.ty == .string or z.ty == .string)) {
        const format_ = try g.a.create(Format);
        format_.* = .{ .g = g, .compile_time = c.constant };
        try format_.value(a, loc);
        try format_.value(z, loc);
        return .{ .ty = .string, .code = try format_.finish(false), .formatted = format_ };
    }
    if (model.optionalChild(g, a.ty) != null or model.optionalChild(g, z.ty) != null) {
        return core.fail(g, loc, "optional operand needs ?? or !", .{});
    }
    const equality = equalityOperator(b.op);
    const not_equal = zig_text.eq(b.op, "!==") or zig_text.eq(b.op, "!=");
    if (equality and a.ty == .string and z.ty == .string) return .{
        .ty = .boolean,
        .code = try zig_text.fmt(g, "{s}std.mem.eql(u8, {s}, {s})", .{ if (not_equal) "!" else "", a.code, z.code }),
        .precedence = if (not_equal) .unary else .primary,
    };
    if (equality and a.ty == .boolean and z.ty == .boolean) {
        return infix(g, a, if (not_equal) "!=" else "==", z, .boolean);
    }
    if (!a.ty.numeric() or !z.ty.numeric()) return core.fail(
        g,
        loc,
        "operator `{s}` requires matching numbers{s}",
        .{ b.op, "" },
    );
    const t = (try types.unify(g, a.ty, z.ty, loc)).concrete();
    a = try types.coerce(g, a, t, b.lhs.loc);
    z = try types.coerce(g, z, t, b.rhs.loc);
    // A result location does not flow into binary operands. Conversions and
    // runtime conditionals need their own type; two float literals need one
    // typed peer to preserve finite precision and floating-point division.
    if (a.inference == .result_location) a = try types.concrete(g, a);
    if (z.inference == .result_location) z = try types.concrete(g, z);
    if (t.float() and a.inference == .literal and z.inference == .literal) a = try types.concrete(g, a);
    const comparison = equality or relationalOperator(b.op);
    const op = if (equality) (if (not_equal) "!=" else "==") else b.op;
    if (t.integer() and (zig_text.eq(op, "/") or zig_text.eq(op, "%"))) {
        return .{
            .ty = t,
            .code = try zig_text.fmt(
                g,
                "{s}({s}, {s})",
                .{ if (zig_text.eq(op, "/")) "@divTrunc" else "@rem", a.code, z.code },
            ),
            .inference = if (a.inference == .literal and z.inference == .literal) .literal else .known,
        };
    }
    if (zig_text.eq(op, "%")) return .{ .ty = t, .code = try zig_text.fmt(g, "@rem({s}, {s})", .{ a.code, z.code }) };
    return infix(g, a, op, z, if (comparison) .boolean else t);
}

const operator_precedence = std.StaticStringMap(zig_text.Precedence).initComptime(.{
    .{ "or", .logical_or },
    .{ "and", .logical_and },
    .{ "+", .sum },
    .{ "-", .sum },
    .{ "*", .product },
    .{ "/", .product },
});

/// Preserve the source grouping, including floating-point right operands and
/// non-associative comparisons, without wrapping complete initializers.
pub fn infix(g: *Generator, a: Value, op: []const u8, b: Value, t: Type) Error!Value {
    const precedence = operator_precedence.get(op) orelse .comparison;
    const lhs = try zig_text.operand(g, a, precedence, precedence == .comparison);
    const rhs = try zig_text.operand(g, b, precedence, precedence != .logical_and and precedence != .logical_or);
    return .{
        .ty = t,
        .code = try zig_text.fmt(g, "{s} {s} {s}", .{ lhs, op, rhs }),
        .precedence = precedence,
        .inference = if (t.integer() and a.inference == .literal and b.inference == .literal) .literal else .known,
    };
}

pub fn template(g: *Generator, c: *Context, t: ast.Template) Error!Value {
    if (t.exprs.len == 0) return .{ .ty = .string, .code = try zig_text.quote(g, t.quasis[0]), .text = t.quasis[0] };
    const f = try g.a.create(Format);
    f.* = .{ .g = g, .compile_time = c.constant };
    for (t.quasis, 0..) |s, i| {
        try f.text(s);
        if (i < t.exprs.len) try f.value(try expr.emit(g, c, t.exprs[i], null, false), t.exprs[i].loc);
    }
    return .{ .ty = .string, .code = try f.finish(false), .formatted = f };
}

pub fn callValue(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call, expected: ?Type) Error!Value {
    if (try @import("components.zig").tryCall(g, c, loc, call)) |value| return value;
    if (try @import("zig_imports.zig").tryCall(g, c, loc, call)) |value| return value;
    if (try @import("stores.zig").tryCall(g, c, loc, call)) |value| return value;
    if (try @import("windows.zig").tryCall(g, c, loc, call)) |value| return value;
    if (try @import("functions.zig").tryCall(g, c, loc, call)) |value| return value;
    if (try @import("zant_api.zig").tryCall(g, c, loc, call)) |value| return value;
    if (try @import("builtins.zig").tryCall(g, c, loc, call)) |value| return value;
    if (call.callee.data == .member) {
        return @import("array_methods.zig").emit(g, c, loc, call, expected);
    }
    if (call.callee.data != .ident) return core.fail(
        g,
        loc,
        "only component functions can be called here; console.log is a statement",
        .{},
    );
    const n = call.callee.data.ident;
    if (scope.local(c, n) == null and zig_text.eq(n, "useTheme")) {
        if (!c.render) return core.fail(g, loc, "useTheme() is read while rendering; capture the value in handlers", .{});
        if (call.args.len != 0) return core.fail(g, loc, "useTheme() takes no arguments", .{});
        return .{ .ty = try model.themeType(g), .code = "ui.currentTheme()" };
    }
    if (scope.local(c, n) != null) return core.fail(g, loc, "`{s}` is a local value, not a component function", .{n});
    if (core.hookName(g, n)) return core.fail(g, loc, "hooks must be called at the top level of the component", .{});
    if (scope.setterIndex(g, n) != null) return core.fail(
        g,
        loc,
        "{s}",
        .{if (c.render) "state can only be set in event handlers" else "setter calls must be statements"},
    );
    if (call.type_args.len != 0) return core.fail(g, loc, "type arguments are only supported on useState", .{});
    const i = scope.sourceMethod(g, n) orelse return core.fail(g, loc, "unknown function `{s}`", .{n});
    const m = g.methods.items[i];
    var receiver: []const u8 = "self";
    var target = try zig_text.fmt(g, "self.{s}", .{try zig_text.id(g, n)});
    if (m.inline_only) {
        // reads render locals: its own copy, capturing them through this handler
        const nested = try @import("captures.zig").nested(g, c, loc, i);
        receiver = nested.receiver;
        target = nested.target;
    }
    const f = m.func.?;
    if (call.args.len != f.params.len) return core.fail(g, loc, "`{s}` expects {d} arguments", .{ n, f.params.len });
    var args: std.ArrayList([]const u8) = .empty;
    if (m.inline_only) try args.append(g.a, receiver);
    for (call.args, f.params, 0..) |arg, p, index| {
        const t = try events.methodParamType(g, m, index, p);
        try args.append(g.a, (try expr.emit(g, c, arg, t, false)).code);
    }
    return .{
        .ty = m.ret,
        .code = try zig_text.fmt(g, "{s}({s})", .{ target, try std.mem.join(g.a, ", ", args.items) }),
    };
}

/// console.log is accepted only as a statement and emits space-separated values plus a newline.
pub fn callStatement(g: *Generator, c: *Context, e: *ast.Expr) Error![]const u8 {
    const call = e.data.call;
    if (call.callee.data == .member) {
        const m = call.callee.data.member;
        const console = m.object.data == .ident and zig_text.eq(m.object.data.ident, "console");
        if (console and zig_text.eq(m.name.name, "log")) {
            var f = Format{ .g = g };
            for (call.args, 0..) |arg, i| {
                if (i > 0) try f.text(" ");
                try f.value(try expr.emit(g, c, arg, null, false), arg.loc);
            }
            try f.text("\n");
            return zig_text.fmt(g, "{s};\n", .{try f.finish(true)});
        }
    }
    const v = try expr.emit(g, c, e, null, false);
    return zig_text.fmt(g, "{s}{s};\n", .{ if (v.ty == .void_) "" else "_ = ", v.code });
}

/// Recognize equality spellings before selecting string or numeric comparison.
fn equalityOperator(op: []const u8) bool {
    inline for (.{ "===", "!==", "==", "!=" }) |spelling| {
        if (zig_text.eq(op, spelling)) return true;
    }
    return false;
}

/// Recognize ordering comparisons without duplicating a long equality chain.
fn relationalOperator(op: []const u8) bool {
    inline for (.{ "<", ">", "<=", ">=" }) |spelling| {
        if (zig_text.eq(op, spelling)) return true;
    }
    return false;
}

/// Assertions change numeric representation explicitly, without enabling general arrays or optionals.
fn explicitCast(g: *Generator, c: *Context, loc: ast.Loc, cast: ast.As, child: bool) Error!Value {
    const value = try expr.emit(g, c, cast.expr, null, child);
    const target = try types.typeRef(g, cast.ty, false);
    return types.explicitCast(g, value, target, loc);
}
