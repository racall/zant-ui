//! Copies statements and expressions while renaming the bindings they
//! declare and replacing the names they read: how a custom hook's body is
//! inlined into a component (`expand.zig`), and how names a hook returned
//! are replaced in the rest of the component.
const std = @import("std");
const ast = @import("../ast.zig");

pub const Error = error{ OutOfMemory, Hook };

/// Why inlining failed (`error.Hook`).
pub const Failure = struct {
    loc: ast.Loc,
    msg: []const u8,
};

pub const Binding = union(enum) {
    /// Another name (a renamed local, an import alias).
    rename: []const u8,
    /// An expression (a hook's argument, a value it returned).
    expr: *ast.Expr,
    /// An object a hook returned: `v.key` is the property's value.
    object: []const ast.Property,
};

pub const Entry = struct {
    name: []const u8,
    binding: Binding,
};

/// What a name the copied code reads without declaring it names.
pub const Kind = enum { value, type_name, tag };

pub const Free = struct {
    ctx: *anyopaque,
    /// The name to use for `name` in the copy.
    resolve: *const fn (ctx: *anyopaque, name: []const u8, loc: ast.Loc, kind: Kind) Error![]const u8,
};

pub const Rewriter = struct {
    arena: std.mem.Allocator,
    failure: *Failure,
    env: std.ArrayList(Entry) = .empty,
    /// Bindings declared in the copy get this prefix (null: they keep their names).
    prefix: ?[]const u8 = null,
    /// Or-ed into every location (the file of inlined code, `expand.zig`).
    loc_tag: ast.Loc = 0,
    free: ?Free = null,

    pub fn fail(self: *Rewriter, at: ast.Loc, comptime f: []const u8, args: anytype) Error {
        self.failure.* = .{ .loc = at, .msg = try std.fmt.allocPrint(self.arena, f, args) };
        return error.Hook;
    }

    fn lookup(self: *const Rewriter, name: []const u8) ?Binding {
        var i = self.env.items.len;
        while (i > 0) {
            i -= 1;
            if (std.mem.eql(u8, self.env.items[i].name, name)) return self.env.items[i].binding;
        }
        return null;
    }

    /// Declares `name` in the copy; returns its new name.
    pub fn bind(self: *Rewriter, name: []const u8) Error![]const u8 {
        const new = if (self.prefix) |p| try std.fmt.allocPrint(self.arena, "{s}{s}", .{ p, name }) else name;
        try self.env.append(self.arena, .{ .name = name, .binding = .{ .rename = new } });
        return new;
    }

    fn renamed(self: *Rewriter, name: ast.Name) Error!ast.Name {
        const new = if (self.lookup(name.name)) |b| switch (b) {
            .rename => |n| n,
            else => try self.bind(name.name),
        } else try self.bind(name.name);
        return .{ .name = new, .loc = self.loc(name.loc) };
    }

    fn loc(self: *const Rewriter, l: ast.Loc) ast.Loc {
        return l | self.loc_tag;
    }

    /// Declares the names a block declares (functions are hoisted, so all of them first).
    pub fn predeclare(self: *Rewriter, ss: []const *ast.Stmt) Error!void {
        for (ss) |s| switch (s.data) {
            .decl => |d| switch (d.pattern) {
                .ident => |n| _ = try self.bind(n.name),
                .array => |names| for (names) |n| if (n) |v| {
                    _ = try self.bind(v.name);
                },
                .object => |bindings| for (bindings) |b| {
                    _ = try self.bind(b.local.name);
                },
            },
            .func => |f| _ = try self.bind(f.name),
            else => {},
        };
    }

    pub fn statements(self: *Rewriter, ss: []const *ast.Stmt) Error![]const *ast.Stmt {
        const mark = self.env.items.len;
        defer self.env.shrinkRetainingCapacity(mark);
        try self.predeclare(ss);
        const out = try self.arena.alloc(*ast.Stmt, ss.len);
        for (ss, out) |s, *o| o.* = try self.stmt(s);
        return out;
    }

    fn opt(self: *Rewriter, e: ?*ast.Expr) Error!?*ast.Expr {
        return if (e) |v| try self.expr(v) else null;
    }

    fn newStmt(self: *Rewriter, at: ast.Loc, data: ast.Stmt.Data) Error!*ast.Stmt {
        const s = try self.arena.create(ast.Stmt);
        s.* = .{ .loc = self.loc(at), .data = data };
        return s;
    }

    pub fn stmt(self: *Rewriter, s: *const ast.Stmt) Error!*ast.Stmt {
        return switch (s.data) {
            .decl => |d| blk: {
                // the initializer reads the names before this declaration
                const init = try self.opt(d.init);
                const ty = if (d.ty) |t| try self.typeRef(t) else null;
                break :blk self.newStmt(s.loc, .{ .decl = .{
                    .is_const = d.is_const,
                    .pattern = try self.pattern(d.pattern),
                    .ty = ty,
                    .init = init,
                } });
            },
            .func => |f| blk: {
                const name = try self.renamed(.{ .name = f.name, .loc = f.loc });
                const copy = try self.function(f);
                copy.name = name.name;
                break :blk self.newStmt(s.loc, .{ .func = copy });
            },
            .ret => |e| self.newStmt(s.loc, .{ .ret = try self.opt(e) }),
            .if_ => |v| self.newStmt(s.loc, .{ .if_ = .{
                .cond = try self.expr(v.cond),
                .then = try self.scoped(v.then),
                .else_ = if (v.else_) |el| try self.scoped(el) else null,
            } }),
            .for_ => |v| blk: {
                const mark = self.env.items.len;
                defer self.env.shrinkRetainingCapacity(mark);
                const init = if (v.init) |i| try self.stmt(i) else null;
                break :blk self.newStmt(s.loc, .{ .for_ = .{
                    .init = init,
                    .cond = try self.opt(v.cond),
                    .update = try self.opt(v.update),
                    .body = try self.scoped(v.body),
                } });
            },
            .for_of => |v| blk: {
                const iterable = try self.expr(v.iterable);
                const mark = self.env.items.len;
                defer self.env.shrinkRetainingCapacity(mark);
                const name = try self.bind(v.name.name);
                break :blk self.newStmt(s.loc, .{ .for_of = .{
                    .name = .{ .name = name, .loc = self.loc(v.name.loc) },
                    .is_const = v.is_const,
                    .iterable = iterable,
                    .body = try self.scoped(v.body),
                } });
            },
            .while_ => |v| self.newStmt(s.loc, .{ .while_ = .{
                .cond = try self.expr(v.cond),
                .body = try self.scoped(v.body),
                .do_first = v.do_first,
            } }),
            .switch_ => |v| blk: {
                const value = try self.expr(v.value);
                const mark = self.env.items.len;
                defer self.env.shrinkRetainingCapacity(mark);
                for (v.cases) |case| try self.predeclare(case.body);
                const cases = try self.arena.alloc(ast.Case, v.cases.len);
                for (v.cases, cases) |case, *c| {
                    const body = try self.arena.alloc(*ast.Stmt, case.body.len);
                    for (case.body, body) |b, *o| o.* = try self.stmt(b);
                    c.* = .{ .loc = self.loc(case.loc), .value = try self.opt(case.value), .body = body };
                }
                break :blk self.newStmt(s.loc, .{ .switch_ = .{ .value = value, .cases = cases } });
            },
            .break_ => self.newStmt(s.loc, .break_),
            .continue_ => self.newStmt(s.loc, .continue_),
            .expr => |e| self.newStmt(s.loc, .{ .expr = try self.expr(e) }),
            .block => |ss| self.newStmt(s.loc, .{ .block = try self.statements(ss) }),
        };
    }

    /// A statement in its own scope (the body of an `if` or a loop).
    fn scoped(self: *Rewriter, s: *const ast.Stmt) Error!*ast.Stmt {
        if (s.data == .block) return self.stmt(s);
        const mark = self.env.items.len;
        defer self.env.shrinkRetainingCapacity(mark);
        return self.stmt(s);
    }

    fn pattern(self: *Rewriter, p: ast.Pattern) Error!ast.Pattern {
        return switch (p) {
            .ident => |n| .{ .ident = try self.renamed(n) },
            .array => |names| blk: {
                const out = try self.arena.alloc(?ast.Name, names.len);
                for (names, out) |n, *o| o.* = if (n) |v| try self.renamed(v) else null;
                break :blk .{ .array = out };
            },
            .object => |bindings| blk: {
                const out = try self.arena.alloc(ast.ObjectBinding, bindings.len);
                for (bindings, out) |b, *o| o.* = .{ .key = b.key, .local = try self.renamed(b.local) };
                break :blk .{ .object = out };
            },
        };
    }

    pub fn function(self: *Rewriter, f: *const ast.Function) Error!*ast.Function {
        const mark = self.env.items.len;
        defer self.env.shrinkRetainingCapacity(mark);
        const params = try self.arena.alloc(ast.Param, f.params.len);
        for (f.params, params) |p, *o| {
            o.* = p;
            o.ty = if (p.ty) |t| try self.typeRef(t) else null;
            o.default = try self.opt(p.default);
            if (p.destructure.len > 0) {
                // `{ todo }: Props` reads props by these names: they keep them
                const defaults = try self.arena.alloc(?*ast.Expr, p.destructure_defaults.len);
                for (p.destructure_defaults, defaults) |d, *od| od.* = try self.opt(d);
                o.destructure_defaults = defaults;
                for (p.destructure) |n| {
                    try self.env.append(self.arena, .{ .name = n.name, .binding = .{ .rename = n.name } });
                }
                continue;
            }
            o.name = .{ .name = try self.bind(p.name.name), .loc = self.loc(p.name.loc) };
        }
        const copy = try self.arena.create(ast.Function);
        copy.* = .{
            .loc = self.loc(f.loc),
            .name = f.name,
            .params = params,
            .ret = if (f.ret) |r| try self.typeRef(r) else null,
            .body = switch (f.body) {
                .block => |ss| .{ .block = try self.statements(ss) },
                .expr => |e| .{ .expr = try self.expr(e) },
            },
        };
        return copy;
    }

    fn newExpr(self: *Rewriter, at: ast.Loc, data: ast.Expr.Data) Error!*ast.Expr {
        const e = try self.arena.create(ast.Expr);
        e.* = .{ .loc = self.loc(at), .data = data };
        return e;
    }

    fn exprs(self: *Rewriter, es: []const *ast.Expr) Error![]const *ast.Expr {
        const out = try self.arena.alloc(*ast.Expr, es.len);
        for (es, out) |e, *o| o.* = try self.expr(e);
        return out;
    }

    pub fn expr(self: *Rewriter, e: *const ast.Expr) Error!*ast.Expr {
        return switch (e.data) {
            .number, .string, .boolean, .null_lit => self.newExpr(e.loc, e.data),
            .ident => |n| self.ident(e.loc, n),
            .template => |t| self.newExpr(e.loc, .{ .template = .{
                .quasis = t.quasis,
                .exprs = try self.exprs(t.exprs),
            } }),
            .unary => |u| self.newExpr(e.loc, .{ .unary = .{ .op = u.op, .operand = try self.expr(u.operand) } }),
            .binary => |b| self.newExpr(e.loc, .{ .binary = .{
                .op = b.op,
                .lhs = try self.expr(b.lhs),
                .rhs = try self.expr(b.rhs),
            } }),
            .cond => |v| self.newExpr(e.loc, .{ .cond = .{
                .cond = try self.expr(v.cond),
                .then = try self.expr(v.then),
                .else_ = try self.expr(v.else_),
            } }),
            .call => |v| blk: {
                const type_args = try self.arena.alloc(ast.TypeRef, v.type_args.len);
                for (v.type_args, type_args) |t, *o| o.* = try self.typeRef(t);
                break :blk self.newExpr(e.loc, .{ .call = .{
                    .callee = try self.expr(v.callee),
                    .type_args = type_args,
                    .args = try self.exprs(v.args),
                } });
            },
            .member => |m| self.member(e, m),
            .arrow => |f| self.newExpr(e.loc, .{ .arrow = try self.function(f) }),
            .jsx => |j| self.newExpr(e.loc, .{ .jsx = try self.jsx(j) }),
            .object => |props| blk: {
                const out = try self.arena.alloc(ast.Property, props.len);
                for (props, out) |p, *o| o.* = .{ .key = p.key, .value = try self.expr(p.value), .spread = p.spread };
                break :blk self.newExpr(e.loc, .{ .object = out });
            },
            .array => |items| self.newExpr(e.loc, .{ .array = try self.exprs(items) }),
            .spread => |inner| self.newExpr(e.loc, .{ .spread = try self.expr(inner) }),
            .non_null => |inner| self.newExpr(e.loc, .{ .non_null = try self.expr(inner) }),
            .index => |ix| self.newExpr(e.loc, .{ .index = .{
                .object = try self.expr(ix.object),
                .index = try self.expr(ix.index),
                .optional = ix.optional,
            } }),
            .as => |cast| self.newExpr(e.loc, .{ .as = .{
                .expr = try self.expr(cast.expr),
                .ty = try self.typeRef(cast.ty),
            } }),
            .assign => |v| self.newExpr(e.loc, .{ .assign = .{
                .op = v.op,
                .target = try self.expr(v.target),
                .value = try self.expr(v.value),
            } }),
            .update => |v| self.newExpr(e.loc, .{ .update = .{ .op = v.op, .target = try self.expr(v.target) } }),
        };
    }

    fn ident(self: *Rewriter, at: ast.Loc, n: []const u8) Error!*ast.Expr {
        if (self.lookup(n)) |b| return switch (b) {
            .rename => |new| self.newExpr(at, .{ .ident = new }),
            .expr => |value| value,
            .object => self.fail(
                at | self.loc_tag,
                "`{s}` is the object a hook returned: read its fields (`{s}.name`)",
                .{ n, n },
            ),
        };
        const name = if (self.free) |free| try free.resolve(free.ctx, n, at | self.loc_tag, .value) else n;
        return self.newExpr(at, .{ .ident = name });
    }

    fn member(self: *Rewriter, e: *const ast.Expr, m: ast.Member) Error!*ast.Expr {
        if (m.object.data == .ident) if (self.lookup(m.object.data.ident)) |b| if (b == .object) {
            for (b.object) |p| if (!p.spread and std.mem.eql(u8, p.key.name, m.name.name)) return p.value;
            return self.fail(e.loc | self.loc_tag, "the hook's result has no field `{s}`", .{m.name.name});
        };
        return self.newExpr(e.loc, .{ .member = .{
            .object = try self.expr(m.object),
            .name = m.name,
            .optional = m.optional,
        } });
    }

    fn jsx(self: *Rewriter, j: *const ast.Jsx) Error!*ast.Jsx {
        const attrs = try self.arena.alloc(ast.Attr, j.attrs.len);
        for (j.attrs, attrs) |a, *o| o.* = .{ .name = a.name, .value = try self.opt(a.value), .spread = a.spread };
        const children = try self.arena.alloc(ast.Child, j.children.len);
        for (j.children, children) |child, *o| o.* = switch (child) {
            .text => child,
            .expr => |v| .{ .expr = try self.opt(v) },
            .element => |el| .{ .element = try self.jsx(el) },
        };
        const copy = try self.arena.create(ast.Jsx);
        copy.* = .{ .loc = self.loc(j.loc), .name = try self.tagName(j), .attrs = attrs, .children = children };
        return copy;
    }

    /// `<Row>` names a component (or a tag) the hook's file imported.
    fn tagName(self: *Rewriter, j: *const ast.Jsx) Error![]const u8 {
        const free = self.free orelse return j.name;
        if (j.name.len == 0) return j.name;
        const dot = std.mem.indexOfScalar(u8, j.name, '.') orelse j.name.len;
        const head = try free.resolve(free.ctx, j.name[0..dot], j.loc | self.loc_tag, .tag);
        if (head.ptr == j.name.ptr and head.len == dot) return j.name;
        return std.fmt.allocPrint(self.arena, "{s}{s}", .{ head, j.name[dot..] });
    }

    pub fn typeRef(self: *Rewriter, t: ast.TypeRef) Error!ast.TypeRef {
        var out = t;
        out.loc = self.loc(t.loc);
        if (t.name.len > 0 and std.mem.indexOfScalar(u8, t.name, '.') == null) {
            if (self.free) |free| out.name = try free.resolve(free.ctx, t.name, t.loc | self.loc_tag, .type_name);
        }
        if (t.detail) |d| {
            const copy = try self.arena.create(ast.TypeDetail);
            copy.* = switch (d.*) {
                .array => |item| .{ .array = try self.typeRef(item) },
                .union_ => |members| blk: {
                    const ms = try self.arena.alloc(ast.TypeRef, members.len);
                    for (members, ms) |m, *o| o.* = try self.typeRef(m);
                    break :blk .{ .union_ = ms };
                },
                .string_lit, .null_ => d.*,
                .object => |fields| blk: {
                    const fs = try self.arena.alloc(ast.TypeField, fields.len);
                    for (fields, fs) |f, *o| {
                        o.* = .{ .name = f.name, .ty = try self.typeRef(f.ty), .optional = f.optional };
                    }
                    break :blk .{ .object = fs };
                },
                .func => |f| blk: {
                    const ps = try self.arena.alloc(ast.Param, f.params.len);
                    for (f.params, ps) |p, *o| {
                        o.* = p;
                        o.ty = if (p.ty) |pt| try self.typeRef(pt) else null;
                    }
                    break :blk .{ .func = .{ .params = ps, .ret = try self.typeRef(f.ret) } };
                },
            };
            out.detail = copy;
        }
        return out;
    }
};
