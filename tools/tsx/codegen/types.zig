//! TSX types, literal inference, numeric peers, and the permitted implicit conversions.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @This();
const model = @import("model.zig");
const scope = @import("scope.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Format = zig_text.Format;
const Error = core.Error;

pub const compound_base: u32 = 256;

/// Compound tags index Generator.compounds; primitive tags keep their existing behavior.
pub const Type = enum(u32) {
    int_literal,
    float_literal,
    u8,
    u16,
    u32,
    u64,
    i8,
    i16,
    i32,
    i64,
    usize,
    isize,
    f32,
    f64,
    boolean,
    string,
    node,
    optional_node,
    null_,
    void_,
    _,
    pub fn integer(t: Type) bool {
        return switch (t) {
            .u8, .u16, .u32, .u64, .i8, .i16, .i32, .i64, .usize, .isize => true,
            else => false,
        };
    }

    pub fn float(t: Type) bool {
        return t == .f32 or t == .f64;
    }

    pub fn numeric(t: Type) bool {
        return t.integer() or t.float() or t.literal();
    }

    pub fn literal(t: Type) bool {
        return t == .int_literal or t == .float_literal;
    }

    pub fn numericPeer(a: Type, b: Type) ?Type {
        if (!a.numeric() or !b.numeric()) return null;
        if (a == b) return a;
        if (a == .int_literal) return b;
        if (b == .int_literal) return a;
        if (a == .float_literal) return if (b.float()) b else .f64;
        if (b == .float_literal) return if (a.float()) a else .f64;
        if (a.float() or b.float()) return if (a == .f64 or b == .f64) .f64 else .f32;
        return null;
    }

    pub fn concrete(t: Type) Type {
        return if (t.literal()) .f64 else t;
    }

    pub fn zig(t: Type, g: *Generator) []const u8 {
        return switch (t) {
            .boolean => "bool",
            .string => "[]const u8",
            .node => "ui.Node",
            .optional_node => "?ui.Node",
            // Zig's literal null type is comptime-only; null-only slices need a runtime representation.
            .null_ => "?u8",
            .void_ => "void",
            .int_literal, .float_literal => "f64",
            else => if (@backingInt(t) >= 256) model.info(g, t).?.zig else @tagName(t),
        };
    }
};

pub const Value = struct {
    ty: Type,
    code: []const u8,
    text: ?[]const u8 = null,
    formatted: ?*Format = null,
    precedence: zig_text.Precedence = .primary,
    inference: enum { known, literal, result_location } = .known,
};

pub fn typeRef(g: *Generator, ref: ast.TypeRef, allow_void: bool) Error!Type {
    if (ref.detail) |detail| {
        for (g.resolved_types.items) |resolved| if (resolved.detail == detail) return resolved.ty;
        const ty = try model.resolve(g, ref, null);
        try g.resolved_types.append(g.a, .{ .detail = detail, .ty = ty });
        return ty;
    }
    if (try model.named(g, ref.name)) |ty| return ty;
    // React element types: a prop holding an element (`children`, `icon`)
    inline for (.{ "JSX.Element", "React.ReactNode", "ReactNode", "React.ReactElement", "ReactElement" }) |name| {
        if (zig_text.eq(ref.name, name)) return .node;
    }
    if (zig_text.eq(ref.name, "number")) return .f64;
    if (zig_text.eq(ref.name, "boolean")) return .boolean;
    if (zig_text.eq(ref.name, "string")) return .string;
    if (allow_void and zig_text.eq(ref.name, "void")) return .void_;
    if (types.numericName(ref.name)) return std.meta.stringToEnum(Type, ref.name).?;
    return core.fail(g, ref.loc, "unsupported type `{s}`; use number, boolean, string, or a numeric type", .{ref.name});
}

/// Allow matching types, contextual literals, integer-to-float, and float-width conversions.
pub fn coerce(g: *Generator, v: Value, t: Type, loc: ast.Loc) Error!Value {
    if (v.ty == t) return v;
    if (t == .string and model.enumType(g, v.ty)) return .{
        .ty = .string,
        .code = try zig_text.fmt(g, "@tagName({s})", .{(try concrete(g, v)).code}),
    };
    if (model.optionalChild(g, t)) |child| {
        if (v.ty == .null_) return .{
            .ty = t,
            .code = if (zig_text.eq(v.code, "null")) "null" else try zig_text.fmt(g, "tsx.nullAs({s}, {s})", .{
                child.zig(g), v.code,
            }),
        };
        var result = try coerce(g, v, child, loc);
        result.ty = t;
        return result;
    }
    if (model.optionalChild(g, v.ty) != null) {
        return core.fail(g, loc, "optional value needs a fallback with ?? or a non-null assertion !", .{});
    }
    if (model.enumType(g, t) and v.ty == .string and v.text != null) {
        const members = model.info(g, t).?.members;
        for (members) |member| if (zig_text.eq(member, v.text.?)) return .{
            .ty = t,
            .code = try zig_text.fmt(g, ".{s}", .{try zig_text.id(g, member)}),
            .inference = .result_location,
        };
        return core.fail(g, loc, "unknown {s} member `{s}`; expected: {s}", .{
            t.zig(g), v.text.?, try std.mem.join(g.a, ", ", members),
        });
    }
    if ((v.ty == .int_literal and t.numeric()) or (v.ty == .float_literal and t.float())) {
        var result = v;
        result.ty = t;
        return result;
    }
    if (v.ty.integer() and t.float()) {
        return .{
            .ty = t,
            .code = try zig_text.fmt(g, "@floatFromInt({s})", .{(try concrete(g, v)).code}),
            .inference = .result_location,
        };
    }
    if (v.ty.float() and t.float()) {
        return .{
            .ty = t,
            .code = try zig_text.fmt(g, "@floatCast({s})", .{(try concrete(g, v)).code}),
            .inference = .result_location,
        };
    }
    if ((v.ty == .node or v.ty == .null_) and t == .optional_node) {
        var result = v;
        result.ty = t;
        return result;
    }
    if (v.ty.float() and t.integer() or v.ty == .float_literal and t.integer()) {
        return core.fail(
            g,
            loc,
            "cannot convert {s} to {s}; float-to-integer conversion is not implicit; use an integer value",
            .{ v.ty.zig(g), t.zig(g) },
        );
    }
    return core.fail(g, loc, "cannot convert {s} to {s}; use an explicit matching type", .{ v.ty.zig(g), t.zig(g) });
}

/// Explicit assertions allow numeric conversions that implicit coercion rejects.
pub fn explicitCast(g: *Generator, value: Value, target: Type, loc: ast.Loc) Error!Value {
    var source = value;
    if (source.ty.literal()) source = try coerce(g, source, source.ty.concrete(), loc);
    if (source.ty == target) return source;
    if (!source.ty.numeric() or !target.numeric()) {
        return core.fail(g, loc, "cannot cast {s} to {s}; only numeric conversions are supported", .{
            source.ty.zig(g),
            target.zig(g),
        });
    }
    const builtin = if (source.ty.integer())
        (if (target.integer()) "@intCast" else "@floatFromInt")
    else
        (if (target.integer()) "@intFromFloat" else "@floatCast");
    source = try concrete(g, source);
    return .{
        .ty = target,
        .code = try zig_text.fmt(g, "{s}({s})", .{ builtin, source.code }),
        .inference = .result_location,
    };
}

/// Arithmetic operands and generic tuples do not provide a Zig result location.
/// Only those consumers materialize a contextual cast; assignments and typed
/// arguments can keep the literal or conversion builtin bare.
pub fn concrete(g: *Generator, v: Value) Error!Value {
    if (v.inference == .known or (!v.ty.numeric() and !model.enumType(g, v.ty))) return v;
    var result = v;
    result.code = try zig_text.fmt(g, "@as({s}, {s})", .{ v.ty.zig(g), v.code });
    result.inference = .known;
    result.precedence = .primary;
    return result;
}

/// Generic formatting observes a comptime literal's precision. Keep a cast
/// only when printing that literal directly would differ from its TS number
/// type, or when runtime control flow needs a concrete result location.
pub fn formatValue(g: *Generator, v: Value) Error!Value {
    if (v.inference == .result_location) return concrete(g, v);
    if (v.inference != .literal or !v.ty.float()) return v;
    if (types.floatLiteral(v.code)) {
        const wide = std.fmt.parseFloat(f128, v.code) catch return concrete(g, v);
        const original = try zig_text.fmt(g, "{d}", .{wide});
        const rounded = if (v.ty == .f32)
            try zig_text.fmt(g, "{d}", .{@as(f32, @floatCast(wide))})
        else
            try zig_text.fmt(g, "{d}", .{@as(f64, @floatCast(wide))});
        if (zig_text.eq(original, rounded)) return v;
    } else {
        const integer = std.fmt.parseInt(i128, v.code, 0) catch return concrete(g, v);
        const original = try zig_text.fmt(g, "{d}", .{integer});
        const rounded = if (v.ty == .f32)
            try zig_text.fmt(g, "{d}", .{@as(f32, @floatFromInt(integer))})
        else
            try zig_text.fmt(g, "{d}", .{@as(f64, @floatFromInt(integer))});
        if (zig_text.eq(original, rounded)) return v;
    }
    return concrete(g, v);
}

pub fn unify(g: *Generator, a: Type, b: Type, loc: ast.Loc) Error!Type {
    if (a == b) return a;
    if (a.numeric() and b.numeric()) {
        if (Type.numericPeer(a, b)) |t| return t;
        return core.fail(
            g,
            loc,
            "different integer types {s} and {s}; use the same integer type",
            .{ a.zig(g), b.zig(g) },
        );
    }
    if (a == .null_) return model.optional(g, b);
    if (b == .null_) return model.optional(g, a);
    if (model.optionalChild(g, a)) |child| if (child == b) return a;
    if (model.optionalChild(g, b)) |child| if (child == a) return b;
    const a_node = a == .node or a == .optional_node or a == .null_;
    const b_node = b == .node or b == .optional_node or b == .null_;
    if (a_node and b_node) return .optional_node;
    return core.fail(g, loc, "incompatible types {s} and {s}", .{ a.zig(g), b.zig(g) });
}

/// Only literal-only numeric trees take peer context; typed integers must still unify.
pub fn numericHint(g: *Generator, c: *Context, e: *ast.Expr) ?Type {
    const t = types.expressionHint(g, c, e) orelse return null;
    return if (t.numeric() and !t.literal()) t else null;
}

pub fn expressionHint(g: *Generator, c: *Context, e: *ast.Expr) ?Type {
    const t: ?Type = switch (e.data) {
        .number => |n| if (types.floatLiteral(n)) .float_literal else .int_literal,
        .ident => |n| nameHint(g, c, n),
        .non_null => |inner| types.expressionHint(g, c, inner),
        .as => |cast| castHint(g, cast.ty),
        .call => |call| callHint(g, call),
        .unary => |u| if (zig_text.eq(u.op, "!")) null else types.expressionHint(g, c, u.operand),
        .binary => |b| binaryHint(g, c, b),
        .cond => |v| branchHint(g, c, v),
        .member => |m| valueHint(g, c, .{ .data = .{ .member = m }, .loc = e.loc }),
        .index => valueHint(g, c, e.*),
        else => null,
    };
    return if (t != null and t.?.numeric()) t else null;
}

pub fn numericContext(g: *Generator, c: *Context, e: *ast.Expr, candidate: ?Type) ?Type {
    const t = candidate orelse return null;
    // A decimal literal cannot adopt an integer operand's type. It makes
    // the arithmetic floating point, even when the other side is integer.
    const hint = types.expressionHint(g, c, e) orelse return t;
    return if (t.integer() and (hint.float() or hint == .float_literal)) null else t;
}

/// Reject an out-of-range integer literal at its TSX source position.
pub fn checkLiteral(g: *Generator, e: *ast.Expr, t: Type) Error!void {
    if (!t.integer()) return;
    const raw: []const u8 = switch (e.data) {
        .number => |n| n,
        .unary => |u| (try negativeLiteral(g, u)) orelse return,
        else => return,
    };
    if (types.floatLiteral(raw)) return;
    const n = std.fmt.parseInt(i128, raw, 0) catch return core.fail(
        g,
        e.loc,
        "integer literal does not fit {s}",
        .{t.zig(g)},
    );
    const bits: u7 = switch (t) {
        .u8, .i8 => 8,
        .u16, .i16 => 16,
        .u32, .i32 => 32,
        .u64, .i64 => 64,
        .usize, .isize => @bitSizeOf(usize),
        else => unreachable,
    };
    const signed = t.zig(g)[0] == 'i';
    const bound = @as(i128, 1) << (bits - @as(u7, if (signed) 1 else 0));
    if (n < (if (signed) -bound else 0) or n >= bound) return core.fail(
        g,
        e.loc,
        "integer literal does not fit {s}; choose a wider or signed type",
        .{t.zig(g)},
    );
}

pub fn numericName(n: []const u8) bool {
    inline for (.{ "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "usize", "isize", "f32", "f64" }) |name| {
        if (zig_text.eq(n, name)) return true;
    }
    return false;
}

pub fn floatLiteral(raw: []const u8) bool {
    const n = std.mem.trimStart(u8, raw, "+-");
    const hexadecimal = std.mem.startsWith(u8, n, "0x") or std.mem.startsWith(u8, n, "0X");
    return !hexadecimal and std.mem.indexOfAny(u8, n, ".eE") != null;
}

pub fn arithmetic(op: []const u8) bool {
    inline for (.{ "+", "-", "*", "/", "%" }) |spelling| {
        if (zig_text.eq(op, spelling)) return true;
    }
    return false;
}

pub fn numericLiterals(e: *ast.Expr) bool {
    return switch (e.data) {
        .number => true,
        .non_null => |inner| types.numericLiterals(inner),
        .unary => |u| !zig_text.eq(u.op, "!") and types.numericLiterals(u.operand),
        .binary => |b| types.arithmetic(b.op) and types.numericLiterals(b.lhs) and types.numericLiterals(b.rhs),
        .cond => |v| types.numericLiterals(v.then) and types.numericLiterals(v.else_),
        else => false,
    };
}

pub fn isLiteral(e: *ast.Expr) bool {
    return switch (e.data) {
        .number, .string, .boolean => true,
        .unary => |u| zig_text.eq(u.op, "-") and u.operand.data == .number,
        else => false,
    };
}

/// Resolve a local before consulting state, matching ordinary identifier reads.
fn nameHint(g: *Generator, c: *Context, name: []const u8) ?Type {
    if (scope.local(c, name)) |binding| return binding.ty;
    if (scope.stateIndex(g, name)) |index| return g.states.items[index].ty;
    for (g.constants.items) |constant| if (zig_text.eq(constant.name, name)) return constant.value.ty;
    return null;
}

/// Only a named component function has a supported call result type.
fn callHint(g: *Generator, call: ast.Call) ?Type {
    if (call.callee.data != .ident) return null;
    const index = scope.sourceMethod(g, call.callee.data.ident) orelse return null;
    return g.methods.items[index].ret;
}

/// Infer numeric peers only for supported arithmetic operators.
fn binaryHint(g: *Generator, c: *Context, b: ast.Binary) ?Type {
    if (!types.arithmetic(b.op)) return null;
    const lhs = types.expressionHint(g, c, b.lhs) orelse return null;
    const rhs = types.expressionHint(g, c, b.rhs) orelse return null;
    return Type.numericPeer(lhs, rhs);
}

/// Infer a conditional's numeric peer without evaluating its condition.
fn branchHint(g: *Generator, c: *Context, value: ast.Cond) ?Type {
    const then = types.expressionHint(g, c, value.then) orelse return null;
    const otherwise = types.expressionHint(g, c, value.else_) orelse return null;
    return Type.numericPeer(then, otherwise);
}

/// Recognize only the supported minus-number literal shape for range checks.
fn negativeLiteral(g: *Generator, unary: ast.Unary) Error!?[]const u8 {
    if (!zig_text.eq(unary.op, "-") or unary.operand.data != .number) return null;
    return try zig_text.fmt(g, "-{s}", .{unary.operand.data.number});
}

/// A numeric cast supplies the peer type even when its operand was untyped.
fn castHint(g: *Generator, ref: ast.TypeRef) ?Type {
    if (ref.detail != null) return null;
    for (g.type_aliases.items) |alias| if (zig_text.eq(ref.name, alias.name)) return alias.ty;
    if (zig_text.eq(ref.name, "number")) return .f64;
    if (types.numericName(ref.name)) return std.meta.stringToEnum(Type, ref.name);
    return null;
}

/// Inspect members and indexes without emitting expressions or registering captures.
fn valueHint(g: *Generator, c: *Context, e: ast.Expr) ?Type {
    return switch (e.data) {
        .ident => |name| nameHint(g, c, name),
        .non_null => |inner| if (valueHint(g, c, inner.*)) |ty| model.optionalChild(g, ty) orelse ty else null,
        .index => |ix| if (valueHint(g, c, ix.object.*)) |ty| model.element(g, ty) else null,
        .member => |member| memberHint(g, c, member),
        .call => |call| callHint(g, call),
        else => null,
    };
}

fn memberHint(g: *Generator, c: *Context, member: ast.Member) ?Type {
    const ty = valueHint(g, c, member.object.*) orelse return null;
    if (zig_text.eq(member.name.name, "length") and (ty == .string or model.element(g, ty) != null)) return .usize;
    const entry = model.info(g, ty) orelse return null;
    if (entry.kind == .object) for (entry.fields) |field| {
        if (zig_text.eq(member.name.name, field.name)) return field.ty;
    };
    return null;
}
