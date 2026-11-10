//! Public TSX generation entry, module validation, state collection, and component methods.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @This();
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const stmt = @import("stmt.zig");
const events = @import("events.zig");
const zig_text = @import("zig_text.zig");
const Type = types.Type;
const Context = scope.Context;
const Method = events.Method;
const Event = events.Event;
pub const Diag = @import("../parser.zig").Diag;
pub const Error = error{ Tsx, OutOfMemory };
pub const Options = struct {
    source_name: []const u8,
};

pub const State = struct {
    name: []const u8,
    setter: []const u8,
    ty: Type,
    initial: []const u8,
};

pub const Generator = struct {
    a: std.mem.Allocator,
    diag: *?Diag,
    name: []const u8 = "",
    states: std.ArrayList(State) = .empty,
    methods: std.ArrayList(Method) = .empty,
    events: std.ArrayList(Event) = .empty,
    render_names: std.ArrayList([]const u8) = .empty,
    hook_names: std.ArrayList([]const u8) = .empty,
    type_aliases: std.ArrayList(struct { name: []const u8, ty: Type }) = .empty,
    module_names: [4][]const u8 = .{ "std", "zant", "ui", "tsx" },
    counter: usize = 0,
};

pub fn fail(g: *Generator, loc: ast.Loc, comptime f: []const u8, args: anytype) Error {
    if (g.diag.* == null) g.diag.* = .{ .loc = loc, .msg = try zig_text.fmt(g, f, args) };
    return error.Tsx;
}

pub fn hookName(g: *Generator, n: []const u8) bool {
    if (zig_text.eq(n, "useState")) return true;
    for (g.hook_names.items) |name| if (zig_text.eq(n, name)) return true;
    return false;
}

pub fn hook(g: *Generator, e: *ast.Expr) bool {
    return e.data == .call and e.data.call.callee.data == .ident and core.hookName(g, e.data.call.callee.data.ident);
}

pub fn renderNames(g: *Generator, s: *ast.Stmt) Error!void {
    switch (s.data) {
        .decl => |d| if (d.pattern == .ident and (d.init == null or d.init.?.data != .arrow)) {
            try g.render_names.append(g.a, d.pattern.ident.name);
        },
        .block => |ss| {
            for (ss) |child| try core.renderNames(g, child);
        },
        .if_ => |v| {
            try core.renderNames(g, v.then);
            if (v.else_) |el| try core.renderNames(g, el);
        },
        else => {},
    }
}

/// Collect top-level hooks and component functions before resolving JSX event references.
pub fn collect(g: *Generator, f: *ast.Function) Error!void {
    for (f.body.block) |s| switch (s.data) {
        .decl => |d| try collectDeclaration(g, s.loc, d),
        .func => |func| try core.collectMethod(g, func.name, func),
        else => {},
    };
    for (f.body.block) |s| try core.renderNames(g, s);
    for (g.states.items) |s| {
        if (scope.methodIndex(g, s.name) != null or zig_text.eq(
            s.name,
            "render",
        )) return core.fail(g, f.loc, "state `{s}` conflicts with a method; rename it", .{s.name});
    }
}

pub fn collectMethod(g: *Generator, n: []const u8, f: *ast.Function) Error!void {
    const reserved = zig_text.eq(n, "render") or zig_text.eq(n, "self") or zig_text.eq(n, g.name);
    if (scope.methodIndex(g, n) != null or scope.stateIndex(g, n) != null or reserved) {
        return core.fail(g, f.loc, "duplicate or reserved method name `{s}`; rename it", .{n});
    }
    try g.methods.append(
        g.a,
        .{ .name = n, .func = f, .ret = if (f.ret) |r| try types.typeRef(g, r, true) else .void_, .user = true },
    );
}

pub fn collectState(g: *Generator, loc: ast.Loc, d: ast.Decl, call: ast.Call) Error!void {
    if (!statePattern(d.pattern)) return core.fail(
        g,
        loc,
        "useState needs exactly two names: const [value, setValue] = useState(...)",
        .{},
    );
    if (!d.is_const) return core.fail(g, loc, "declare useState with const", .{});
    if (call.args.len != 1 or call.type_args.len > 1) return core.fail(
        g,
        loc,
        "useState takes one literal initializer and at most one type argument",
        .{},
    );
    const init = call.args[0];
    if (!types.isLiteral(init)) return core.fail(
        g,
        init.loc,
        "useState initializer must be a number, string, or boolean literal",
        .{},
    );
    var c = Context{ .g = g, .render = true, .ret = .node };
    const target: ?Type = if (call.type_args.len == 1) try types.typeRef(g, call.type_args[0], false) else null;
    var v = try expr.emit(g, &c, init, target, false);
    v = try types.coerce(g, v, target orelse v.ty.concrete(), init.loc);
    const n = d.pattern.array[0].?.name;
    const setter = d.pattern.array[1].?.name;
    const value_conflict = scope.stateIndex(g, n) != null or scope.setterIndex(g, n) != null;
    const setter_conflict = scope.stateIndex(g, setter) != null or scope.setterIndex(g, setter) != null;
    if (zig_text.eq(n, setter) or value_conflict or setter_conflict) {
        return core.fail(g, loc, "duplicate state or setter name; rename `{s}`", .{n});
    }
    try g.states.append(g.a, .{ .name = n, .setter = setter, .ty = v.ty, .initial = v.code });
}

/// Emit a method with inferred event parameters and only the state snapshots it reads.
pub fn emitMethod(g: *Generator, m: Method) Error![]const u8 {
    if (m.setter != null) return events.emitSetter(g, m);
    const f = m.func.?;
    var c = Context{ .g = g, .render = false, .ret = m.ret };
    var params: std.ArrayList([]const u8) = .empty;
    for (f.params) |p| {
        const t = try events.parameterType(g, p, m.action);
        const b = try scope.bind(&c, p.name.name, t, false);
        try c.params.append(g.a, b);
        try params.append(g.a, try zig_text.fmt(g, ", {s}: {s}", .{ b.zig, t.zig() }));
    }
    if (m.action) |kind| if (kind != .action and f.params.len == 0) {
        try params.append(g.a, try zig_text.fmt(g, ", _: {s}", .{events.actionType(kind).zig()}));
    };
    const body = switch (f.body) {
        .block => |ss| try methodBlock(g, &c, f.loc, ss),
        .expr => |e| try methodExpression(g, &c, e),
    };
    return zig_text.fmt(
        g,
        "fn {s}(self: *{s}{s}) {s} {{\n{s}}}\n",
        .{
            try zig_text.id(g, m.name),
            try zig_text.id(g, g.name),
            try std.mem.join(g.a, "", params.items),
            m.ret.zig(),
            try scope.finish(&c, body),
        },
    );
}

/// Validate the module, register state and handlers, then emit an initialized component struct.
pub fn generate(
    arena: std.mem.Allocator,
    src: []const u8,
    module: ast.Module,
    options: Options,
    diag: *?Diag,
) Error![]const u8 {
    _ = src;
    diag.* = null;
    var g = Generator{ .a = arena, .diag = diag };
    for (module.imports) |imp| {
        if (!zig_text.eq(
            imp.source,
            "zant",
        )) return core.fail(&g, imp.source_loc, "only imports from \"zant\" are supported", .{});
        for (imp.names) |n| {
            const tag = tags.find(n.imported.name);
            if (!zig_text.eq(
                n.imported.name,
                "useState",
            ) and tag == null and !types.numericName(n.imported.name)) return core.fail(
                &g,
                n.imported.loc,
                "unknown import `{s}`; import useState, a tag, or a numeric type from zant",
                .{n.imported.name},
            );
            if (!zig_text.eq(n.imported.name, n.local.name)) {
                if (tag != null) return core.fail(
                    &g,
                    n.local.loc,
                    "tag imports cannot be renamed; use `{s}`",
                    .{n.imported.name},
                );
                if (zig_text.eq(n.imported.name, "useState")) {
                    try g.hook_names.append(arena, n.local.name);
                } else {
                    try g.type_aliases.append(
                        arena,
                        .{ .name = n.local.name, .ty = std.meta.stringToEnum(Type, n.imported.name).? },
                    );
                }
            }
        }
    }
    var component: ?*ast.Function = null;
    for (module.functions) |top| if (top.default) {
        if (component != null) return core.fail(
            &g,
            top.func.loc,
            "exactly one component is supported; child components arrive in phase 2",
            .{},
        );
        component = top.func;
    };
    const f = component orelse return core.fail(
        &g,
        0,
        "missing default exported component; use export default function App()",
        .{},
    );
    for (module.functions) |top| {
        if (top.func != f) return core.fail(
            &g,
            top.func.loc,
            "other top-level functions are not supported; child components arrive in phase 2",
            .{},
        );
    }
    if (f.params.len > 0) return core.fail(
        &g,
        f.params[0].name.loc,
        "component parameters are not supported; props arrive in phase 2",
        .{},
    );
    if (reservedComponentName(f.name)) {
        return core.fail(&g, f.loc, "component name `{s}` is reserved; rename it", .{f.name});
    }
    if (f.ret) |ref| if (!zig_text.eq(
        ref.name,
        "JSX.Element",
    ) and !zig_text.eq(ref.name, "React.ReactNode") and !zig_text.eq(ref.name, "ReactNode")) {
        return core.fail(&g, ref.loc, "component return type must be JSX.Element, React.ReactNode, or ReactNode", .{});
    };
    g.name = f.name;
    try core.collect(&g, f);
    try scope.moduleNames(&g);
    try events.scanBody(&g, f.body);
    const body = f.body.block;
    if (body.len == 0 or body[body.len - 1].data != .ret or !stmt.returnsAll(body)) {
        return core.fail(
            &g,
            f.loc,
            "render must return a node on every path; the final statement must be a return",
            .{},
        );
    }
    var c = Context{ .g = &g, .render = true, .ret = .node };
    const render_body = try stmt.statements(&g, &c, body, true);
    var out: std.ArrayList(u8) = .empty;
    // Source names are comments, so strip newlines supplied by a caller.
    const source_name = try std.mem.replaceOwned(u8, arena, options.source_name, "\n", " ");
    try out.appendSlice(arena, try zig_text.fmt(&g, "//! Generated by zant-tsx from {s}. Do not edit.\nconst std = " ++
        "@import(\"std\");\nconst zant = @import(\"zant\");\nconst ui = " ++
        "zant.ui;\nconst tsx = zant.tsx;\n\npub const {s} = struct {{\n", .{
        source_name,
        try zig_text.id(&g, f.name),
    }));
    for (g.states.items) |s| {
        const initial = if (s.ty == .string) try zig_text.fmt(&g, ".{{ .initial = {s} }}", .{s.initial}) else s.initial;
        try out.appendSlice(
            arena,
            try zig_text.fmt(
                &g,
                "{s}: {s} = {s},\n",
                .{ try zig_text.id(&g, s.name), if (s.ty == .string) "tsx.String" else s.ty.zig(), initial },
            ),
        );
    }
    if (g.states.items.len > 0) try out.appendSlice(arena, "\n");
    try out.appendSlice(
        arena,
        try zig_text.fmt(
            &g,
            "pub fn render(self: *{s}) ui.Node {{\n{s}}}\n",
            .{ try zig_text.id(&g, f.name), try scope.finish(&c, render_body) },
        ),
    );
    for (g.methods.items) |m| {
        try out.appendSlice(arena, "\n");
        try out.appendSlice(arena, try core.emitMethod(&g, m));
    }
    try out.appendSlice(arena, "};\n");
    return scope.renameModules(&g, out.items);
}

/// Validate destructuring with short circuit checks before accessing its names.
fn statePattern(pattern: ast.Pattern) bool {
    return switch (pattern) {
        .array => |names| names.len == 2 and names[0] != null and names[1] != null,
        else => false,
    };
}

/// Keep reserved import and receiver names out of the component declaration.
fn reservedComponentName(name: []const u8) bool {
    inline for (.{ "self", "std", "zant", "ui", "tsx" }) |reserved| {
        if (zig_text.eq(name, reserved)) return true;
    }
    return false;
}

/// Check complete returns before emitting a component method's statements.
fn methodBlock(g: *Generator, c: *Context, loc: ast.Loc, ss: []const *ast.Stmt) Error![]const u8 {
    if (c.ret != .void_ and !stmt.returnsAll(ss)) {
        return core.fail(g, loc, "function must return a value on every path", .{});
    }
    return stmt.statements(g, c, ss, false);
}

/// Treat a concise arrow as a statement or return according to its declared type.
fn methodExpression(g: *Generator, c: *Context, e: *ast.Expr) Error![]const u8 {
    if (c.ret == .void_) return stmt.expressionStatement(g, c, e);
    const value = try expr.emit(g, c, e, c.ret, false);
    return zig_text.fmt(g, "return {s};\n", .{value.code});
}

/// Collect top-level state and functions without adding derived render locals.
fn collectDeclaration(g: *Generator, loc: ast.Loc, declaration: ast.Decl) Error!void {
    const init = declaration.init orelse return;
    if (core.hook(g, init)) return core.collectState(g, loc, declaration, init.data.call);
    if (init.data == .arrow and declaration.pattern == .ident) {
        try core.collectMethod(g, declaration.pattern.ident.name, init.data.arrow);
    }
}
