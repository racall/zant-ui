//! Lower qualified React hooks and Fragment to the existing Zant syntax.
//! Runs after parsing all imports, before stateful analysis or generation.
const std = @import("std");
const ast = @import("ast.zig");

/// Whether a name is a default or namespace binding imported from React.
pub fn namespace(imports: []const ast.Import, name: []const u8) bool {
    for (imports) |imp| {
        if (!std.mem.eql(u8, imp.source, "react")) continue;
        for ([_]?ast.Name{ imp.default_name, imp.namespace_name }) |binding| {
            if (binding) |n| if (std.mem.eql(u8, n.name, name)) return true;
        }
    }
    return false;
}

/// A qualified element type through a React binding (including renamed namespaces).
pub fn nodeType(imports: []const ast.Import, name: []const u8) bool {
    const dot = std.mem.indexOfScalar(u8, name, '.') orelse return false;
    return namespace(imports, name[0..dot]) and
        (std.mem.eql(u8, name[dot + 1 ..], "ReactNode") or std.mem.eql(u8, name[dot + 1 ..], "ReactElement"));
}

/// Normalize only the four supported React hooks and React's Fragment tag.
pub fn normalize(module: ast.Module) void {
    for (module.imports) |imp| {
        if (!std.mem.eql(u8, imp.source, "react") or imp.type_only) continue;
        for ([_]?ast.Name{ imp.default_name, imp.namespace_name }) |binding| {
            const name = binding orelse continue;
            var walker = Walker{ .name = name.name };
            for (module.functions) |top| walker.function(top.func);
            for (module.consts) |decl| walker.expr(decl.value);
        }
    }
}

const Walker = struct {
    name: []const u8,

    fn matches(w: *const Walker, name: []const u8) bool {
        return std.mem.eql(u8, w.name, name);
    }

    fn binds(w: *const Walker, pattern: ast.Pattern) bool {
        return switch (pattern) {
            .ident => |n| w.matches(n.name),
            .array => |ns| blk: {
                for (ns) |n| if (n) |v| if (w.matches(v.name)) break :blk true;
                break :blk false;
            },
            .object => |bs| blk: {
                for (bs) |b| if (w.matches(b.local.name)) break :blk true;
                break :blk false;
            },
        };
    }

    fn function(w: *Walker, f: *ast.Function) void {
        if (w.matches(f.name)) return;
        for (f.params) |param| {
            if (w.matches(param.name.name)) return;
            for (param.destructure) |n| if (w.matches(n.name)) return;
        }
        w.body(f.body);
    }

    fn block(w: *Walker, ss: []const *ast.Stmt) void {
        // Lexical declarations shadow the import throughout their block (including the TDZ).
        for (ss) |s| switch (s.data) {
            .decl => |d| if (w.binds(d.pattern)) return,
            .func => |f| if (w.matches(f.name)) return,
            else => {},
        };
        for (ss) |s| w.stmt(s);
    }

    fn body(w: *Walker, b: ast.Body) void {
        switch (b) {
            .block => |ss| w.block(ss),
            .expr => |e| w.expr(e),
        }
    }

    fn stmt(w: *Walker, s: *ast.Stmt) void {
        switch (s.data) {
            .decl => |d| if (d.init) |e| w.expr(e),
            .func => |f| w.function(f),
            .ret => |e| if (e) |v| w.expr(v),
            .expr => |e| w.expr(e),
            .block => |ss| w.block(ss),
            .if_ => |v| {
                w.expr(v.cond);
                w.stmt(v.then);
                if (v.else_) |el| w.stmt(el);
            },
            .for_ => |v| {
                if (v.init) |init| if (init.data == .decl and w.binds(init.data.decl.pattern)) return;
                if (v.init) |init| w.stmt(init);
                if (v.cond) |cond| w.expr(cond);
                if (v.update) |update| w.expr(update);
                w.stmt(v.body);
            },
            .for_of => |v| {
                w.expr(v.iterable);
                if (!w.matches(v.name.name)) w.stmt(v.body);
            },
            .while_ => |v| {
                w.expr(v.cond);
                w.stmt(v.body);
            },
            .switch_ => |v| {
                w.expr(v.value);
                for (v.cases) |case| {
                    if (case.value) |value| w.expr(value);
                    for (case.body) |child| w.stmt(child);
                }
            },
            .break_, .continue_ => {},
        }
    }

    fn expr(w: *Walker, e: *ast.Expr) void {
        switch (e.data) {
            .number, .string, .boolean, .null_lit, .ident => {},
            .call => |c| {
                if (c.callee.data == .member) {
                    const m = c.callee.data.member;
                    if (!m.optional and m.object.data == .ident and w.matches(m.object.data.ident)) {
                        inline for (.{ "useState", "useEffect", "useMemo", "useRef" }) |hook| {
                            if (std.mem.eql(u8, m.name.name, hook)) c.callee.data = .{ .ident = hook };
                        }
                    }
                }
                w.expr(c.callee);
                for (c.args) |a| w.expr(a);
            },
            .template => |t| for (t.exprs) |x| w.expr(x),
            .unary => |u| w.expr(u.operand),
            .binary => |b| {
                w.expr(b.lhs);
                w.expr(b.rhs);
            },
            .cond => |c| {
                w.expr(c.cond);
                w.expr(c.then);
                w.expr(c.else_);
            },
            .member => |m| w.expr(m.object),
            .arrow => |f| w.function(f),
            .jsx => |j| w.jsx(j),
            .object => |props| for (props) |p| w.expr(p.value),
            .array => |items| for (items) |x| w.expr(x),
            .spread, .non_null => |x| w.expr(x),
            .index => |i| {
                w.expr(i.object);
                w.expr(i.index);
            },
            .as => |a| w.expr(a.expr),
            .assign => |a| {
                w.expr(a.target);
                w.expr(a.value);
            },
            .update => |u| w.expr(u.target),
        }
    }

    fn jsx(w: *Walker, j: *ast.Jsx) void {
        if (std.mem.indexOfScalar(u8, j.name, '.')) |dot| {
            if (w.matches(j.name[0..dot]) and std.mem.eql(u8, j.name[dot + 1 ..], "Fragment")) {
                j.name = "";
            }
        }
        for (j.attrs) |a| if (a.value) |v| w.expr(v);
        for (j.children) |child| switch (child) {
            .text => {},
            .expr => |x| if (x) |e| w.expr(e),
            .element => |el| w.jsx(el),
        };
    }
};
