//! Render and handler statements, local mutation, setters, and functional updates.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const stmt = @This();
const model = @import("model.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Context = scope.Context;
const Error = core.Error;

pub fn statements(g: *Generator, c: *Context, ss: []const *ast.Stmt, top: bool) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    const mark = c.narrowed.items.len;
    defer @import("narrowing.zig").restore(c, mark);
    for (ss) |s| {
        try out.appendSlice(g.a, try stmt.statement(g, c, s, top));
        if (@import("loops.zig").exits(s)) break;
        // `if (!entry) return …;` narrows `entry` for the rest of the block
        if (s.data == .if_ and s.data.if_.else_ == null and leaves(s.data.if_.then)) {
            _ = try @import("narrowing.zig").push(g, c, s.data.if_.cond, false);
        }
    }
    return out.items;
}

/// The statement always leaves the block (return, break, continue).
fn leaves(s: *ast.Stmt) bool {
    const body: []const *ast.Stmt = if (s.data == .block) s.data.block else &.{s};
    if (stmt.returnsAll(body)) return true;
    return body.len > 0 and (body[body.len - 1].data == .break_ or body[body.len - 1].data == .continue_);
}

pub fn scoped(g: *Generator, c: *Context, s: *ast.Stmt) Error![]const u8 {
    const depth = c.active.items.len;
    defer c.active.shrinkRetainingCapacity(depth);
    return zig_text.fmt(
        g,
        "{{\n{s}}}",
        .{if (s.data == .block) try stmt.statements(g, c, s.data.block, false) else try stmt.statement(g, c, s, false)},
    );
}

pub fn statement(g: *Generator, c: *Context, s: *ast.Stmt, top: bool) Error![]const u8 {
    switch (s.data) {
        .for_, .for_of, .while_, .switch_, .break_, .continue_ => return @import("loops.zig").emit(g, c, s),
        .decl => |d| return declaration(g, c, s.loc, d, top),
        .func => {
            if (c.render and top) return "";
            return core.fail(g, s.loc, "nested functions are not supported yet (closures arrive in phase 2)", .{});
        },
        .ret => |e| return returnStatement(g, c, s.loc, e),
        .expr => |e| {
            if (core.hook(g, e)) {
                if (@import("app_hooks.zig").isAppHook(e.data.call.callee.data.ident)) {
                    return @import("app_hooks.zig").statement(g, c, e, top);
                }
                if (@import("desktop_hooks.zig").isHook(e.data.call.callee.data.ident)) {
                    const v = try @import("desktop_hooks.zig").value(g, c, e, top);
                    return zig_text.fmt(g, "_ = {s};\n", .{v.code});
                }
                if (!zig_text.eq(e.data.call.callee.data.ident, "useEffect")) {
                    return core.fail(g, e.loc, "keep the value of this hook: const x = …", .{});
                }
                return @import("hooks.zig").effect(g, c, e, top);
            }
            return stmt.expressionStatement(g, c, e);
        },
        .block => |ss| return block(g, c, ss),
        .if_ => |v| return ifStatement(g, c, v),
    }
}

/// Emit a typed local or skip a registered top-level hook or component function.
fn declaration(g: *Generator, c: *Context, loc: ast.Loc, d: ast.Decl, top: bool) Error![]const u8 {
    const e = d.init orelse return core.fail(g, loc, "local declarations need an initializer", .{});
    if (core.hook(g, e) and zig_text.eq(e.data.call.callee.data.ident, "useMemo")) {
        return @import("hooks.zig").memo(g, c, loc, d, top);
    }
    if (core.hook(g, e) and zig_text.eq(e.data.call.callee.data.ident, "useStore")) {
        return @import("stores.zig").use(g, c, loc, d, top);
    }
    if (core.hook(g, e) and @import("desktop_hooks.zig").isHook(e.data.call.callee.data.ident)) {
        if (d.pattern != .ident) return core.fail(g, loc, "const ok = {s}(…)", .{e.data.call.callee.data.ident});
        const v = try @import("desktop_hooks.zig").value(g, c, e, top);
        const b = try scope.bind(c, d.pattern.ident.name, v.ty, !d.is_const);
        b.init = v.code;
        return zig_text.fmt(g, "\x01{d}\x02", .{b.serial});
    }
    if (core.hook(g, e)) {
        if (c.render and top) return "";
        return core.fail(g, e.loc, "hooks must be called at the top level of the component", .{});
    }
    if (d.pattern != .ident) return core.fail(g, loc, "destructuring is only supported for useState", .{});
    if (e.data == .arrow) {
        if (c.render and top) return "";
        return core.fail(g, e.loc, "nested functions are not supported yet (closures arrive in phase 2)", .{});
    }
    const target = if (d.ty) |t| try types.typeRef(g, t, false) else null;
    var v = try expr.emit(g, c, e, target, false);
    v = try types.coerce(g, v, target orelse v.ty.concrete(), e.loc);
    if (v.ty == .void_ or v.ty == .null_) {
        return core.fail(g, e.loc, "local must have a value with a concrete type", .{});
    }
    const b = try scope.bind(c, d.pattern.ident.name, v.ty, !d.is_const);
    b.init = v.code;
    return zig_text.fmt(g, "\x01{d}\x02", .{b.serial});
}

/// Validate the return value against the surrounding render or handler result type.
fn returnStatement(g: *Generator, c: *Context, loc: ast.Loc, e: ?*ast.Expr) Error![]const u8 {
    if (e) |x| if (x.data == .arrow and c.effect_field != null) return @import("hooks.zig").cleanupReturn(g, c, x);
    if (e) |x| {
        if (c.return_label) |label| {
            if (c.return_values) |values| {
                const value = try expr.emit(g, c, x, null, false);
                const index = values.items.len;
                try values.append(g.a, .{ .value = value, .loc = x.loc });
                return zig_text.fmt(g, "break :{s} \x05{d}\x06;\n", .{ label, index });
            }
            const value = try expr.emit(g, c, x, c.ret, false);
            return zig_text.fmt(g, "break :{s} {s};\n", .{ label, value.code });
        }
        if (c.ret == .void_) {
            if (x.data != .call) return core.fail(g, x.loc, "void functions cannot return a value", .{});
            if (x.data.call.callee.data == .ident and scope.setterIndex(g, x.data.call.callee.data.ident) != null) {
                return zig_text.fmt(g, "{s}return;\n", .{try stmt.expressionStatement(g, c, x)});
            }
            const v = try expr.emit(g, c, x, .void_, false);
            return zig_text.fmt(g, "{s};\nreturn;\n", .{v.code});
        }
        const v = try expr.emit(g, c, x, c.ret, false);
        return zig_text.fmt(g, "return {s};\n", .{v.code});
    }
    if (c.return_label != null) return core.fail(g, loc, "array callbacks must return a value", .{});
    if (c.ret != .void_) return core.fail(g, loc, "return needs a value of type {s}", .{c.ret.zig(g)});
    return "return;\n";
}

/// Restore the enclosing lexical scope after emitting a nested block.
fn block(g: *Generator, c: *Context, ss: []const *ast.Stmt) Error![]const u8 {
    const depth = c.active.items.len;
    defer c.active.shrinkRetainingCapacity(depth);
    return zig_text.fmt(g, "{{\n{s}}}\n", .{try stmt.statements(g, c, ss, false)});
}

/// Emit boolean branches with a separate scope for each branch.
fn ifStatement(g: *Generator, c: *Context, v: ast.If) Error![]const u8 {
    const narrowing = @import("narrowing.zig");
    const cond = try expr.condition(g, c, v.cond);
    var mark = try narrowing.push(g, c, v.cond, true);
    const then = try stmt.scoped(g, c, v.then);
    narrowing.restore(c, mark);
    mark = try narrowing.push(g, c, v.cond, false);
    const otherwise = if (v.else_) |el| try zig_text.fmt(g, " else {s}", .{try stmt.scoped(g, c, el)}) else "";
    narrowing.restore(c, mark);
    return zig_text.fmt(g, "if ({s}) {s}{s}\n", .{ cond.code, then, otherwise });
}

pub fn expressionStatement(g: *Generator, c: *Context, e: *ast.Expr) Error![]const u8 {
    if (e.data == .call) {
        const call = e.data.call;
        if (call.callee.data == .ident) {
            if (scope.local(c, call.callee.data.ident) == null) {
                if (scope.setterIndex(g, call.callee.data.ident)) |s| return stmt.set(g, c, e.loc, s, call.args);
            }
        }
        return expr.callStatement(g, c, e);
    }
    if (e.data == .assign or e.data == .update) {
        if (try @import("hooks.zig").refWrite(g, c, e)) |code| return code;
        const target = if (e.data == .assign) e.data.assign.target else e.data.update.target;
        if (target.data != .ident) return core.fail(g, target.loc, "only let locals can be assigned", .{});
        const n = target.data.ident;
        const binding = scope.local(c, n) orelse {
            if (scope.stateIndex(g, n)) |i| {
                return core.fail(g, target.loc, "use {s}(…) to update state", .{g.states.items[i].setter});
            }
            return core.fail(g, target.loc, "unknown local `{s}`", .{n});
        };
        if (!binding.mutable) return core.fail(g, target.loc, "`{s}` is const; declare it with let to assign it", .{n});
        binding.assigned = true;
        // A write counts as Zig usage, but not as a TS read of state.
        if (e.data == .update) {
            if (!binding.ty.numeric()) return core.fail(g, e.loc, "++ and -- require a number", .{});
            return zig_text.fmt(
                g,
                "{s} {s}= 1;\n",
                .{ binding.zig, if (zig_text.eq(e.data.update.op, "++")) "+" else "-" },
            );
        }
        const as = e.data.assign;
        if (zig_text.eq(as.op, "=")) {
            const v = try expr.emit(g, c, as.value, binding.ty, false);
            return zig_text.fmt(g, "{s} = {s};\n", .{ binding.zig, v.code });
        }
        // Reuse the arithmetic checker so integer /= and %= retain Zig's
        // truncating semantics rather than accidentally emitting JS division.
        const lhs = try g.a.create(ast.Expr);
        lhs.* = .{ .loc = target.loc, .data = .{ .ident = n } };
        const b = ast.Binary{ .op = as.op[0..1], .lhs = lhs, .rhs = as.value };
        const v = try types.coerce(g, try expr.binary(g, c, e.loc, b, binding.ty, false), binding.ty, e.loc);
        return zig_text.fmt(g, "{s} = {s};\n", .{ binding.zig, v.code });
    }
    return core.fail(g, e.loc, "only calls, assignments, and updates are supported as expression statements", .{});
}

/// Ordinary setters use snapshots; a functional update reads the current state inside its own block.
pub fn set(g: *Generator, c: *Context, loc: ast.Loc, i: usize, args: []const *ast.Expr) Error![]const u8 {
    if (c.render) return core.fail(g, loc, "state can only be set in event handlers", .{});
    if (args.len != 1) return core.fail(g, loc, "state setter takes one argument", .{});
    const s = g.states.items[i];
    const field = try scope.stateField(g, s.name);
    const arg = args[0];
    if (arg.data == .arrow) {
        const f = arg.data.arrow;
        if (f.params.len != 1 or f.body != .expr) return core.fail(
            g,
            arg.loc,
            "functional update needs one parameter and an expression body",
            .{},
        );
        if (f.params[0].ty) |r| if (try types.typeRef(g, r, false) != s.ty) {
            return core.fail(g, r.loc, "functional update parameter must match the state type", .{});
        };
        const depth = c.active.items.len;
        defer c.active.shrinkRetainingCapacity(depth);
        const b = try scope.bind(c, f.params[0].name.name, s.ty, false);
        const current = if (model.owned(g, s.ty))
            try zig_text.fmt(g, "self.{s}.get()", .{field})
        else if (s.ty == .string)
            try zig_text.fmt(g, "ui.frameString(self.{s}.get())", .{field})
        else
            try zig_text.fmt(g, "self.{s}", .{field});
        b.init = current;
        const v = try expr.emit(g, c, f.body.expr, s.ty, false);
        const update = try stmt.stateWrite(g, field, s.ty, v.code);
        return zig_text.fmt(g, "{{\n\x01{d}\x02{s}\n}}\n", .{ b.serial, update });
    }
    const v = try expr.emit(g, c, arg, s.ty, false);
    return zig_text.fmt(g, "{s}\n", .{try stmt.stateWrite(g, field, s.ty, v.code)});
}

pub fn stateWrite(g: *Generator, field: []const u8, t: Type, v: []const u8) Error![]const u8 {
    return if (t == .string or model.owned(g, t))
        zig_text.fmt(g, "self.{s}.set({s});", .{ field, v })
    else
        zig_text.fmt(g, "self.{s} = {s};", .{ field, v });
}

pub fn stmtReturns(s: *ast.Stmt) bool {
    return switch (s.data) {
        .ret => true,
        .switch_ => |v| @import("loops.zig").returns(v),
        .for_ => |v| (v.cond == null or (v.cond.?.data == .boolean and v.cond.?.data.boolean)) and
            !@import("loops.zig").jumps(v.body) and stmt.stmtReturns(v.body),
        .while_ => |v| (v.do_first or (v.cond.data == .boolean and v.cond.data.boolean)) and
            !@import("loops.zig").jumps(v.body) and stmt.stmtReturns(v.body),
        .block => |ss| stmt.returnsAll(ss),
        .if_ => |v| if (v.else_) |el| stmt.stmtReturns(v.then) and stmt.stmtReturns(el) else false,
        else => false,
    };
}

pub fn returnsAll(ss: []const *ast.Stmt) bool {
    for (ss) |s| {
        if (stmt.stmtReturns(s)) return true;
        if (@import("loops.zig").exits(s)) return false;
    }
    return false;
}
