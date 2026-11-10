//! Which child components keep local state (phase 3, docs/tsx.md): a
//! component that calls a hook (`useState`, `useRef`, …), renders a `<List>` (its scroll state), or
//! renders another stateful component needs an instance id, so its builder
//! takes one (`todoRow(props, id)`) and its call sites pass
//! `tsx.childId(parent, "TodoRow", key)`. Components without state keep the
//! plain `todoRow(props)` and cost nothing extra.
const std = @import("std");
const ast = @import("../ast.zig");
const imports = @import("imports.zig");

/// Nesting limit for components rendering components across files (cycles).
const max_depth = 16;

/// Whether `func` of `module` (a file of `project`) keeps local state.
pub fn stateful(project: []const imports.NamedModule, module: ast.Module, func: *const ast.Function) bool {
    return check(project, module, func, 0);
}

fn check(project: []const imports.NamedModule, module: ast.Module, func: *const ast.Function, depth: u32) bool {
    if (depth > max_depth) return false;
    var walker = Walker{ .project = project, .module = module, .depth = depth };
    walker.body(func.body);
    return walker.found;
}

const Walker = struct {
    project: []const imports.NamedModule,
    module: ast.Module,
    depth: u32,
    found: bool = false,

    fn body(w: *Walker, b: ast.Body) void {
        switch (b) {
            .block => |ss| for (ss) |s| w.stmt(s),
            .expr => |e| w.expr(e),
        }
    }

    fn stmt(w: *Walker, s: *const ast.Stmt) void {
        if (w.found) return;
        switch (s.data) {
            .decl => |d| if (d.init) |e| w.expr(e),
            .func => |f| w.body(f.body),
            .ret => |e| if (e) |v| w.expr(v),
            .if_ => |v| {
                w.expr(v.cond);
                w.stmt(v.then);
                if (v.else_) |el| w.stmt(el);
            },
            .expr => |e| w.expr(e),
            .block => |ss| for (ss) |child| w.stmt(child),
        }
    }

    fn expr(w: *Walker, e: *const ast.Expr) void {
        if (w.found) return;
        switch (e.data) {
            .number, .string, .boolean, .null_lit, .ident => {},
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
            .call => |c| {
                if (c.callee.data == .ident and w.isHook(c.callee.data.ident)) w.found = true;
                w.expr(c.callee);
                for (c.args) |a| w.expr(a);
            },
            .member => |m| w.expr(m.object),
            .arrow => |f| w.body(f.body),
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

    fn jsx(w: *Walker, j: *const ast.Jsx) void {
        if (w.found) return;
        if (std.mem.eql(u8, j.name, "List") or w.statefulTag(j.name)) {
            w.found = true;
            return;
        }
        for (j.attrs) |a| if (a.value) |v| w.expr(v);
        for (j.children) |child| switch (child) {
            .text => {},
            .expr => |x| if (x) |e| w.expr(e),
            .element => |el| w.jsx(el),
        };
    }

    /// `useRef`, `useMemo`, `useEffect`, `useState` or the local name it is
    /// imported under.
    fn isHook(w: *const Walker, name: []const u8) bool {
        if (@import("hooks.zig").extraHook(name)) return true;
        for (w.module.imports) |imp| for (imp.names) |n| {
            if (std.mem.eql(u8, n.imported.name, "useState") and std.mem.eql(u8, n.local.name, name)) return true;
        };
        return std.mem.eql(u8, name, "useState");
    }

    /// A tag naming a stateful component of this file or an imported one.
    fn statefulTag(w: *const Walker, name: []const u8) bool {
        for (w.module.functions) |top| if (std.mem.eql(u8, top.func.name, name)) {
            return check(w.project, w.module, top.func, w.depth + 1);
        };
        for (w.module.imports) |imp| {
            if (!imports.isRelative(imp.source)) continue;
            for (imp.names) |n| {
                if (!std.mem.eql(u8, n.local.name, name)) continue;
                const target = imports.findModule(w.project, imp.source) orelse return false;
                for (target.functions) |top| if (top.exported and std.mem.eql(u8, top.func.name, n.imported.name)) {
                    return check(w.project, target, top.func, w.depth + 1);
                };
                return false;
            }
        }
        return false;
    }
};

test "useState, List and stateful children make a component stateful" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const src =
        \\import { useState as useS } from "zant";
        \\function Plain() { return <Text>hi</Text>; }
        \\function Counter() { const [n, setN] = useS(0); return <Button onClick={() => setN(n + 1)}>{n}</Button>; }
        \\function Wrapper() { return <View>{true && <Counter />}</View>; }
        \\function Feed() { return <List data={[1]} renderItem={(x) => <Text>{x}</Text>} />; }
        \\export default function App() { return <View><Plain /></View>; }
    ;
    var p = @import("../parser.zig").Parser.init(arena.allocator(), src);
    const module = try p.parseModule();
    const expected = [_]bool{ false, true, true, true, false };
    for (module.functions, expected) |top, want| {
        try std.testing.expectEqual(want, stateful(&.{}, module, top.func));
    }
}
