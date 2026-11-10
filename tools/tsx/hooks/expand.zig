//! Custom hooks (React's `function useTimers() { … }`): a call at the top
//! level of a component is replaced by the hook's body, before code is
//! generated, so the hooks inside it (state, effects, app and desktop hooks)
//! belong to that component as if written there:
//!
//!     function useToast(ms: number) {            // src/hooks/useToast.tsx
//!       const [text, setText] = useState("");
//!       useEffect(() => { … }, [text]);
//!       return { text, show: setText };
//!     }
//!     const { text, show } = useToast(2000);     // in a component
//!
//! The body's names get the hook's prefix (`useToast_text`), arguments
//! replace the parameters (or become constants), and the names the hook
//! returned stand for its values in the rest of the component. Names the
//! hook's file imports are imported where it is inlined (`resolve.zig`).
//! Hooks may call hooks; a call anywhere but the top level is an error.
const std = @import("std");
const ast = @import("../ast.zig");
const rewrite = @import("rewrite.zig");
const resolve = @import("resolve.zig");
const leftover = @import("leftover.zig");
pub const Unit = resolve.Unit;
const Error = rewrite.Error;

/// A failure in file `unit` (inlined code is reported in the hook's file).
pub const Failure = struct {
    unit: usize,
    loc: ast.Loc,
    msg: []const u8,
};

/// Locations of code inlined from another file carry that file (`unit + 1`)
/// in their top bits; `decode` splits them again.
pub const loc_bits = 24;

pub fn tag(unit: usize) ast.Loc {
    if (unit >= 255) return 0;
    return @as(ast.Loc, @intCast(unit + 1)) << loc_bits;
}

/// The file and offset of `loc` in a file that is `unit`.
pub fn decode(unit: usize, loc: ast.Loc) struct { unit: usize, loc: ast.Loc } {
    const t = loc >> loc_bits;
    if (t == 0) return .{ .unit = unit, .loc = loc };
    return .{ .unit = t - 1, .loc = loc & ((@as(ast.Loc, 1) << loc_bits) - 1) };
}

/// Inlines every custom hook call of every unit's components, then removes
/// the hooks (they are not code of their own).
pub fn expand(arena: std.mem.Allocator, units: []Unit, failure: *?Failure) error{OutOfMemory}!bool {
    var why: rewrite.Failure = .{ .loc = 0, .msg = "" };
    for (units, 0..) |*u, i| {
        for (u.module.functions) |top| {
            if (resolve.isHookName(top.func.name) or !isComponent(top.func)) continue;
            var e = Expander{ .arena = arena, .units = units, .unit = i, .component = top.func, .failure = &why };
            e.run() catch |err| switch (err) {
                error.OutOfMemory => return error.OutOfMemory,
                error.Hook => return report(failure, i, why),
            };
        }
    }
    for (units, 0..) |*u, i| {
        if (leftover.find(units, i)) |found| {
            why = .{ .loc = found.loc, .msg = try std.fmt.allocPrint(
                arena,
                "`{s}` is a custom hook: call it at the top level of a component or of another hook",
                .{found.name},
            ) };
            return report(failure, i, why);
        }
        _ = u;
    }
    // which imports name hooks, before the hooks go
    const imports = try arena.alloc([]const ast.Import, units.len);
    for (units, imports) |*u, *out| out.* = try importsWithoutHooks(arena, units, u);
    for (units, imports) |*u, list| {
        try finish(arena, u);
        u.module.imports = list;
    }
    return true;
}

fn report(failure: *?Failure, unit: usize, why: rewrite.Failure) bool {
    const at = decode(unit, why.loc);
    failure.* = .{ .unit = at.unit, .loc = at.loc, .msg = why.msg };
    return false;
}

/// React's rule: components are capitalized.
fn isComponent(f: *const ast.Function) bool {
    return f.name.len > 0 and std.ascii.isUpper(f.name[0]) and f.body == .block;
}

/// The unit's imports and the ones inlining added, without the hooks.
fn importsWithoutHooks(arena: std.mem.Allocator, units: []Unit, u: *Unit) error{OutOfMemory}![]const ast.Import {
    var imports: std.ArrayList(ast.Import) = .empty;
    for ([_][]const ast.Import{ u.module.imports, u.added.items }) |list| for (list) |imp| {
        var names: std.ArrayList(ast.ImportName) = .empty;
        const target = resolve.unitOf(units, imp.source);
        for (imp.names) |n| {
            if (target) |t| if (resolve.hookIn(units[t].module, n.imported.name) != null) continue;
            try names.append(arena, n);
        }
        const bare = imp.default_name == null and imp.namespace_name == null;
        if (names.items.len == 0 and imp.names.len > 0 and bare) continue;
        var copy = imp;
        copy.names = names.items;
        try imports.append(arena, copy);
    };
    return imports.items;
}

/// The hooks go; the declarations other files now import are exported.
fn finish(arena: std.mem.Allocator, u: *Unit) error{OutOfMemory}!void {
    const m = u.module;
    var functions: std.ArrayList(ast.TopFunction) = .empty;
    for (m.functions) |top| {
        if (resolve.isHookName(top.func.name)) continue;
        var copy = top;
        if (contains(u.exported.items, top.func.name)) copy.exported = true;
        try functions.append(arena, copy);
    }
    var consts: std.ArrayList(ast.TopConst) = .empty;
    for (m.consts) |c| {
        if (resolve.isHookName(c.name.name) and c.value.data == .arrow) continue;
        var copy = c;
        if (contains(u.exported.items, c.name.name)) copy.exported = true;
        try consts.append(arena, copy);
    }
    const types = try arena.dupe(ast.TypeDecl, m.types);
    for (types) |*t| if (contains(u.exported.items, t.name.name)) {
        t.exported = true;
    };
    m.functions = functions.items;
    m.consts = consts.items;
    m.types = types;
}

fn contains(list: []const []const u8, name: []const u8) bool {
    for (list) |n| if (std.mem.eql(u8, n, name)) return true;
    return false;
}

/// The hook call a top-level statement makes, if any.
const Site = struct {
    call: ast.Call,
    loc: ast.Loc,
    hook: resolve.Hook,
    /// Where its result goes.
    target: union(enum) { none, ident: ast.Name, array: []const ?ast.Name, object: []const ast.ObjectBinding },
    ty: ?ast.TypeRef = null,
};

const Expander = struct {
    arena: std.mem.Allocator,
    units: []Unit,
    unit: usize,
    component: *ast.Function,
    failure: *rewrite.Failure,
    locals: std.StringHashMapUnmanaged(void) = .empty,
    /// Names the component's later statements read as a hook's values.
    aliases: rewrite.Rewriter = undefined,
    counts: std.StringHashMapUnmanaged(usize) = .empty,

    fn fail(self: *Expander, loc: ast.Loc, comptime f: []const u8, args: anytype) Error {
        self.failure.* = .{ .loc = loc, .msg = try std.fmt.allocPrint(self.arena, f, args) };
        return error.Hook;
    }

    fn run(self: *Expander) Error!void {
        self.aliases = .{ .arena = self.arena, .failure = self.failure };
        for (self.component.params) |p| {
            try self.locals.put(self.arena, p.name.name, {});
            for (p.destructure) |n| try self.locals.put(self.arena, n.name, {});
        }
        var list: std.ArrayList(*ast.Stmt) = .empty;
        try list.appendSlice(self.arena, self.component.body.block);
        for (list.items) |s| try self.declare(s);
        var depth: std.ArrayList(u8) = .empty;
        try depth.appendNTimes(self.arena, 0, list.items.len);
        var changed = false;
        var i: usize = 0;
        while (i < list.items.len) {
            const s = if (self.aliases.env.items.len > 0) try self.aliases.stmt(list.items[i]) else list.items[i];
            list.items[i] = s;
            const call = (try self.site(s)) orelse {
                i += 1;
                continue;
            };
            if (depth.items[i] >= 32) return self.fail(s.loc, "custom hooks call each other without end", .{});
            const body = try self.inline_(call);
            for (body) |b| try self.declare(b);
            try list.replaceRange(self.arena, i, 1, body);
            try depth.replaceRange(self.arena, i, 1, try filled(self.arena, body.len, depth.items[i] + 1));
            changed = true;
        }
        if (changed) self.component.body = .{ .block = list.items };
    }

    fn filled(arena: std.mem.Allocator, n: usize, value: u8) Error![]const u8 {
        const out = try arena.alloc(u8, n);
        @memset(out, value);
        return out;
    }

    fn declare(self: *Expander, s: *const ast.Stmt) Error!void {
        switch (s.data) {
            .decl => |d| switch (d.pattern) {
                .ident => |n| try self.locals.put(self.arena, n.name, {}),
                .array => |names| for (names) |n| if (n) |v| try self.locals.put(self.arena, v.name, {}),
                .object => |bindings| for (bindings) |b| try self.locals.put(self.arena, b.local.name, {}),
            },
            .func => |f| try self.locals.put(self.arena, f.name, {}),
            else => {},
        }
    }

    fn site(self: *Expander, s: *const ast.Stmt) Error!?Site {
        const e, const target: @FieldType(Site, "target"), const ty: ?ast.TypeRef = switch (s.data) {
            .expr => |e| .{ e, .none, null },
            .decl => |d| .{ d.init orelse return null, switch (d.pattern) {
                .ident => |n| .{ .ident = n },
                .array => |names| .{ .array = names },
                .object => |bindings| .{ .object = bindings },
            }, d.ty },
            else => return null,
        };
        if (e.data != .call or e.data.call.callee.data != .ident) return null;
        const name = e.data.call.callee.data.ident;
        const hook = resolve.findHookIn(self.units, self.unit, name) orelse return null;
        if (std.mem.indexOfScalar(u8, name, '@') == null and hook.unit != self.unit and !hook.exported) {
            return self.fail(e.loc, "`{s}` is not exported by its file", .{name});
        }
        return .{ .call = e.data.call, .loc = e.loc, .hook = hook, .target = target, .ty = ty };
    }

    /// The statements that replace the call at `site`.
    fn inline_(self: *Expander, at: Site) Error![]const *ast.Stmt {
        const h = at.hook.func;
        const count = (self.counts.get(h.name) orelse 0) + 1;
        try self.counts.put(self.arena, h.name, count);
        const prefix = if (count == 1)
            try std.fmt.allocPrint(self.arena, "{s}_", .{h.name})
        else
            try std.fmt.allocPrint(self.arena, "{s}{d}_", .{ h.name, count });
        var names = resolve.Context{
            .arena = self.arena,
            .units = self.units,
            .caller = self.unit,
            .from = at.hook.unit,
            .hook = h.name,
            .component = self.component.name,
            .locals = &self.locals,
            .failure = self.failure,
        };
        var rw = rewrite.Rewriter{
            .arena = self.arena,
            .failure = self.failure,
            .prefix = prefix,
            .loc_tag = if (at.hook.unit == self.unit) 0 else tag(at.hook.unit),
            .free = names.free(),
        };
        var out: std.ArrayList(*ast.Stmt) = .empty;
        try self.arguments(&rw, at, &out);
        const body: []const *ast.Stmt = switch (h.body) {
            .block => |ss| ss,
            .expr => |e| &.{try self.newStmt(e.loc, .{ .ret = e })},
        };
        try rw.predeclare(body);
        var returned: ?*ast.Expr = null;
        for (body, 0..) |s, i| {
            if (s.data == .ret) {
                if (i + 1 != body.len) return self.fail(
                    s.loc | rw.loc_tag,
                    "a custom hook returns once, at its end",
                    .{},
                );
                returned = if (s.data.ret) |e| try rw.expr(e) else null;
                continue;
            }
            if (leftover.returns(s)) |loc| {
                return self.fail(
                    loc | rw.loc_tag,
                    "a custom hook returns once, at its end (not inside `if` or loops)",
                    .{},
                );
            }
            try out.append(self.arena, try rw.stmt(s));
        }
        try self.result(at, returned, &out);
        return out.items;
    }

    fn newStmt(self: *Expander, loc: ast.Loc, data: ast.Stmt.Data) Error!*ast.Stmt {
        const s = try self.arena.create(ast.Stmt);
        s.* = .{ .loc = loc, .data = data };
        return s;
    }

    fn decl(self: *Expander, name: []const u8, loc: ast.Loc, ty: ?ast.TypeRef, value: *ast.Expr) Error!*ast.Stmt {
        return self.newStmt(loc, .{ .decl = .{
            .is_const = true,
            .pattern = .{ .ident = .{ .name = name, .loc = loc } },
            .ty = ty,
            .init = value,
        } });
    }

    /// Parameters become the arguments: names and literals are used as
    /// they are, functions and other values become constants.
    fn arguments(self: *Expander, rw: *rewrite.Rewriter, at: Site, out: *std.ArrayList(*ast.Stmt)) Error!void {
        const h = at.hook.func;
        if (at.call.args.len > h.params.len) {
            return self.fail(at.loc, "`{s}` takes {d} arguments", .{ h.name, h.params.len });
        }
        if (at.call.type_args.len > 0) return self.fail(at.loc, "`{s}` takes no type arguments", .{h.name});
        var bindings: std.ArrayList(rewrite.Entry) = .empty;
        for (h.params, 0..) |p, i| {
            if (p.destructure.len > 0) {
                return self.fail(p.name.loc | rw.loc_tag, "a custom hook's parameters are plain names", .{});
            }
            const given = i < at.call.args.len;
            if (given and at.call.args[i].data == .spread) {
                return self.fail(at.call.args[i].loc, "spread arguments are not supported", .{});
            }
            // defaults are the hook's code (its file's names)
            const value = if (given) at.call.args[i] else if (p.default) |d| try rw.expr(d) else if (p.optional)
                try self.undefinedExpr(at.loc)
            else
                return self.fail(at.loc, "`{s}` takes {d} arguments", .{ h.name, h.params.len });
            if (simple(value)) {
                try bindings.append(self.arena, .{ .name = p.name.name, .binding = .{ .expr = value } });
                continue;
            }
            const name = try std.fmt.allocPrint(self.arena, "{s}{s}", .{ rw.prefix.?, p.name.name });
            const ty = if (p.ty) |t| try rw.typeRef(t) else null;
            try out.append(self.arena, try self.decl(name, value.loc, if (value.data == .arrow) null else ty, value));
            try self.locals.put(self.arena, name, {});
            try bindings.append(self.arena, .{ .name = p.name.name, .binding = .{ .rename = name } });
        }
        try rw.env.appendSlice(self.arena, bindings.items);
    }

    fn undefinedExpr(self: *Expander, loc: ast.Loc) Error!*ast.Expr {
        const e = try self.arena.create(ast.Expr);
        e.* = .{ .loc = loc, .data = .{ .ident = "undefined" } };
        return e;
    }

    /// The caller's names for what the hook returned.
    fn result(self: *Expander, at: Site, value: ?*ast.Expr, out: *std.ArrayList(*ast.Stmt)) Error!void {
        const h = at.hook.func;
        switch (at.target) {
            .none => if (value) |v| if (v.data == .call) {
                try out.append(self.arena, try self.newStmt(at.loc, .{ .expr = v }));
            },
            .ident => |n| {
                const v = value orelse return self.fail(at.loc, "`{s}` returns nothing", .{h.name});
                if (v.data == .object) {
                    try self.alias(n.name, .{ .object = v.data.object });
                } else try self.give(n, v, at.ty, out);
            },
            .array => |names| {
                const v = value orelse return self.fail(at.loc, "`{s}` returns nothing", .{h.name});
                if (v.data != .array) return self.fail(
                    at.loc,
                    "`{s}` must return an array literal (`return [a, b]`) to destructure it",
                    .{h.name},
                );
                for (names, 0..) |n, i| if (n) |name| {
                    if (i >= v.data.array.len) {
                        return self.fail(name.loc, "`{s}` returns {d} values", .{ h.name, v.data.array.len });
                    }
                    try self.give(name, v.data.array[i], null, out);
                };
            },
            .object => |bindings| {
                const v = value orelse return self.fail(at.loc, "`{s}` returns nothing", .{h.name});
                if (v.data != .object) return self.fail(
                    at.loc,
                    "`{s}` must return an object literal (`return {{ a, b }}`) to destructure it",
                    .{h.name},
                );
                for (bindings) |b| {
                    const field = for (v.data.object) |p| {
                        if (!p.spread and std.mem.eql(u8, p.key.name, b.key.name)) break p.value;
                    } else return self.fail(b.key.loc, "`{s}` returns no `{s}`", .{ h.name, b.key.name });
                    try self.give(b.local, field, null, out);
                }
            },
        }
    }

    /// The rest of the component reads `name` as `binding`. Earlier aliases
    /// may name it (a hook returned a value of a hook it called): they are
    /// updated, so every alias is final.
    fn alias(self: *Expander, name: []const u8, binding: rewrite.Binding) Error!void {
        var one = rewrite.Rewriter{ .arena = self.arena, .failure = self.failure };
        try one.env.append(self.arena, .{ .name = name, .binding = binding });
        for (self.aliases.env.items) |*entry| switch (entry.binding) {
            .rename => {},
            .expr => |e| entry.binding = .{ .expr = try one.expr(e) },
            .object => |props| {
                const copy = try self.arena.alloc(ast.Property, props.len);
                for (props, copy) |p, *o| o.* = .{ .key = p.key, .value = try one.expr(p.value), .spread = p.spread };
                entry.binding = .{ .object = copy };
            },
        };
        try self.aliases.env.append(self.arena, .{ .name = name, .binding = binding });
    }

    /// `name` stands for `value` in the rest of the component: replaced
    /// when it is a name or a literal, else a constant.
    fn give(
        self: *Expander,
        name: ast.Name,
        value: *ast.Expr,
        ty: ?ast.TypeRef,
        out: *std.ArrayList(*ast.Stmt),
    ) Error!void {
        if (simple(value) and ty == null) return self.alias(name.name, .{ .expr = value });
        try out.append(self.arena, try self.decl(name.name, name.loc, ty, value));
    }
};

/// Names, literals and member paths: substituting them changes nothing.
fn simple(e: *const ast.Expr) bool {
    return switch (e.data) {
        .ident, .number, .string, .boolean, .null_lit => true,
        .member => |m| simple(m.object),
        .non_null => |inner| simple(inner),
        .template => |t| t.exprs.len == 0,
        else => false,
    };
}
