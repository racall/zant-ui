//! `useRef`, `useMemo` and `useEffect` (docs/tsx.md, "第三阶段"). Like state,
//! their storage is a field of the root struct or of a child's `_Local` slot.
//!
//! - `const r = useRef(0)`: a field read and written through `r.current`;
//!   writes don't snapshot and don't need a setter (handlers read the live
//!   value).
//! - `const v = useMemo(() => f(a), [a, b])`: a `tsx.Memo` field; the body
//!   runs inline only when the dependency list changed (by value).
//! - `useEffect(() => { ... }, [a])`: a method queued with `ui.effect` after
//!   the build when the list changed (`[]`: once on mount; no list: after
//!   every build).
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const stmt = @import("stmt.zig");
const captures = @import("captures.zig");
const callbacks = @import("callbacks.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

/// A hidden field of the component (memo cache, effect deps).
pub const Field = struct {
    name: []const u8,
    zig_type: []const u8,
};

pub const Effect = struct {
    call: *const ast.Expr,
    method: usize,
    /// Has `return () => …` (a `tsx.Effect` field keeps the cleanup).
    cleanup: bool = false,
};

/// A cleanup arrow and its method.
pub const Cleanup = struct {
    arrow: *const ast.Expr,
    method: usize,
};

/// `useRef` / `useMemo` / `useEffect` (never renamed).
pub fn extraHook(name: []const u8) bool {
    return text.eq(name, "useRef") or text.eq(name, "useMemo") or text.eq(name, "useEffect");
}

pub fn callName(e: *const ast.Expr) ?[]const u8 {
    if (e.data != .call or e.data.call.callee.data != .ident) return null;
    return e.data.call.callee.data.ident;
}

// ── useRef ────────────────────────────────────────────────────────────

/// `const r = useRef<T>(initial)`: a state entry of kind `.ref`.
pub fn collectRef(g: *Generator, loc: ast.Loc, d: ast.Decl, call: ast.Call) Error!void {
    if (d.pattern != .ident) return core.fail(g, loc, "useRef returns one object: const ref = useRef(…)", .{});
    if (!d.is_const) return core.fail(g, loc, "declare useRef with const", .{});
    if (call.args.len != 1 or call.type_args.len > 1) {
        return core.fail(g, loc, "useRef takes one literal initial value and at most one type argument", .{});
    }
    const init = call.args[0];
    const literal = types.isLiteral(init) or init.data == .array or init.data == .object or init.data == .null_lit;
    if (!literal) return core.fail(g, init.loc, "useRef initial value must be a literal", .{});
    var c = Context{ .g = g, .render = true, .ret = .node, .constant = true };
    const target: ?Type = if (call.type_args.len == 1) try types.typeRef(g, call.type_args[0], false) else null;
    var v = try expr.emit(g, &c, init, target, false);
    v = try types.coerce(g, v, target orelse v.ty.concrete(), init.loc);
    const n = d.pattern.ident.name;
    if (scope.stateIndex(g, n) != null or scope.setterIndex(g, n) != null) {
        return core.fail(g, loc, "duplicate state or ref name; rename `{s}`", .{n});
    }
    try g.states.append(g.a, .{ .name = n, .setter = "", .ty = v.ty, .initial = v.code, .kind = .ref });
}

fn refIndex(g: *Generator, c: *Context, e: *const ast.Expr) ?usize {
    if (e.data != .ident or scope.local(c, e.data.ident) != null) return null;
    const i = scope.stateIndex(g, e.data.ident) orelse return null;
    return if (g.states.items[i].kind == .ref) i else null;
}

/// `ref.current` (live value, in render and in handlers), or null when `m`
/// is not a ref access.
pub fn refRead(g: *Generator, c: *Context, m: ast.Member) Error!?Value {
    const i = refIndex(g, c, m.object) orelse return null;
    if (!text.eq(m.name.name, "current")) return core.fail(g, m.name.loc, "a ref only has `.current`", .{});
    const s = g.states.items[i];
    const field = try scope.stateField(g, s.name);
    const code = if (model.owned(g, s.ty))
        try text.fmt(g, "self.{s}.get()", .{field})
    else if (s.ty == .string)
        // a later write frees the old text; reads keep a frame copy
        try text.fmt(g, "ui.frameString(self.{s}.get())", .{field})
    else
        try text.fmt(g, "self.{s}", .{field});
    return .{ .ty = s.ty, .code = code };
}

/// `ref.current = v`, `ref.current += v`, `ref.current++` in handlers and
/// effects; null when the target is not a ref.
pub fn refWrite(g: *Generator, c: *Context, e: *ast.Expr) Error!?[]const u8 {
    const target = if (e.data == .assign) e.data.assign.target else e.data.update.target;
    if (target.data != .member) return null;
    const m = target.data.member;
    const i = refIndex(g, c, m.object) orelse return null;
    if (!text.eq(m.name.name, "current")) return core.fail(g, m.name.loc, "a ref only has `.current`", .{});
    if (c.render) {
        return core.fail(g, e.loc, "refs are written in event handlers and effects, not while rendering", .{});
    }
    const s = g.states.items[i];
    const field = try scope.stateField(g, s.name);
    if (e.data == .update) {
        if (!s.ty.numeric()) return core.fail(g, e.loc, "++ and -- require a number", .{});
        const op = if (text.eq(e.data.update.op, "++")) "+" else "-";
        return try text.fmt(g, "self.{s} {s}= 1;\n", .{ field, op });
    }
    const as = e.data.assign;
    const value = if (text.eq(as.op, "=")) try expr.emit(g, c, as.value, s.ty, false) else blk: {
        const b = ast.Binary{ .op = as.op[0 .. as.op.len - 1], .lhs = target, .rhs = as.value };
        break :blk try types.coerce(g, try expr.binary(g, c, e.loc, b, s.ty, false), s.ty, e.loc);
    };
    return try text.fmt(g, "{s}\n", .{try stmt.stateWrite(g, field, s.ty, value.code)});
}

/// A bare `ref` used as a value.
pub fn bareRef(g: *Generator, i: usize, loc: ast.Loc) Error {
    return core.fail(g, loc, "read and write a ref through `{s}.current`", .{g.states.items[i].name});
}

// ── dependency lists ──────────────────────────────────────────────────

const DepList = struct {
    /// `.{ a, b }` and its tuple type `struct { A, B }`.
    value: []const u8,
    ty: []const u8,
};

fn depList(g: *Generator, c: *Context, e: *ast.Expr) Error!DepList {
    if (e.data != .array) return core.fail(g, e.loc, "the dependency list must be an array literal: [a, b]", .{});
    var values: std.ArrayList([]const u8) = .empty;
    var tys: std.ArrayList([]const u8) = .empty;
    for (e.data.array) |item| {
        if (item.data == .spread) return core.fail(g, item.loc, "list each dependency", .{});
        var v = try expr.emit(g, c, item, null, false);
        v = try types.coerce(g, v, v.ty.concrete(), item.loc);
        if (v.ty == .node or v.ty == .optional_node or v.ty == .void_ or v.ty == .null_) {
            return core.fail(g, item.loc, "dependencies are values (numbers, strings, arrays, objects)", .{});
        }
        if (model.info(g, v.ty)) |entry| if (entry.kind == .callback) {
            return core.fail(g, item.loc, "functions can't be dependencies", .{});
        };
        try values.append(g.a, v.code);
        try tys.append(g.a, v.ty.zig(g));
    }
    return .{
        .value = try text.fmt(g, ".{{ {s} }}", .{try std.mem.join(g.a, ", ", values.items)}),
        .ty = try text.fmt(g, "struct {{ {s} }}", .{try std.mem.join(g.a, ", ", tys.items)}),
    };
}

fn addField(g: *Generator, base: []const u8, zig_type: []const u8) Error![]const u8 {
    g.counter += 1;
    const name = try scope.uniqueMethod(g, try text.fmt(g, "{s}{d}", .{ base, g.counter }));
    try g.hook_fields.append(g.a, .{ .name = name, .zig_type = zig_type });
    return name;
}

// ── useMemo ───────────────────────────────────────────────────────────

/// `const v = useMemo(() => …, [deps])` as a render local.
pub fn memo(g: *Generator, c: *Context, loc: ast.Loc, d: ast.Decl, top: bool) Error![]const u8 {
    const call = d.init.?.data.call;
    if (!c.render or !top) return core.fail(g, loc, "hooks must be called at the top level of the component", .{});
    if (d.pattern != .ident) return core.fail(g, loc, "const value = useMemo(() => …, [deps])", .{});
    if (call.args.len != 2 or call.args[0].data != .arrow or call.args[0].data.arrow.params.len != 0) {
        return core.fail(g, loc, "useMemo takes a function without parameters and a dependency list", .{});
    }
    if (call.type_args.len > 1) return core.fail(g, loc, "useMemo takes at most one type argument", .{});
    const annotation: ?Type = if (call.type_args.len == 1)
        try types.typeRef(g, call.type_args[0], false)
    else if (d.ty) |t| try types.typeRef(g, t, false) else null;
    const deps = try depList(g, c, call.args[1]);
    const label = try @import("aggregate.zig").temporary(c, "_memo");
    const computed = try callbacks.emit(g, c, call.args[0], &.{}, annotation);
    const ty = computed.ty;
    if (ty == .node or ty == .optional_node or ty == .void_) {
        return core.fail(g, loc, "useMemo caches values; elements are rebuilt every frame", .{});
    }
    const field = try addField(g, "_memo", try text.fmt(g, "tsx.Memo({s}, {s})", .{ ty.zig(g), deps.ty }));
    const slot = try text.fmt(g, "self.{s}", .{try scope.stateField(g, field)});
    const b = try scope.bind(c, d.pattern.ident.name, ty, false);
    const block = "{s}: {{\nif ({s}.hit({s})) |_cached| break :{s} _cached;\nbreak :{s} {s}.store({s});\n}}";
    b.init = try text.fmt(g, block, .{ label, slot, deps.value, label, label, slot, computed.code });
    return text.fmt(g, "\x01{d}\x02", .{b.serial});
}

// ── useEffect ─────────────────────────────────────────────────────────

/// Registers the effect body as a method (before handlers are scanned, so
/// its captures and nested handlers resolve like an event arrow's).
pub fn collectEffect(g: *Generator, e: *ast.Expr) Error!void {
    const call = e.data.call;
    if (call.args.len < 1 or call.args.len > 2 or call.args[0].data != .arrow) {
        return core.fail(g, e.loc, "useEffect takes a function and an optional dependency list", .{});
    }
    const f = call.args[0].data.arrow;
    if (f.params.len != 0) return core.fail(g, f.loc, "the effect function takes no parameters", .{});
    const cleanups = g.cleanups.items.len;
    if (f.body == .block) for (f.body.block) |s| try collectCleanups(g, s);
    g.counter += 1;
    try g.effects.append(g.a, .{
        .call = e,
        .method = g.methods.items.len,
        .cleanup = g.cleanups.items.len > cleanups,
    });
    try g.methods.append(g.a, .{
        .name = try scope.uniqueMethod(g, try text.fmt(g, "_effect{d}", .{g.counter})),
        .func = f,
        .action = .action,
        .capture_name = try scope.uniqueMethod(g, try text.fmt(g, "_EffectCapture{d}", .{g.counter})),
    });
}

/// `return () => { … }` in an effect body (also inside `if`s).
fn collectCleanups(g: *Generator, s: *const ast.Stmt) Error!void {
    switch (s.data) {
        .ret => |r| if (r) |value| if (value.data == .arrow) {
            const f = value.data.arrow;
            if (f.params.len != 0) return core.fail(g, f.loc, "the cleanup function takes no parameters", .{});
            g.counter += 1;
            try g.cleanups.append(g.a, .{ .arrow = value, .method = g.methods.items.len });
            try g.methods.append(g.a, .{
                .name = try scope.uniqueMethod(g, try text.fmt(g, "_cleanup{d}", .{g.counter})),
                .func = f,
                .action = .action,
                .cleanup = true,
                .capture_name = try scope.uniqueMethod(g, try text.fmt(g, "_CleanupCapture{d}", .{g.counter})),
            });
        },
        .if_ => |v| {
            try collectCleanups(g, v.then);
            if (v.else_) |el| try collectCleanups(g, el);
        },
        .block => |ss| for (ss) |child| try collectCleanups(g, child),
        else => {},
    }
}

/// `return () => …` inside an effect: keep the cleanup in the effect's field.
pub fn cleanupReturn(g: *Generator, c: *Context, arrow: *ast.Expr) Error![]const u8 {
    const info = for (g.cleanups.items) |item| {
        if (item.arrow == arrow) break item;
    } else return core.fail(g, arrow.loc, "only the effect itself can return a cleanup function", .{});
    try captures.prepare(g, c, info.method);
    const m = g.methods.items[info.method];
    const receiver = if (m.captures.items.len > 0) try captures.value(g, m) else "self";
    return text.fmt(g, "self.{s}.setCleanup({s}, {s}.{s}, {s});\nreturn;\n", .{
        c.effect_field.?,
        receiver,
        try text.id(g, g.name),
        try text.id(g, m.name),
        if (g.current_component != null) "true" else "false",
    });
}

/// The statement queuing the effect when its dependency list changed.
pub fn effect(g: *Generator, c: *Context, e: *ast.Expr, top: bool) Error![]const u8 {
    if (!c.render or !top) return core.fail(g, e.loc, "hooks must be called at the top level of the component", .{});
    const info = for (g.effects.items) |item| {
        if (item.call == e) break item;
    } else unreachable; // registered by `collectEffect`
    const call = e.data.call;
    const deps = if (call.args.len == 2) try depList(g, c, call.args[1]) else null;
    // an effect with a cleanup keeps it in a `tsx.Effect` (deps included)
    const effect_field = if (info.cleanup) try addField(g, "_effect", try text.fmt(g, "tsx.Effect({s})", .{
        if (deps) |d| d.ty else "struct {}",
    })) else null;
    if (effect_field) |field| g.methods.items[info.method].effect_field = try scope.stateField(g, field);
    try captures.prepare(g, c, info.method);
    if (changesState(g, g.methods.items[info.method].func.?.body)) {
        // ask for another build first (an early `return` can't skip it)
        const code = g.methods.items[info.method].emitted.?;
        const open = std.mem.indexOf(u8, code, "{\n").? + 2;
        g.methods.items[info.method].emitted = try text.fmt(g, "{s}ui.requestRebuild();\n{s}", .{
            code[0..open], code[open..],
        });
    }
    const m = g.methods.items[info.method];
    const receiver = if (m.captures.items.len > 0) try captures.value(g, m) else "self";
    const action = try text.fmt(g, "ui.action({s}, {s}.{s})", .{
        receiver, try text.id(g, g.name), try text.id(g, m.name),
    });
    if (effect_field) |field| {
        const slot = try scope.stateField(g, field);
        const queue = try text.fmt(g, "self.{s}.queue({s});\n", .{ slot, action });
        if (deps) |d| return text.fmt(g, "if (self.{s}.changed({s})) {{\n{s}}}\n", .{ slot, d.value, queue });
        return queue;
    }
    const queue = try text.fmt(g, "ui.effect({s});\n", .{action});
    const d = deps orelse return queue;
    const field = try addField(g, "_deps", try text.fmt(g, "tsx.Deps({s})", .{d.ty}));
    const slot = try scope.stateField(g, field);
    return text.fmt(g, "if (self.{s}.changed({s})) {{\n{s}}}\n", .{ slot, d.value, queue });
}

/// Whether an effect body may change state: it calls a setter, a component
/// function, a callback prop or an imported function (ref writes and array
/// methods don't count).
fn changesState(g: *Generator, body: ast.Body) bool {
    return switch (body) {
        .expr => |e| exprChanges(g, e),
        .block => |ss| for (ss) |s| {
            if (stmtChanges(g, s)) break true;
        } else false,
    };
}

fn stmtChanges(g: *Generator, s: *const ast.Stmt) bool {
    return switch (s.data) {
        .decl => |d| if (d.init) |e| exprChanges(g, e) else false,
        .func => false,
        .ret => |e| if (e) |v| exprChanges(g, v) else false,
        .if_ => |v| exprChanges(g, v.cond) or stmtChanges(g, v.then) or
            (if (v.else_) |el| stmtChanges(g, el) else false),
        .expr => |e| exprChanges(g, e),
        .block => |ss| for (ss) |child| {
            if (stmtChanges(g, child)) break true;
        } else false,
    };
}

fn exprChanges(g: *Generator, e: *const ast.Expr) bool {
    return switch (e.data) {
        .call => |c| blk: {
            switch (c.callee.data) {
                // console.log and array methods are pure
                .member => |m| if (m.object.data == .ident and !text.eq(m.object.data.ident, "console")) {
                    const props = if (g.current_component) |comp| comp.param else "";
                    if (text.eq(m.object.data.ident, props)) break :blk true;
                },
                .ident => break :blk true,
                else => {},
            }
            if (exprChanges(g, c.callee)) break :blk true;
            for (c.args) |a| if (exprChanges(g, a)) break :blk true;
            break :blk false;
        },
        .template => |t| for (t.exprs) |x| {
            if (exprChanges(g, x)) break true;
        } else false,
        .unary => |u| exprChanges(g, u.operand),
        .binary => |b| exprChanges(g, b.lhs) or exprChanges(g, b.rhs),
        .cond => |c| exprChanges(g, c.cond) or exprChanges(g, c.then) or exprChanges(g, c.else_),
        .member => |m| exprChanges(g, m.object),
        .arrow => |f| changesState(g, f.body),
        .object => |props| for (props) |p| {
            if (exprChanges(g, p.value)) break true;
        } else false,
        .array => |items| for (items) |x| {
            if (exprChanges(g, x)) break true;
        } else false,
        .spread, .non_null => |x| exprChanges(g, x),
        .index => |i| exprChanges(g, i.object) or exprChanges(g, i.index),
        .as => |a| exprChanges(g, a.expr),
        .assign => |a| exprChanges(g, a.value),
        .update, .number, .string, .boolean, .null_lit, .ident, .jsx => false,
    };
}
