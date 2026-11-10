//! Does a function body mention one of these names? Used to find component
//! functions (`const copy = () => …`) that read render locals: those are
//! emitted like inline arrows where they are passed (`onClick={copy}`), so
//! they capture the values of that render.
//!
//! Over-approximates (a parameter or inner local with the same name counts):
//! inlining a function that did not need it is still correct.
const std = @import("std");
const ast = @import("../ast.zig");

pub fn function(f: *const ast.Function, names: []const []const u8) bool {
    return switch (f.body) {
        .block => |ss| statements(ss, names),
        .expr => |e| expr(e, names),
    };
}

fn statements(ss: []const *ast.Stmt, names: []const []const u8) bool {
    for (ss) |s| if (statement(s, names)) return true;
    return false;
}

fn opt(e: ?*ast.Expr, names: []const []const u8) bool {
    return if (e) |v| expr(v, names) else false;
}

fn statement(s: *const ast.Stmt, names: []const []const u8) bool {
    return switch (s.data) {
        .decl => |d| opt(d.init, names),
        .func => |f| function(f, names),
        .ret => |e| opt(e, names),
        .if_ => |v| expr(v.cond, names) or statement(v.then, names) or
            (if (v.else_) |el| statement(el, names) else false),
        .for_ => |v| (if (v.init) |i| statement(i, names) else false) or opt(v.cond, names) or
            opt(v.update, names) or statement(v.body, names),
        .for_of => |v| expr(v.iterable, names) or statement(v.body, names),
        .while_ => |v| expr(v.cond, names) or statement(v.body, names),
        .switch_ => |v| blk: {
            if (expr(v.value, names)) break :blk true;
            for (v.cases) |case| if (opt(case.value, names) or statements(case.body, names)) break :blk true;
            break :blk false;
        },
        .break_, .continue_ => false,
        .expr => |e| expr(e, names),
        .block => |ss| statements(ss, names),
    };
}

fn exprs(es: []const *ast.Expr, names: []const []const u8) bool {
    for (es) |e| if (expr(e, names)) return true;
    return false;
}

fn expr(e: *const ast.Expr, names: []const []const u8) bool {
    return switch (e.data) {
        .number, .string, .boolean, .null_lit => false,
        .ident => |n| for (names) |name| {
            if (std.mem.eql(u8, name, n)) break true;
        } else false,
        .template => |t| exprs(t.exprs, names),
        .unary => |u| expr(u.operand, names),
        .binary => |b| expr(b.lhs, names) or expr(b.rhs, names),
        .cond => |v| expr(v.cond, names) or expr(v.then, names) or expr(v.else_, names),
        .call => |v| expr(v.callee, names) or exprs(v.args, names),
        .member => |m| expr(m.object, names),
        .arrow => |f| function(f, names),
        .jsx => |j| jsx(j, names),
        .object => |ps| for (ps) |p| {
            if (expr(p.value, names)) break true;
        } else false,
        .array => |items| exprs(items, names),
        .spread, .non_null => |inner| expr(inner, names),
        .index => |ix| expr(ix.object, names) or expr(ix.index, names),
        .as => |cast| expr(cast.expr, names),
        .assign => |v| expr(v.target, names) or expr(v.value, names),
        .update => |v| expr(v.target, names),
    };
}

fn jsx(j: *const ast.Jsx, names: []const []const u8) bool {
    for (j.attrs) |a| if (opt(a.value, names)) return true;
    for (j.children) |child| switch (child) {
        .text => {},
        .expr => |e| if (opt(e, names)) return true,
        .element => |el| if (jsx(el, names)) return true,
    };
    return false;
}
