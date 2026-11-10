//! String calls use typed runtime helpers, preserving receiver/argument evaluation order.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const aggregate = @import("aggregate.zig");
const expr = @import("expr.zig");
const scope = @import("scope.zig");
const text = @import("zig_text.zig");

const supported = @import("../string_method_names.zig").supported;
const Method = struct {
    min: usize,
    max: usize,
    numeric: bool = false,
    result: enum { string, boolean, index, array, optional, number, optional_number } = .string,
};
const methods = std.StaticStringMap(Method).initComptime(.{
    .{ "includes", Method{ .min = 1, .max = 1, .result = .boolean } },
    .{ "startsWith", Method{ .min = 1, .max = 1, .result = .boolean } },
    .{ "endsWith", Method{ .min = 1, .max = 1, .result = .boolean } },
    .{ "indexOf", Method{ .min = 1, .max = 1, .result = .index } },
    .{ "lastIndexOf", Method{ .min = 1, .max = 1, .result = .index } },
    .{ "trim", Method{ .min = 0, .max = 0 } },
    .{ "trimStart", Method{ .min = 0, .max = 0 } },
    .{ "trimEnd", Method{ .min = 0, .max = 0 } },
    .{ "toLowerCase", Method{ .min = 0, .max = 0 } },
    .{ "toUpperCase", Method{ .min = 0, .max = 0 } },
    .{ "slice", Method{ .min = 1, .max = 2, .numeric = true } },
    .{ "substring", Method{ .min = 1, .max = 2, .numeric = true } },
    .{ "padStart", Method{ .min = 1, .max = 2, .numeric = true } },
    .{ "padEnd", Method{ .min = 1, .max = 2, .numeric = true } },
    .{ "repeat", Method{ .min = 1, .max = 1, .numeric = true } },
    .{ "replace", Method{ .min = 2, .max = 2 } },
    .{ "replaceAll", Method{ .min = 2, .max = 2 } },
    .{ "split", Method{ .min = 1, .max = 1, .result = .array } },
    .{ "charAt", Method{ .min = 1, .max = 1, .numeric = true } },
    .{ "at", Method{ .min = 1, .max = 1, .numeric = true, .result = .optional } },
    .{ "charCodeAt", Method{ .min = 1, .max = 1, .numeric = true, .result = .number } },
    .{ "codePointAt", Method{ .min = 1, .max = 1, .numeric = true, .result = .optional_number } },
});

/// Lower one known-string receiver, evaluating it and each argument exactly once in source order.
pub fn emit(
    g: *core.Generator,
    c: *scope.Context,
    loc: ast.Loc,
    call: ast.Call,
    receiver: types.Value,
) core.Error!types.Value {
    const name = call.callee.data.member.name;
    const method = methods.get(name.name) orelse return core.fail(
        g,
        name.loc,
        "unknown string method `{s}`; supported: {s}",
        .{ name.name, supported },
    );
    if (call.type_args.len != 0) return core.fail(g, loc, "string methods do not take type arguments", .{});
    if (c.constant) return core.fail(g, loc, "string methods cannot run in a top-level constant", .{});
    if (call.args.len < method.min or call.args.len > method.max) {
        return core.fail(g, loc, "string method `{s}` expects {d}..{d} arguments", .{
            name.name, method.min, method.max,
        });
    }
    const label = try aggregate.temporary(c, "_string");
    const source = try aggregate.temporary(c, "_text");
    var setup: std.ArrayList([]const u8) = .empty;
    var args: std.ArrayList([]const u8) = .empty;
    try setup.append(g.a, try text.fmt(g, "const {s}: []const u8 = {s};", .{ source, receiver.code }));
    try args.append(g.a, source);
    const endpoints = text.eq(name.name, "slice") or text.eq(name.name, "substring");
    for (call.args, 0..) |arg, i| {
        const numeric = method.numeric and (i == 0 or endpoints);
        const ty: types.Type = if (numeric) .f64 else .string;
        const value = try expr.emit(g, c, arg, ty, false);
        const temp = try aggregate.temporary(c, "_arg");
        try setup.append(g.a, try text.fmt(g, "const {s}: {s} = {s};", .{ temp, ty.zig(g), value.code }));
        try args.append(g.a, temp);
    }
    if (call.args.len == 1 and method.max == 2) {
        try args.append(g.a, if (endpoints) "null" else "\" \"");
    }
    const ty: types.Type = switch (method.result) {
        .string => .string,
        .boolean => .boolean,
        .index => .isize,
        .array => try model.array(g, .string),
        .optional => try model.optional(g, .string),
        .number => .f64,
        .optional_number => try model.optional(g, .f64),
    };
    return .{ .ty = ty, .code = try text.fmt(g, "{s}: {{\n{s}\nbreak :{s} tsx.strings.{s}({s});\n}}", .{
        label, try std.mem.join(g.a, "\n", setup.items), label, name.name, try std.mem.join(g.a, ", ", args.items),
    }) };
}
