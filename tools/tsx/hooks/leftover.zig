//! After inlining: a custom hook still named somewhere was called where
//! hooks can't be (inside a condition, a loop, a handler, a plain
//! function); and a hook's body may only return at its end.
const std = @import("std");
const ast = @import("../ast.zig");
const resolve = @import("resolve.zig");

pub const Found = struct {
    name: []const u8,
    loc: ast.Loc,
};

const Walker = struct {
    units: []const resolve.Unit,
    unit: usize,
    found: ?Found = null,

    fn check(w: *Walker, name: []const u8, loc: ast.Loc) void {
        if (w.found != null) return;
        if (resolve.findHookIn(w.units, w.unit, name) == null) return;
        const at = std.mem.indexOfScalar(u8, name, '@') orelse name.len;
        w.found = .{ .name = name[0..at], .loc = loc };
    }

    fn function(w: *Walker, f: *const ast.Function) void {
        for (f.params) |p| if (p.default) |d| w.expr(d);
        switch (f.body) {
            .block => |ss| w.stmts(ss),
            .expr => |e| w.expr(e),
        }
    }

    fn stmts(w: *Walker, ss: []const *ast.Stmt) void {
        for (ss) |s| w.stmt(s);
    }

    fn opt(w: *Walker, e: ?*ast.Expr) void {
        if (e) |v| w.expr(v);
    }

    fn stmt(w: *Walker, s: *const ast.Stmt) void {
        switch (s.data) {
            .decl => |d| w.opt(d.init),
            .func => |f| w.function(f),
            .ret => |e| w.opt(e),
            .if_ => |v| {
                w.expr(v.cond);
                w.stmt(v.then);
                if (v.else_) |el| w.stmt(el);
            },
            .for_ => |v| {
                if (v.init) |i| w.stmt(i);
                w.opt(v.cond);
                w.opt(v.update);
                w.stmt(v.body);
            },
            .for_of => |v| {
                w.expr(v.iterable);
                w.stmt(v.body);
            },
            .while_ => |v| {
                w.expr(v.cond);
                w.stmt(v.body);
            },
            .switch_ => |v| {
                w.expr(v.value);
                for (v.cases) |case| {
                    w.opt(case.value);
                    w.stmts(case.body);
                }
            },
            .break_, .continue_ => {},
            .expr => |e| w.expr(e),
            .block => |ss| w.stmts(ss),
        }
    }

    fn expr(w: *Walker, e: *const ast.Expr) void {
        switch (e.data) {
            .number, .string, .boolean, .null_lit => {},
            .ident => |n| w.check(n, e.loc),
            .template => |t| for (t.exprs) |x| w.expr(x),
            .unary => |u| w.expr(u.operand),
            .binary => |b| {
                w.expr(b.lhs);
                w.expr(b.rhs);
            },
            .cond => |v| {
                w.expr(v.cond);
                w.expr(v.then);
                w.expr(v.else_);
            },
            .call => |v| {
                w.expr(v.callee);
                for (v.args) |a| w.expr(a);
            },
            .member => |m| w.expr(m.object),
            .arrow => |f| w.function(f),
            .jsx => |j| w.jsx(j),
            .object => |ps| for (ps) |p| w.expr(p.value),
            .array => |items| for (items) |x| w.expr(x),
            .spread, .non_null => |inner| w.expr(inner),
            .index => |ix| {
                w.expr(ix.object);
                w.expr(ix.index);
            },
            .as => |cast| w.expr(cast.expr),
            .assign => |v| {
                w.expr(v.target);
                w.expr(v.value);
            },
            .update => |v| w.expr(v.target),
        }
    }

    fn jsx(w: *Walker, j: *const ast.Jsx) void {
        for (j.attrs) |a| w.opt(a.value);
        for (j.children) |child| switch (child) {
            .text => {},
            .expr => |e| w.opt(e),
            .element => |el| w.jsx(el),
        };
    }
};

/// The first custom hook named in unit `unit`'s code (hooks themselves excluded).
pub fn find(units: []const resolve.Unit, unit: usize) ?Found {
    var w = Walker{ .units = units, .unit = unit };
    const m = units[unit].module;
    for (m.functions) |top| if (!resolve.isHookName(top.func.name)) w.function(top.func);
    for (m.consts) |c| if (!(resolve.isHookName(c.name.name) and c.value.data == .arrow)) w.expr(c.value);
    return w.found;
}

/// A `return` inside `s` outside nested functions.
pub fn returns(s: *const ast.Stmt) ?ast.Loc {
    return switch (s.data) {
        .ret => s.loc,
        .if_ => |v| returns(v.then) orelse if (v.else_) |el| returns(el) else null,
        .for_ => |v| returns(v.body),
        .for_of => |v| returns(v.body),
        .while_ => |v| returns(v.body),
        .switch_ => |v| for (v.cases) |case| {
            if (first(case.body)) |loc| break loc;
        } else null,
        .block => |ss| first(ss),
        else => null,
    };
}

fn first(ss: []const *ast.Stmt) ?ast.Loc {
    for (ss) |b| if (returns(b)) |loc| return loc;
    return null;
}
