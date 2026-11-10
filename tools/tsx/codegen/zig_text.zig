//! Zig identifiers and strings, format text escaping, and CSS color literals.
const std = @import("std");
const ast = @import("../ast.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const Context = scope.Context;
const Value = types.Value;
const core = @import("codegen.zig");
const zig_text = @This();
const Generator = core.Generator;
const Error = core.Error;

/// Zig binding strength, from a conditional expression to a primary value.
pub const Precedence = enum(u8) {
    conditional,
    logical_or,
    logical_and,
    comparison,
    sum,
    product,
    unary,
    suffix,
    primary,
};

/// Parenthesize only operands whose binding strength would change the parse.
pub fn operand(g: *Generator, v: Value, parent: Precedence, equal: bool) Error![]const u8 {
    if (@backingInt(v.precedence) < @backingInt(parent) or (equal and v.precedence == parent)) {
        return fmt(g, "({s})", .{v.code});
    }
    return v.code;
}

pub fn fmt(g: *Generator, comptime f: []const u8, args: anytype) Error![]const u8 {
    return std.fmt.allocPrint(g.a, f, args);
}

pub fn id(g: *Generator, n: []const u8) Error![]const u8 {
    return zig_text.fmt(g, "{f}", .{std.zig.fmtId(n)});
}

/// Escape Zig string bytes while preserving UTF-8 text.
pub fn quote(g: *Generator, s: []const u8) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    try out.append(g.a, '"');
    for (s) |ch| switch (ch) {
        '"' => try out.appendSlice(g.a, "\\\""),
        '\\' => try out.appendSlice(g.a, "\\\\"),
        '\n' => try out.appendSlice(g.a, "\\n"),
        '\r' => try out.appendSlice(g.a, "\\r"),
        '\t' => try out.appendSlice(g.a, "\\t"),
        0...8, 11, 12, 14...31, 127 => try out.appendSlice(g.a, try zig_text.fmt(g, "\\x{x:0>2}", .{ch})),
        else => try out.append(g.a, ch),
    };
    try out.append(g.a, '"');
    return out.items;
}

/// Convert CSS RGB/RGBA literals into ARGB, or type-check numeric color expressions.
pub fn color(g: *Generator, c: *Context, e: *ast.Expr) Error![]const u8 {
    if (e.data == .cond) {
        const v = e.data.cond;
        return zig_text.fmt(
            g,
            "if ({s}) {s} else {s}",
            .{
                (try expr.condition(g, c, v.cond)).code,
                try zig_text.color(g, c, v.then),
                try zig_text.color(g, c, v.else_),
            },
        );
    }
    if (e.data == .string) {
        const s = e.data.string;
        if (s.len == 0 or s[0] != '#' or (s.len != 4 and s.len != 7 and s.len != 9)) {
            return core.fail(g, e.loc, "color must be #rgb, #rrggbb, or #rrggbbaa", .{});
        }
        const n = std.fmt.parseInt(u32, s[1..], 16) catch return core.fail(g, e.loc, "invalid hexadecimal color", .{});
        const argb: u32 = switch (s.len) {
            4 => 0xff000000 | ((n >> 8) * 17 << 16) | (((n >> 4) & 15) * 17 << 8) | ((n & 15) * 17),
            7 => 0xff000000 | n,
            9 => (n >> 8) | ((n & 255) << 24),
            else => unreachable,
        };
        return zig_text.fmt(g, "0x{x:0>8}", .{argb});
    }
    return (try expr.emit(g, c, e, .u32, false)).code;
}

pub fn eq(a: []const u8, b: []const u8) bool {
    return std.mem.eql(u8, a, b);
}

/// Keep literal text separate from the pattern: braces are doubled only for
/// a format call, while a substitution-free string remains ordinary text.
pub const Format = struct {
    g: *Generator,
    compile_time: bool = false,
    literal: std.ArrayList(u8) = .empty,
    pattern: std.ArrayList(u8) = .empty,
    args: std.ArrayList([]const u8) = .empty,
    pub fn text(f: *Format, s: []const u8) Error!void {
        try f.literal.appendSlice(f.g.a, s);
        for (s) |ch| {
            try f.pattern.append(f.g.a, ch);
            if (ch == '{' or ch == '}') try f.pattern.append(f.g.a, ch);
        }
    }

    pub fn value(f: *Format, raw: Value, loc: ast.Loc) Error!void {
        if (@import("model.zig").optionalChild(f.g, raw.ty) != null) {
            return core.fail(f.g, loc, "optional text value needs ?? or !", .{});
        }
        if (raw.text) |text_| return f.text(text_);
        if (raw.formatted) |other| {
            try f.literal.appendSlice(f.g.a, other.literal.items);
            try f.pattern.appendSlice(f.g.a, other.pattern.items);
            try f.args.appendSlice(f.g.a, other.args.items);
            return;
        }
        const v = try types.formatValue(f.g, try types.coerce(f.g, raw, raw.ty.concrete(), loc));
        const spec = switch (v.ty) {
            .string => "{s}",
            .boolean => "{}",
            else => if (@import("model.zig").enumType(f.g, v.ty))
                "{s}"
            else if (v.ty.numeric())
                "{d}"
            else
                return core.fail(
                    f.g,
                    loc,
                    "text substitutions must be strings, numbers, or booleans",
                    .{},
                ),
        };
        try f.pattern.appendSlice(f.g.a, spec);
        try f.args.append(f.g.a, if (@import("model.zig").enumType(f.g, v.ty))
            try zig_text.fmt(f.g, "@tagName({s})", .{(try types.concrete(f.g, v)).code})
        else
            v.code);
    }

    pub fn expression(f: *Format, c: *Context, e: *ast.Expr) Error!void {
        if (e.data == .string) return f.text(e.data.string);
        if (e.data == .template) {
            const t = e.data.template;
            for (t.quasis, 0..) |s, i| {
                try f.text(s);
                if (i < t.exprs.len) try f.expression(c, t.exprs[i]);
            }
            return;
        }
        try f.value(try expr.emit(f.g, c, e, null, false), e.loc);
    }

    pub fn finish(f: *Format, log: bool) Error![]const u8 {
        if (!log and f.args.items.len == 0) return zig_text.quote(f.g, f.literal.items);
        // a lone string needs no formatting (and no frame allocation)
        if (!log and f.args.items.len == 1 and std.mem.eql(u8, f.pattern.items, "{s}")) return f.args.items[0];
        const args = if (f.args.items.len == 0)
            ".{}"
        else
            try zig_text.fmt(f.g, ".{{{s}}}", .{try std.mem.join(f.g.a, ", ", f.args.items)});
        const callee = if (log) "std.debug.print" else if (f.compile_time) "std.fmt.comptimePrint" else "tsx.fmt";
        return zig_text.fmt(
            f.g,
            "{s}({s}, {s})",
            .{ callee, try zig_text.quote(f.g, f.pattern.items), args },
        );
    }
};
