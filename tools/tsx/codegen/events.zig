//! Event signatures, inferred parameters, generated handlers, and reusable setter adapters.
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
    if (tags.find(j.name)) |tag| for (j.attrs) |attr| {
        if (attr.value) |v| {
            if (zig_text.eq(attr.name.name, "style") and v.data == .object) {
                for (v.data.object) |p| {
                    if (tags.findProp(tag, p.key.name)) |prop| if (events.isAction(prop.kind)) {
                        try events.event(g, p.value, prop, p.key.loc);
                    };
                }
            } else if (tags.findProp(tag, attr.name.name)) |prop| {
                if (events.isAction(prop.kind)) try events.event(g, v, prop, attr.name.loc);
            }
            try events.scanExpr(g, v);
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
        } else {
            i = scope.sourceMethod(g, n) orelse return core.fail(
                g,
                e.loc,
                "event handler must be a component function, setter, or inline arrow",
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
        g.counter += 1;
        const n = try scope.uniqueMethod(g, try zig_text.fmt(g, "_{s}{d}", .{ prop.name, g.counter }));
        const m = Method{
            .name = n,
            .func = e.data.arrow,
            .action = prop.kind,
            .ret = if (e.data.arrow.ret) |r| try types.typeRef(g, r, true) else .void_,
        };
        try events.signature(g, m, prop.kind, e.loc, true);
        i = g.methods.items.len;
        try g.methods.append(g.a, m);
    } else return core.fail(g, e.loc, "event handler must be a component function, setter, or inline arrow", .{});
    try g.events.append(g.a, .{ .expr = e, .kind = prop.kind, .method = i });
}

pub fn eventValue(g: *Generator, e: *ast.Expr, kind: tags.Kind) Error![]const u8 {
    for (g.events.items) |ev| if (ev.expr == e and ev.kind == kind) {
        return zig_text.fmt(
            g,
            "ui.{s}(self, {s}.{s})",
            .{
                events.actionBuilder(kind),
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
            events.actionType(m.action.?).zig(),
            try stmt.stateWrite(g, try zig_text.id(g, s.name), s.ty, value),
        },
    );
}

/// Visit the condition and both branches when discovering nested event handlers.
fn scanConditional(g: *Generator, v: ast.Cond) Error!void {
    try events.scanExpr(g, v.cond);
    try events.scanExpr(g, v.then);
    try events.scanExpr(g, v.else_);
}

/// Prefer an annotation, then infer a parameter from its registered action kind.
pub fn parameterType(g: *Generator, param: ast.Param, kind: ?tags.Kind) Error!Type {
    if (param.ty) |ref| return types.typeRef(g, ref, false);
    const action = kind orelse return core.fail(
        g,
        param.name.loc,
        "parameter `{s}` needs a type annotation",
        .{param.name.name},
    );
    return events.actionType(action);
}
