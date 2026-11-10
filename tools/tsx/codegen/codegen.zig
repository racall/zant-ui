//! Public TSX generation entry, module validation, state collection, and component methods.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @This();
const model = @import("model.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const stmt = @import("stmt.zig");
const events = @import("events.zig");
const components = @import("components.zig");
const zig_text = @import("zig_text.zig");
const Type = types.Type;
const Context = scope.Context;
const Method = events.Method;
const Event = events.Event;
pub const Diag = @import("../parser.zig").Diag;
pub const Error = error{ Tsx, OutOfMemory };
pub const Options = struct {
    source_name: []const u8,
    /// The project's parsed files, for relative imports.
    project: []const @import("imports.zig").NamedModule = &.{},
    /// Hand-written Zig files parsed for function signatures.
    zig_sources: []const @import("zig_imports.zig").Source = &.{},
    /// A file without a root component (only child components) is an error
    /// unless false (library files of a project).
    require_root: bool = true,
};

pub const State = struct {
    name: []const u8,
    /// Empty for a ref.
    setter: []const u8,
    ty: Type,
    initial: []const u8,
    /// `useState` or `useRef` (read and written through `.current`).
    kind: enum { state, ref } = .state,
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
    managed_strings: bool = false,
    compounds: std.ArrayList(model.Info) = .empty,
    type_decls: []const ast.TypeDecl = &.{},
    resolving: []bool = &.{},
    resolved_types: std.ArrayList(struct { detail: *const ast.TypeDetail, ty: Type }) = .empty,
    constants: std.ArrayList(struct { name: []const u8, value: types.Value }) = .empty,
    /// Child components of the module, and the one being generated (null
    /// while generating the root component).
    components: std.ArrayList(components.Component) = .empty,
    current_component: ?*components.Component = null,
    callback_events: std.ArrayList(components.CallbackEvent) = .empty,
    /// Types imported from other files, and the other file whose
    /// declarations are being resolved (null: this file).
    imported_types: std.ArrayList(@import("imports.zig").ImportedType) = .empty,
    type_scope: ?*const @import("imports.zig").Scope = null,
    zig_sources: []const @import("zig_imports.zig").Source = &.{},
    zig_functions: std.ArrayList(@import("zig_imports.zig").Function) = .empty,
    /// `<List>`s of the component being generated.
    lists: std.ArrayList(@import("list.zig").ListInfo) = .empty,
    /// The project's files (stateful analysis of imported components).
    project: []const @import("imports.zig").NamedModule = &.{},
    /// Key-less stateful child call sites of the current component, numbered
    /// for their instance ids.
    child_sites: usize = 0,
    /// `useMemo` caches and `useEffect` dependency lists of the current
    /// component (fields next to its state), and its effects.
    hook_fields: std.ArrayList(@import("hooks.zig").Field) = .empty,
    effects: std.ArrayList(@import("hooks.zig").Effect) = .empty,
    cleanups: std.ArrayList(@import("hooks.zig").Cleanup) = .empty,
    /// Stores declared in or imported into this file (`stores.zig`).
    stores: std.ArrayList(@import("stores.zig").Store) = .empty,
};

pub fn fail(g: *Generator, loc: ast.Loc, comptime f: []const u8, args: anytype) Error {
    if (g.diag.* == null) g.diag.* = .{ .loc = loc, .msg = try zig_text.fmt(g, f, args) };
    return error.Tsx;
}

/// Add schema context to the first diagnostic without moving its source caret.
pub fn contextualize(g: *Generator, prefix: []const u8) Error!void {
    const diagnostic = g.diag.*.?;
    g.diag.* = .{
        .loc = diagnostic.loc,
        .msg = try zig_text.fmt(g, "{s}: {s}", .{ prefix, diagnostic.msg }),
    };
}

pub fn hookName(g: *Generator, n: []const u8) bool {
    if (zig_text.eq(n, "useState") or @import("hooks.zig").extraHook(n) or zig_text.eq(n, "useStore")) return true;
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
        .expr => |e| if (@import("hooks.zig").callName(e)) |n| if (zig_text.eq(n, "useEffect")) {
            try @import("hooks.zig").collectEffect(g, e);
        },
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
    const aggregate_init = init.data == .array or init.data == .object or init.data == .null_lit or init.data == .ident;
    if (!types.isLiteral(init) and !aggregate_init) return core.fail(
        g,
        init.loc,
        "useState initializer must be a number, string, or boolean literal",
        .{},
    );
    var c = Context{ .g = g, .render = true, .ret = .node, .constant = true };
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
    if (m.emitted) |emitted| return emitted;
    if (m.setter != null) return events.emitSetter(g, m);
    return method(g, m, null, null);
}

pub fn method(g: *Generator, m: Method, source: ?*Context, index: ?usize) Error![]const u8 {
    const f = m.func.?;
    var c = Context{
        .g = g,
        .render = false,
        .ret = m.ret,
        .capture_source = source,
        .capture_method = index,
        .effect_field = m.effect_field,
        .cleanup = m.cleanup,
    };
    var params: std.ArrayList([]const u8) = .empty;
    for (f.params, 0..) |p, i| {
        const t = try events.methodParamType(g, m, i, p);
        const b = try scope.bind(&c, p.name.name, t, false);
        try c.params.append(g.a, b);
        try params.append(g.a, try zig_text.fmt(g, ", {s}: {s}", .{ b.zig, t.zig(g) }));
    }
    if (m.callback_params) |expected| {
        // an arrow may ignore trailing callback arguments
        if (f.params.len < expected.len) for (expected[f.params.len..]) |t| {
            try params.append(g.a, try zig_text.fmt(g, ", _: {s}", .{t.zig(g)}));
        };
    } else if (m.action) |kind| if (kind != .action and f.params.len == 0) {
        try params.append(g.a, try zig_text.fmt(g, ", _: {s}", .{events.actionType(kind).zig(g)}));
    };
    const body = switch (f.body) {
        .block => |ss| try methodBlock(g, &c, f.loc, ss),
        .expr => |e| try methodExpression(g, &c, e),
    };
    const finished = try scope.finish(&c, body);
    if (index) |i| if (g.methods.items[i].captures.items.len > 0) {
        return zig_text.fmt(g, "fn {s}(cap: *{s}{s}) {s} {{\nconst self: *{s} = cap.self;\n{s}}}\n", .{
            try zig_text.id(g, m.name), m.capture_name.?,           try std.mem.join(g.a, "", params.items),
            m.ret.zig(g),               try zig_text.id(g, g.name), finished,
        });
    };
    return zig_text.fmt(
        g,
        "fn {s}(self: *{s}{s}) {s} {{\n{s}}}\n",
        .{
            try zig_text.id(g, m.name),
            try zig_text.id(g, g.name),
            try std.mem.join(g.a, "", params.items),
            m.ret.zig(g),
            finished,
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
    var g = Generator{ .a = arena, .diag = diag, .project = options.project };
    g.zig_sources = options.zig_sources;
    g.type_decls = module.types;
    g.resolving = try arena.alloc(bool, module.types.len);
    @memset(g.resolving, false);
    for (module.imports) |imp| {
        if (@import("imports.zig").isRelative(imp.source)) continue;
        if (!zig_text.eq(
            imp.source,
            "zant",
        )) return core.fail(&g, imp.source_loc, "imports come from \"zant\" or another .tsx file (\"./Name\")", .{});
        for (imp.names) |n| {
            const tag = tags.find(n.imported.name);
            const hook_import = zig_text.eq(n.imported.name, "useState") or zig_text.eq(n.imported.name, "useTheme") or
                zig_text.eq(n.imported.name, "List") or @import("hooks.zig").extraHook(n.imported.name) or
                zig_text.eq(n.imported.name, "createStore") or zig_text.eq(n.imported.name, "useStore");
            if (!hook_import and tag == null and !types.numericName(n.imported.name)) return core.fail(
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
                } else if (!types.numericName(n.imported.name)) {
                    return core.fail(&g, n.local.loc, "`{s}` cannot be renamed", .{n.imported.name});
                } else {
                    try g.type_aliases.append(
                        arena,
                        .{ .name = n.local.name, .ty = std.meta.stringToEnum(Type, n.imported.name).? },
                    );
                }
            }
        }
    }
    for (module.types) |decl| _ = try model.named(&g, decl.name.name);
    const import_header = try @import("imports.zig").process(&g, module, options.project);
    // the root is the default export without props (the app's state); a
    // file of a project may hold child components only
    var root: ?*ast.Function = null;
    for (module.functions) |top| if (top.default) {
        if (root != null) return core.fail(&g, top.func.loc, "only one component can be the default export", .{});
        if (top.func.params.len > 0 and !options.require_root) continue;
        root = top.func;
    };
    if (root == null and options.require_root) return core.fail(
        &g,
        0,
        "missing default exported component; use export default function App()",
        .{},
    );
    if (root) |f| {
        if (f.params.len > 0) return core.fail(
            &g,
            f.params[0].name.loc,
            "the root component takes no props; it is the app's state",
            .{},
        );
        if (reservedComponentName(f.name)) {
            return core.fail(&g, f.loc, "component name `{s}` is reserved; rename it", .{f.name});
        }
    }
    const store_code = try @import("stores.zig").collect(&g, module.consts);
    const plain = try @import("stores.zig").plainConstants(&g, module.consts);
    const globals = try std.mem.concat(arena, u8, &.{ try @import("globals.zig").collect(&g, plain), store_code });
    try components.collect(&g, module, root);
    // children first, then the root; the header needs every compound type
    var bodies: std.ArrayList(u8) = .empty;
    for (g.components.items) |*comp| {
        if (comp.external) continue;
        g.current_component = comp;
        try bodies.appendSlice(arena, try emitComponent(&g, comp.func, comp));
        try bodies.appendSlice(arena, "\n");
    }
    g.current_component = null;
    if (root) |f| try bodies.appendSlice(arena, try emitComponent(&g, f, null));
    var out: std.ArrayList(u8) = .empty;
    // Source names are comments, so strip newlines supplied by a caller.
    const source_name = try std.mem.replaceOwned(u8, arena, options.source_name, "\n", " ");
    try out.appendSlice(arena, try zig_text.fmt(&g, "//! Generated by zant-tsx from {s}. Do not edit.\nconst std = " ++
        "@import(\"std\");\nconst zant = @import(\"zant\");\nconst ui = " ++
        "zant.ui;\nconst tsx = zant.tsx;\n{s}\n{s}{s}", .{
        source_name,
        import_header,
        try @import("globals.zig").declarations(&g),
        globals,
    }));
    try out.appendSlice(arena, bodies.items);
    return scope.renameModules(&g, out.items);
}

/// Per-component generator state starts empty for each component; types,
/// constants and import aliases are shared by the whole module.
fn resetComponent(g: *Generator) void {
    g.states.clearRetainingCapacity();
    g.methods.clearRetainingCapacity();
    g.events.clearRetainingCapacity();
    g.render_names.clearRetainingCapacity();
    g.callback_events.clearRetainingCapacity();
    g.lists.clearRetainingCapacity();
    g.managed_strings = false;
    g.child_sites = 0;
    g.hook_fields.clearRetainingCapacity();
    g.effects.clearRetainingCapacity();
    g.cleanups.clearRetainingCapacity();
}

/// One component as a struct: the root with its state fields, or a child
/// with `props` and a builder function after the struct.
fn emitComponent(g: *Generator, f: *ast.Function, comp: ?*components.Component) Error![]const u8 {
    const arena = g.a;
    resetComponent(g);
    if (f.ret) |ref| if (!zig_text.eq(
        ref.name,
        "JSX.Element",
    ) and !zig_text.eq(ref.name, "React.ReactNode") and !zig_text.eq(ref.name, "ReactNode")) {
        return core.fail(g, ref.loc, "component return type must be JSX.Element, React.ReactNode, or ReactNode", .{});
    };
    g.name = f.name;
    try core.collect(g, f);
    try scope.moduleNames(g);
    try events.scanBody(g, f.body);
    const body = f.body.block;
    if (body.len == 0 or body[body.len - 1].data != .ret or !stmt.returnsAll(body)) {
        return core.fail(
            g,
            f.loc,
            "render must return a node on every path; the final statement must be a return",
            .{},
        );
    }
    var c = Context{ .g = g, .render = true, .ret = .node };
    const statements = try stmt.statements(g, &c, body, true);
    var methods: std.ArrayList([]const u8) = .empty;
    for (g.methods.items) |m| try methods.append(arena, try core.emitMethod(g, m));
    for (g.states.items) |state| if (model.owned(g, state.ty)) {
        g.managed_strings = true;
    };
    for (g.methods.items) |m| if (m.captures.items.len > 0) {
        g.managed_strings = true;
    };
    var render_prefix: std.ArrayList(u8) = .empty;
    for (g.states.items) |state| if (model.owned(g, state.ty) or (state.ty == .string and g.managed_strings)) {
        try render_prefix.appendSlice(arena, try zig_text.fmt(g, "self.{s}.sweep();\n", .{
            try scope.stateField(g, state.name),
        }));
    };
    const render_body = try zig_text.fmt(g, "{s}{s}", .{ render_prefix.items, statements });
    const name = try zig_text.id(g, f.name);
    var out: std.ArrayList(u8) = .empty;
    try out.appendSlice(arena, try zig_text.fmt(g, "pub const {s} = struct {{\n", .{name}));
    const fields = try stateFields(g);
    if (comp) |child| {
        const props_type = if (child.props) |t| t.zig(g) else "struct {}";
        try out.appendSlice(arena, "props: Props,\n");
        // stateful children: their instance id, and the state kept in a
        // `ui.localState` slot while the instance stays mounted
        if (child.stateful) try out.appendSlice(arena, "_id: u64,\n");
        if (fields.len > 0) try out.appendSlice(arena, "_s: *_Local,\n");
        try out.appendSlice(arena, try zig_text.fmt(g, "\npub const Props = {s};\n\n", .{props_type}));
        if (fields.len > 0) try out.appendSlice(arena, try zig_text.fmt(
            g,
            "pub const _Local = struct {{\n{s}\n" ++
                "pub fn deinit(self: *_Local) void {{\ntsx.deinitFields(self);\n}}\n}};\n\n",
            .{fields},
        ));
    } else if (fields.len > 0) {
        try out.appendSlice(arena, fields);
        try out.appendSlice(arena, "\n");
    }
    try out.appendSlice(arena, try @import("captures.zig").declarations(g));
    try out.appendSlice(arena, try zig_text.fmt(
        g,
        "pub fn render(self: *{s}) ui.Node {{\n{s}}}\n",
        .{ name, try scope.finish(&c, render_body) },
    ));
    for (methods.items) |code| {
        try out.appendSlice(arena, "\n");
        try out.appendSlice(arena, code);
    }
    try out.appendSlice(arena, "};\n");
    if (comp) |child| try out.appendSlice(arena, try builderFunction(g, child, name, fields.len > 0));
    return out.items;
}

/// State fields and hidden `ListState`s with their initial values.
fn stateFields(g: *Generator) Error![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    for (g.states.items) |s| {
        const wrapped = s.ty == .string or model.owned(g, s.ty);
        const initial = if (wrapped) try zig_text.fmt(g, ".{{ .initial = {s} }}", .{s.initial}) else s.initial;
        const state_type = if (s.ty == .string and !g.managed_strings) "tsx.String" else if (wrapped)
            try zig_text.fmt(g, "tsx.State({s})", .{s.ty.zig(g)})
        else
            s.ty.zig(g);
        try out.appendSlice(g.a, try zig_text.fmt(g, "{s}: {s} = {s},\n", .{
            try zig_text.id(g, s.name), state_type, initial,
        }));
    }
    for (g.lists.items) |l| {
        const field = "{s}: ui.ListState = .{{ .allocator = std.heap.smp_allocator }},\n";
        try out.appendSlice(g.a, try zig_text.fmt(g, field, .{l.field}));
    }
    for (g.hook_fields.items) |f| {
        try out.appendSlice(g.a, try zig_text.fmt(g, "{s}: {s} = .{{}},\n", .{ f.name, f.zig_type }));
    }
    return out.items;
}

/// `pub fn todoRow(props: TodoRow.Props[, id: u64]) ui.Node`.
fn builderFunction(g: *Generator, child: *const components.Component, name: []const u8, local: bool) Error![]const u8 {
    const builder = try zig_text.id(g, child.builder);
    if (!child.stateful) return zig_text.fmt(
        g,
        "\npub fn {s}(props: {s}.Props) ui.Node {{\nreturn tsx.capture({s}{{ .props = props }}).render();\n}}\n",
        .{ builder, name, name },
    );
    const slot = if (local) try zig_text.fmt(g, ", ._s = ui.localState({s}._Local, id, .{{}})", .{name}) else "";
    return zig_text.fmt(
        g,
        "\npub fn {s}(props: {s}.Props, id: u64) ui.Node {{\n" ++
            "return tsx.capture({s}{{ .props = props, ._id = id{s} }}).render();\n}}\n",
        .{ builder, name, name, slot },
    );
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
    if (core.hook(g, init)) {
        const n = init.data.call.callee.data.ident;
        if (zig_text.eq(n, "useRef")) return @import("hooks.zig").collectRef(g, loc, declaration, init.data.call);
        if (zig_text.eq(n, "useMemo") or zig_text.eq(n, "useStore")) return; // render locals (stmt.declaration)
        if (zig_text.eq(n, "useEffect")) return core.fail(g, loc, "useEffect returns nothing", .{});
        return core.collectState(g, loc, declaration, init.data.call);
    }
    if (init.data == .arrow and declaration.pattern == .ident) {
        try core.collectMethod(g, declaration.pattern.ident.name, init.data.arrow);
    }
}
