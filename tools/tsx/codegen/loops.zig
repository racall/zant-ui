//! Structured statement control flow. Targets stay lexical and never cross callback boundaries.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const scope = @import("scope.zig");
const stmt = @import("stmt.zig");
const expr = @import("expr.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const aggregate = @import("aggregate.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Error = core.Error;

/// Emit a loop, switch, or jump using the surrounding function's bindings and return target.
pub fn emit(g: *Generator, c: *Context, s: *ast.Stmt) Error![]const u8 {
    if (s.data == .break_ or s.data == .continue_) {
        const is_break = s.data == .break_;
        const label = (if (is_break) c.break_label else c.continue_label) orelse return core.fail(
            g,
            s.loc,
            "{s}",
            .{if (is_break)
                "break is only allowed inside a loop or switch"
            else
                "continue is only allowed inside a loop"},
        );
        return text.fmt(g, "{s} :{s};\n", .{ if (is_break) "break" else "continue", label });
    }
    if (c.render and c.return_label == null) return core.fail(
        g,
        s.loc,
        "loops run in handlers and functions; use `.map` / `.filter` to render lists",
        .{},
    );
    const depth = c.active.items.len;
    const previous_break = c.break_label;
    const previous_continue = c.continue_label;
    defer {
        c.active.shrinkRetainingCapacity(depth);
        c.break_label = previous_break;
        c.continue_label = previous_continue;
    }
    const label = try aggregate.temporary(c, if (s.data == .switch_) "_switch" else "_loop");
    c.break_label = label;
    if (s.data == .switch_) return switchStatement(g, c, s.data.switch_, label);
    c.continue_label = label;
    return switch (s.data) {
        .for_ => |v| forStatement(g, c, v, label),
        .for_of => |v| forOf(g, c, v, label),
        .while_ => |v| whileStatement(g, c, v, label),
        else => unreachable,
    };
}

fn forStatement(g: *Generator, c: *Context, v: ast.For, label: []const u8) Error![]const u8 {
    const init = if (v.init) |s| try stmt.statement(g, c, s, false) else "";
    const cond = if (v.cond) |e| (try expr.condition(g, c, e)).code else "true";
    const update = if (v.update) |e| try stmt.expressionStatement(g, c, e) else "";
    const body = try stmt.scoped(g, c, v.body);
    const prefix = try labelPrefix(g, label, body);
    const step = if (v.update != null) try text.fmt(g, " : ({{\n{s}}})", .{update}) else "";
    return text.fmt(g, "{{\n{s}{s}while ({s}){s} {s}\n}}\n", .{ init, prefix, cond, step, body });
}

fn forOf(g: *Generator, c: *Context, v: ast.ForOf, label: []const u8) Error![]const u8 {
    var iterable = try expr.emit(g, c, v.iterable, null, false);
    if (iterable.ty == .string) {
        // by code point, like JS: `for (const ch of name)`
        iterable = .{ .ty = try model.array(g, .string), .code = try text.fmt(g, "tsx.strings.chars({s})", .{iterable.code}) };
    }
    const element = model.element(g, iterable.ty) orelse return core.fail(
        g,
        v.iterable.loc,
        "for...of requires an array or a string",
        .{},
    );
    const source = try aggregate.temporary(c, "_items");
    const item = try aggregate.temporary(c, "_item");
    const binding = try scope.bind(c, v.name.name, element, !v.is_const);
    binding.init = item;
    const body = try stmt.scoped(g, c, v.body);
    return text.fmt(g, "{{\nconst {s}: {s} = {s};\n{s}for ({s}) |{s}| {{\n\x01{d}\x02{s}\n}}\n}}\n", .{
        source, iterable.ty.zig(g), iterable.code, try labelPrefix(g, label, body), source, item, binding.serial, body,
    });
}

fn whileStatement(g: *Generator, c: *Context, v: ast.While, label: []const u8) Error![]const u8 {
    const cond = try expr.condition(g, c, v.cond);
    const body = try stmt.scoped(g, c, v.body);
    const prefix = try labelPrefix(g, label, body);
    if (!v.do_first) return text.fmt(g, "{s}while ({s}) {s}\n", .{ prefix, cond.code, body });
    // The continue expression also runs on `continue`, so a post-test loop always checks its condition.
    const first = try aggregate.temporary(c, "_first");
    return text.fmt(g, "{{\nvar {s} = true;\n{s}while ({s} or ({s})) : ({s} = false) {s}\n}}\n", .{
        first, prefix, first, cond.code, first, body,
    });
}

fn switchStatement(g: *Generator, c: *Context, v: ast.Switch, label: []const u8) Error![]const u8 {
    var value = try expr.emit(g, c, v.value, null, false);
    value = try types.coerce(g, value, value.ty.concrete(), v.value.loc);
    if (!value.ty.numeric() and value.ty != .string and value.ty != .boolean and !model.enumType(g, value.ty)) {
        return core.fail(g, v.value.loc, "switch requires a number, string, boolean or string-literal union", .{});
    }
    const discriminant = try aggregate.temporary(c, "_value");
    const default_label = if (groupedDefault(v)) try aggregate.temporary(c, "_default") else null;
    var branches: std.ArrayList(u8) = .empty;
    var fallback: ?[]const u8 = null;
    var start: usize = 0;
    for (v.cases, 0..) |case, i| {
        if (case.body.len > 0 and i + 1 < v.cases.len and !endsCase(case.body)) {
            return core.fail(g, case.loc, "add `break`; falling through to the next case is not supported", .{});
        }
        if (case.body.len == 0 and i + 1 < v.cases.len) continue;
        var conditions: std.ArrayList([]const u8) = .empty;
        var is_default = false;
        for (v.cases[start .. i + 1]) |grouped| {
            if (grouped.value) |test_expr| {
                const test_value = try expr.emit(g, c, test_expr, value.ty, false);
                const code = if (value.ty == .string)
                    try text.fmt(g, "std.mem.eql(u8, {s}, {s})", .{ discriminant, test_value.code })
                else
                    try text.fmt(g, "({s} == {s})", .{ discriminant, test_value.code });
                try conditions.append(g.a, code);
            } else is_default = true;
        }
        const depth = c.active.items.len;
        const body = try stmt.statements(g, c, case.body, false);
        c.active.shrinkRetainingCapacity(depth);
        const end = if (default_label != null and !is_default and !endsCase(case.body))
            try text.fmt(g, "break :{s};\n", .{label})
        else
            "";
        const block = try text.fmt(g, "{{\n{s}{s}}}", .{ body, end });
        if (is_default) fallback = block;
        if (conditions.items.len > 0) {
            try branches.appendSlice(g.a, try text.fmt(g, "{s}if ({s}) {s}", .{
                if (branches.items.len > 0) " else " else "",
                try std.mem.join(g.a, " or ", conditions.items),
                if (is_default and default_label != null)
                    try text.fmt(g, "{{ break :{s}; }}", .{default_label.?})
                else
                    block,
            }));
        }
        start = i + 1;
    }
    if (default_label) |target| {
        // A grouped default body must appear only once (locals and captures have one lexical binding).
        const search = try text.fmt(g, "{s}: {{\n{s}\n}}\n", .{ target, branches.items });
        branches = .empty;
        try branches.appendSlice(g.a, search);
    }
    if (fallback) |block| {
        if (branches.items.len > 0 and default_label == null) try branches.appendSlice(g.a, " else ");
        try branches.appendSlice(g.a, block);
    }
    // Even an empty switch evaluates its discriminant once.
    const discard = if (v.cases.len == 0 or branches.items.len == 0 or !hasTests(v))
        try text.fmt(g, "_ = {s};\n", .{discriminant})
    else
        "";
    return text.fmt(g, "{s}{{\nconst {s}: {s} = {s};\n{s}{s}\n}}\n", .{
        try labelPrefix(g, label, branches.items), discriminant, value.ty.zig(g), value.code, discard, branches.items,
    });
}

fn groupedDefault(v: ast.Switch) bool {
    var has_default = false;
    var has_value = false;
    for (v.cases, 0..) |case, i| {
        if (case.value == null) has_default = true else has_value = true;
        if (case.body.len == 0 and i + 1 < v.cases.len) continue;
        if (has_default and has_value) return true;
        has_default = false;
        has_value = false;
    }
    return false;
}

fn hasTests(v: ast.Switch) bool {
    for (v.cases) |case| if (case.value != null) return true;
    return false;
}

fn labelPrefix(g: *Generator, label: []const u8, body: []const u8) Error![]const u8 {
    // Labels are optional in Zig, but unused labels are errors. Ignore label-like text in strings.
    const source = try g.a.allocSentinel(u8, body.len, 0);
    @memcpy(source, body);
    // Declaration and inferred-return placeholders are expanded later by scope.finish/callbacks.emit.
    // Keep them from making the tokenizer skip a jump on the same generated line.
    for (source) |*byte| {
        if (byte.* >= 1 and byte.* <= 6) byte.* = ' ';
    }
    var tokens = std.zig.Tokenizer.init(source);
    while (true) {
        const token = tokens.next();
        if (token.tag == .eof) return "";
        if (token.tag != .keyword_break and token.tag != .keyword_continue) continue;
        if (tokens.next().tag != .colon) continue;
        const target = tokens.next();
        if (target.tag == .identifier and text.eq(source[target.loc.start..target.loc.end], label)) {
            return text.fmt(g, "{s}: ", .{label});
        }
    }
}

/// Whether this statement always exits the current statement sequence.
pub fn exits(s: *const ast.Stmt) bool {
    return switch (s.data) {
        .ret, .break_, .continue_ => true,
        .block => |ss| endsCase(ss),
        .if_ => |v| if (v.else_) |el| exits(v.then) and exits(el) else false,
        else => stmt.stmtReturns(@constCast(s)),
    };
}

fn endsCase(ss: []const *ast.Stmt) bool {
    for (ss) |s| if (exits(s)) return true;
    return false;
}

/// A switch returns on every path only with a default and a returning body for each group.
pub fn returns(v: ast.Switch) bool {
    var has_default = false;
    for (v.cases, 0..) |case, i| {
        if (case.value == null) has_default = true;
        if (case.body.len == 0 and i + 1 < v.cases.len) continue;
        if (!stmt.returnsAll(case.body) or sequenceJumps(case.body, true)) return false;
    }
    return has_default;
}

/// A possible break/continue escaping the body prevents treating a loop as always returning.
pub fn jumps(s: *const ast.Stmt) bool {
    return escapingJump(s, true);
}

fn escapingJump(s: *const ast.Stmt, include_break: bool) bool {
    return switch (s.data) {
        .break_ => include_break,
        .continue_ => true,
        .block => |ss| sequenceJumps(ss, include_break),
        .if_ => |v| escapingJump(v.then, include_break) or
            (if (v.else_) |el| escapingJump(el, include_break) else false),
        .switch_ => |v| for (v.cases) |case| {
            // A switch consumes its own breaks, but can continue an enclosing loop.
            if (sequenceJumps(case.body, false)) break true;
        } else false,
        // Nested loops consume both kinds of jump. Functions have their own boundary.
        else => false,
    };
}

fn sequenceJumps(ss: []const *ast.Stmt, include_break: bool) bool {
    for (ss) |s| {
        if (escapingJump(s, include_break)) return true;
        if (exits(s)) break;
    }
    return false;
}
