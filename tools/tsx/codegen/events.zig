//! Event signatures, inferred parameters, generated handlers, and reusable setter adapters.
const std = @import("std");
const captures = @import("captures.zig");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const stmt = @import("stmt.zig");
const events = @This();
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Type = types.Type;
const Error = core.Error;

pub const Event = struct {
    expr: *ast.Expr,
    kind: tags.Kind,
    method: usize,
};

pub const Method = struct {
    name: []const u8,
    func: ?*ast.Function = null,
    action: ?tags.Kind = null,
    setter: ?usize = null,
    ret: Type = .void_,
    user: bool = false,
    capture_name: ?[]const u8 = null,
    captures: std.ArrayList(captures.Capture) = .empty,
    emitted: ?[]const u8 = null,
    /// Passed as a child component's callback prop: the parameter types of
    /// that prop (unannotated arrow params take them; missing ones become `_`).
    callback_params: ?[]const Type = null,
    /// An effect with a cleanup: its `tsx.Effect` field (`return () => …`
    /// stores the cleanup there).
    effect_field: ?[]const u8 = null,
    /// An effect's cleanup function (runs later; can't read props).
    cleanup: bool = false,
};

/// Register event methods before local allocation, so names and inferred signatures are stable.
pub fn scanExpr(g: *Generator, e: *ast.Expr) Error!void {
    switch (e.data) {
        .jsx => |j| try scanJsx(g, j),
        .binary => |b| {
            try events.scanExpr(g, b.lhs);
            try events.scanExpr(g, b.rhs);
        },
        .unary => |u| try events.scanExpr(g, u.operand),
        .non_null => |inner| try events.scanExpr(g, inner),
        .as => |cast| try events.scanExpr(g, cast.expr),
        .index => |ix| {
            try events.scanExpr(g, ix.object);
            try events.scanExpr(g, ix.index);
        },
        .member => |member| try events.scanExpr(g, member.object),
        .spread => |inner| try events.scanExpr(g, inner),
        .array => |items| for (items) |item| try events.scanExpr(g, item),
        .cond => |v| try scanConditional(g, v),
        .call => |v| {
            for (v.args) |arg| try events.scanExpr(g, arg);
        },
        .template => |v| {
            for (v.exprs) |x| try events.scanExpr(g, x);
        },
        .arrow => |f| try events.scanBody(g, f.body),
        .assign => |v| try events.scanExpr(g, v.value),
        .object => |ps| {
            for (ps) |p| try events.scanExpr(g, p.value);
        },
        else => {},
    }
}

/// Register attribute callbacks before recursively scanning JSX children.
fn scanJsx(g: *Generator, j: *ast.Jsx) Error!void {
    if (@import("components.zig").find(g, j.name)) |comp| try @import("components.zig").scan(g, j, comp);
    if (zig_text.eq(j.name, "List")) return @import("list.zig").scan(g, j);
    if (tags.find(j.name)) |tag| for (j.attrs) |attr| {
        if (attr.value) |value| {
            if (zig_text.eq(attr.name.name, "style") and value.data == .object) {
                for (value.data.object) |property| {
                    if (property.spread) continue;
                    if (tags.findProp(tag, property.key.name)) |prop| {
                        try scanProperty(g, prop, property.value, property.key.loc);
                    } else try events.scanExpr(g, property.value);
                }
            } else if (tags.findProp(tag, attr.name.name)) |prop| {
                try scanProperty(g, prop, value, attr.name.loc);
            } else try events.scanExpr(g, value);
        }
    };
    for (j.children) |child| switch (child) {
        .element => |el| {
            const temp = try g.a.create(ast.Expr);
            temp.* = .{ .loc = el.loc, .data = .{ .jsx = el } };
            try events.scanExpr(g, temp);
        },
        .expr => |x| if (x) |v| {
            try events.scanExpr(g, v);
        },
        .text => {},
    };
}

pub fn scanBody(g: *Generator, body: ast.Body) Error!void {
    switch (body) {
        .expr => |e| try events.scanExpr(g, e),
        .block => |ss| for (ss) |s| try events.scanStmt(g, s),
    }
}

pub fn scanStmt(g: *Generator, s: *ast.Stmt) Error!void {
    switch (s.data) {
        .for_ => |v| {
            if (v.init) |init| try scanStmt(g, init);
            if (v.cond) |cond| try scanExpr(g, cond);
            if (v.update) |update| try scanExpr(g, update);
            try scanStmt(g, v.body);
        },
        .for_of => |v| {
            try scanExpr(g, v.iterable);
            try scanStmt(g, v.body);
        },
        .while_ => |v| {
            try scanExpr(g, v.cond);
            try scanStmt(g, v.body);
        },
        .switch_ => |v| {
            try scanExpr(g, v.value);
            for (v.cases) |case| {
                if (case.value) |value| try scanExpr(g, value);
                for (case.body) |child| try scanStmt(g, child);
            }
        },
        .break_, .continue_ => {},
        .decl => |d| {
            if (d.init) |e| try events.scanExpr(g, e);
        },
        .func => |f| try events.scanBody(g, f.body),
        .ret => |e| {
            if (e) |x| try events.scanExpr(g, x);
        },
        .expr => |e| try events.scanExpr(g, e),
        .block => |ss| {
            for (ss) |x| try events.scanStmt(g, x);
        },
        .if_ => |v| {
            try events.scanExpr(g, v.cond);
            try events.scanStmt(g, v.then);
            if (v.else_) |x| try events.scanStmt(g, x);
        },
    }
}

pub fn signature(g: *Generator, m: Method, kind: tags.Kind, loc: ast.Loc, inline_: bool) Error!void {
    const f = m.func.?;
    const count: usize = if (kind == .action) 0 else 1;
    if (m.ret != .void_ or f.params.len > count or (!inline_ and f.params.len != count)) return core.fail(
        g,
        loc,
        "handler must have signature {s}",
        .{events.actionSignature(kind)},
    );
    for (f.params) |p| if (p.ty) |ref| {
        if (try types.typeRef(g, ref, false) != events.actionType(kind)) return core.fail(
            g,
            p.name.loc,
            "handler must have signature {s}",
            .{events.actionSignature(kind)},
        );
    };
}

/// Validate the action signature and reuse one adapter per setter and action kind.
pub fn event(g: *Generator, e: *ast.Expr, prop: *const tags.Prop, loc: ast.Loc) Error!void {
    for (g.events.items) |ev| if (ev.expr == e and ev.kind == prop.kind) return;
    // a child component's own callback prop, forwarded (`onClick={onSave}`)
    if (try @import("components.zig").forwardedProp(g, e, prop.kind, loc)) return;
    var i: usize = undefined;
    if (e.data == .ident) {
        const n = e.data.ident;
        if (scope.setterIndex(g, n)) |s| {
            const t = g.states.items[s].ty;
            const valid = switch (prop.kind) {
                .text_action => t == .string,
                .bool_action => t == .boolean,
                .index_action => t.integer() or t.float(),
                else => false,
            };
            if (!valid) return core.fail(
                g,
                e.loc,
                "setter `{s}` does not match {s}",
                .{ n, events.actionSignature(prop.kind) },
            );
            for (g.methods.items, 0..) |m, k| {
                if (m.setter == s and m.action == prop.kind) {
                    i = k;
                    break;
                }
            } else {
                i = g.methods.items.len;
                const name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_{s}", .{n}));
                try g.methods.append(g.a, .{ .name = name, .setter = s, .action = prop.kind });
            }
        } else if (scope.sourceMethod(g, n) == null and @import("functions.zig").find(g, n) != null) {
            // a plain function: handled as `(v) => fn(v)`
            i = try registerArrow(g, prop, try functionArrow(g, e, prop.kind));
        } else {
            i = scope.sourceMethod(g, n) orelse return core.fail(
                g,
                e.loc,
                "event handler must be a component function, plain function, setter, or inline arrow",
                .{},
            );
            const m = g.methods.items[i];
            try events.signature(g, m, prop.kind, loc, false);
            if (m.action) |previous| {
                if (previous != prop.kind) return core.fail(
                    g,
                    e.loc,
                    "handler is used with incompatible action signatures",
                    .{},
                );
            }
            g.methods.items[i].action = prop.kind;
        }
    } else if (e.data == .arrow) {
        i = try registerArrow(g, prop, e);
    } else return core.fail(
        g,
        e.loc,
        "event handler must be a component function, plain function, setter, or inline arrow",
        .{},
    );
    try g.events.append(g.a, .{ .expr = e, .kind = prop.kind, .method = i });
}

pub fn eventValue(g: *Generator, c: *scope.Context, e: *ast.Expr, kind: tags.Kind) Error![]const u8 {
    if (try @import("components.zig").forwardedProp(g, e, kind, e.loc)) {
        return (try @import("expr.zig").emit(g, c, e, null, false)).code;
    }
    for (g.events.items) |ev| if (ev.expr == e and ev.kind == kind) {
        if (e.data == .arrow) try captures.prepare(g, c, ev.method);
        const m = g.methods.items[ev.method];
        const receiver = if (m.captures.items.len > 0) try captures.value(g, m) else "self";
        return zig_text.fmt(
            g,
            "ui.{s}({s}, {s}.{s})",
            .{
                events.actionBuilder(kind),
                receiver,
                try zig_text.id(g, g.name),
                try zig_text.id(g, g.methods.items[ev.method].name),
            },
        );
    };
    return core.fail(g, e.loc, "event handler must be a component function, setter, or inline arrow", .{});
}

pub fn isAction(k: tags.Kind) bool {
    return k == .action or k == .text_action or k == .bool_action or k == .index_action;
}

pub fn actionType(k: tags.Kind) Type {
    return switch (k) {
        .action => .void_,
        .text_action => .string,
        .bool_action => .boolean,
        .index_action => .usize,
        else => unreachable,
    };
}

pub fn actionBuilder(k: tags.Kind) []const u8 {
    return switch (k) {
        .action => "action",
        .text_action => "textAction",
        .bool_action => "boolAction",
        .index_action => "indexAction",
        else => unreachable,
    };
}

pub fn actionSignature(k: tags.Kind) []const u8 {
    return switch (k) {
        .action => "() => void",
        .text_action => "(value: string) => void",
        .bool_action => "(checked: boolean) => void",
        .index_action => "(index: usize) => void",
        else => unreachable,
    };
}

pub fn emitSetter(g: *Generator, m: Method) Error![]const u8 {
    const s = g.states.items[m.setter.?];
    const value = if (m.action == .index_action)
        (if (s.ty.integer()) "@intCast(value)" else "@floatFromInt(value)")
    else
        "value";
    return zig_text.fmt(
        g,
        "fn {s}(self: *{s}, value: {s}) void {{ {s} }}\n",
        .{
            try zig_text.id(g, m.name),
            try zig_text.id(g, g.name),
            events.actionType(m.action.?).zig(g),
            try stmt.stateWrite(g, try scope.stateField(g, s.name), s.ty, value),
        },
    );
}

/// Visit the condition and both branches when discovering nested event handlers.
fn scanConditional(g: *Generator, v: ast.Cond) Error!void {
    try events.scanExpr(g, v.cond);
    try events.scanExpr(g, v.then);
    try events.scanExpr(g, v.else_);
}

/// The type of a method's parameter `index`: from a callback prop's
/// signature when it has one, else as `parameterType`.
pub fn methodParamType(g: *Generator, m: Method, index: usize, param: ast.Param) Error!Type {
    const params = m.callback_params orelse return events.parameterType(g, param, m.action);
    if (index >= params.len) return core.fail(g, param.name.loc, "the callback passes {d} arguments", .{params.len});
    if (param.ty) |ref| {
        const t = try types.typeRef(g, ref, false);
        if (t != params[index]) return core.fail(
            g,
            param.name.loc,
            "parameter type does not match the callback prop",
            .{},
        );
        return t;
    }
    return params[index];
}

/// Prefer an annotation, then infer a parameter from its registered action kind.
pub fn parameterType(g: *Generator, param: ast.Param, kind: ?tags.Kind) Error!Type {
    if (param.optional or param.destructure.len > 0) {
        return core.fail(g, param.name.loc, "optional or destructured parameters are not supported yet", .{});
    }
    if (param.ty) |ref| return types.typeRef(g, ref, false);
    const action = kind orelse return core.fail(
        g,
        param.name.loc,
        "parameter `{s}` needs a type annotation",
        .{param.name.name},
    );
    return events.actionType(action);
}

/// Item actions use their own schema; JSX nested in any supported prop is scanned too.
fn scanProperty(g: *Generator, prop: *const tags.Prop, value: *ast.Expr, loc: ast.Loc) Error!void {
    if (events.isAction(prop.kind)) try events.event(g, value, prop, loc);
    if (prop.kind != .struct_list or value.data != .array) return events.scanExpr(g, value);
    const schema = tags.Tag{ .name = prop.name, .builder = "", .content = .none, .props = prop.item_props };
    for (value.data.array, 0..) |item, index| {
        if (item.data != .object) continue;
        for (item.data.object) |property| {
            if (property.spread) continue;
            if (tags.findProp(&schema, property.key.name)) |field| {
                scanProperty(g, field, property.value, property.key.loc) catch |err| {
                    if (err == error.Tsx) {
                        const prefix = try zig_text.fmt(g, "in {s}[{d}].{s}", .{
                            prop.name,
                            index,
                            property.key.name,
                        });
                        try core.contextualize(g, prefix);
                    }
                    return err;
                };
            } else try events.scanExpr(g, property.value);
        }
    }
}

/// Registers an inline arrow as the handler method of `prop`.
fn registerArrow(g: *Generator, prop: *const tags.Prop, e: *ast.Expr) Error!usize {
    g.counter += 1;
    const n = try scope.uniqueMethod(g, try zig_text.fmt(g, "_{s}{d}", .{ prop.name, g.counter }));
    const m = Method{
        .name = n,
        .func = e.data.arrow,
        .action = prop.kind,
        .capture_name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_Capture{d}", .{g.counter})),
        .ret = if (e.data.arrow.ret) |r| try types.typeRef(g, r, true) else .void_,
    };
    try events.signature(g, m, prop.kind, e.loc, true);
    try g.methods.append(g.a, m);
    return g.methods.items.len - 1;
}

/// `fn` passed as a handler, as the arrow `() => fn()` / `(v) => fn(v)`.
fn functionArrow(g: *Generator, e: *ast.Expr, kind: tags.Kind) Error!*ast.Expr {
    const callee = try g.a.create(ast.Expr);
    callee.* = e.*;
    const takes_value = kind != .action;
    var args: []const *ast.Expr = &.{};
    var params: []const ast.Param = &.{};
    if (takes_value) {
        const value = try g.a.create(ast.Expr);
        value.* = .{ .loc = e.loc, .data = .{ .ident = "_value" } };
        args = try g.a.dupe(*ast.Expr, &.{value});
        params = try g.a.dupe(ast.Param, &.{.{ .name = .{ .name = "_value", .loc = e.loc }, .ty = null }});
    }
    const call = try g.a.create(ast.Expr);
    call.* = .{ .loc = e.loc, .data = .{ .call = .{ .callee = callee, .type_args = &.{}, .args = args } } };
    const f = try g.a.create(ast.Function);
    f.* = .{ .loc = e.loc, .name = "", .params = params, .ret = null, .body = .{ .expr = call } };
    const arrow = try g.a.create(ast.Expr);
    arrow.* = .{ .loc = e.loc, .data = .{ .arrow = f } };
    return arrow;
}
