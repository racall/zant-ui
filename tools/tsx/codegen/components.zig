//! Child components (docs/tsx.md, "子组件"): functions other than the default
//! export, called as tags. Each becomes a struct whose `props` field plays the
//! part state plays in the root component, with its builder inside:
//!
//!     pub const TodoRow = struct {
//!         props: Props,
//!         pub const Props = RowProps;
//!         pub fn build(props: Props) ui.Node { ... }   // called as `TodoRow.build(.{ … })`
//!     };
//!
//! The struct lives in the frame arena (`tsx.capture`), so handlers inside the
//! child keep working until the next frame, like every other callback.
//! Callback props take action types when their signature matches a ui event
//! (`() => void` → `actions.Action`, …) and `tsx.Callback(fn (...) void)` otherwise.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const events = @import("events.zig");
const captures = @import("captures.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

pub const Component = struct {
    /// TSX and Zig struct name (`TodoRow`).
    name: []const u8,
    /// Builder function (`todoRow`).
    builder: []const u8,
    func: *ast.Function,
    /// The props struct type (an empty struct when there is no parameter).
    props: ?Type,
    /// The parameter's name (`props`) when not destructured.
    param: []const u8 = "",
    /// Destructured prop names (`{ todo, onToggle }`) and their defaults
    /// (`{ size = 14 }`, `defaults.zig`).
    destructure: []const ast.Name = &.{},
    destructure_defaults: []const ?*ast.Expr = &.{},
    /// Imported from another file: `builder` is already Zig code
    /// (`_TodoRow.todoRow`) and no struct is generated here.
    external: bool = false,
    /// Keeps local state (`stateful.zig`): the builder takes an instance id.
    stateful: bool = false,
};

/// An inline arrow passed to a child component's callback prop.
pub const CallbackEvent = struct {
    expr: *ast.Expr,
    method: usize,
};

// ── types ─────────────────────────────────────────────────────────────

/// `(id: number) => void` as a callback type (interned by signature).
pub fn callbackType(g: *Generator, loc: ast.Loc, f: ast.FuncType) Error!Type {
    const ret = try types.typeRef(g, f.ret, true);
    if (ret != .void_) return core.fail(g, loc, "callback props must return void", .{});
    var params: std.ArrayList(Type) = .empty;
    for (f.params) |p| {
        const ref = p.ty orelse return core.fail(
            g,
            p.name.loc,
            "callback parameter `{s}` needs a type",
            .{p.name.name},
        );
        try params.append(g.a, try types.typeRef(g, ref, false));
    }
    for (g.compounds.items, 0..) |entry, i| {
        if (entry.kind == .callback and std.mem.eql(Type, entry.params, params.items)) {
            return @fromBackingInt(@intCast(types.compound_base + i));
        }
    }
    const kind = actionKind(params.items);
    const zig = if (kind) |k| try zig_text.fmt(g, "zant.actions.{s}", .{actionTypeName(k)}) else blk: {
        var list: std.ArrayList([]const u8) = .empty;
        for (params.items) |t| try list.append(g.a, t.zig(g));
        break :blk try zig_text.fmt(g, "tsx.Callback(fn ({s}) void)", .{try std.mem.join(g.a, ", ", list.items)});
    };
    return model.add(g, .{ .kind = .callback, .zig = zig, .params = params.items });
}

/// The ui action kind a callback signature matches, if any.
pub fn actionKind(params: []const Type) ?tags.Kind {
    if (params.len == 0) return .action;
    if (params.len != 1) return null;
    return switch (params[0]) {
        .boolean => .bool_action,
        .string => .text_action,
        .usize => .index_action,
        else => null,
    };
}

fn actionTypeName(kind: tags.Kind) []const u8 {
    return switch (kind) {
        .action => "Action",
        .bool_action => "BoolAction",
        .text_action => "TextAction",
        .index_action => "IndexAction",
        else => unreachable,
    };
}

/// The callback info of `ty` (or of its optional child).
pub fn callbackInfo(g: *Generator, ty: Type) ?*model.Info {
    const base = model.optionalChild(g, ty) orelse ty;
    const entry = model.info(g, base) orelse return null;
    return if (entry.kind == .callback) entry else null;
}

// ── collection ────────────────────────────────────────────────────────

/// Registers every non-default top-level function as a child component.
pub fn collect(g: *Generator, module: ast.Module, root: ?*ast.Function) Error!void {
    for (module.functions) |top| {
        if (root != null and top.func == root.?) continue;
        if (@import("functions.zig").isFunction(top.func)) continue;
        const f = top.func;
        if (f.params.len > 1) return core.fail(g, f.params[1].name.loc, "components take one props object", .{});
        var comp = Component{
            .name = f.name,
            .builder = try builderName(g, f.name),
            .func = f,
            .props = null,
            .stateful = @import("stateful.zig").stateful(g.project, module, f),
        };
        if (f.params.len == 1) {
            const p = f.params[0];
            const ref = p.ty orelse return core.fail(
                g,
                p.name.loc,
                "props need a type: function {s}(props: {s}Props)",
                .{ f.name, f.name },
            );
            const ty = try types.typeRef(g, ref, false);
            const entry = model.info(g, ty);
            if (entry == null or entry.?.kind != .object) return core.fail(
                g,
                ref.loc,
                "props must be an object type",
                .{},
            );
            comp.props = ty;
            comp.param = p.name.name;
            comp.destructure = p.destructure;
            comp.destructure_defaults = p.destructure_defaults;
            for (p.destructure, 0..) |n, i| {
                const field = fieldType(g, ty, n.name) orelse {
                    return core.fail(g, n.loc, "`{s}` is not a prop of `{s}`", .{ n.name, ref.name });
                };
                const default = if (i < p.destructure_defaults.len) p.destructure_defaults[i] else null;
                if (default) |d| _ = try @import("defaults.zig").prop(g, n, d, field, "");
            }
        }
        for (g.components.items) |other| if (zig_text.eq(other.name, comp.name)) {
            return core.fail(g, f.loc, "duplicate component `{s}`", .{comp.name});
        };
        try g.components.append(g.a, comp);
    }
}

/// `TodoRow` → `TodoRow.build` (the builder inside the component's struct).
pub fn builderName(g: *Generator, name: []const u8) Error![]const u8 {
    if (name.len == 0 or !std.ascii.isUpper(name[0])) return core.fail(
        g,
        0,
        "component `{s}` must start with a capital letter",
        .{name},
    );
    return zig_text.fmt(g, "{s}.build", .{try zig_text.id(g, name)});
}

pub fn find(g: *Generator, name: []const u8) ?*Component {
    for (g.components.items) |*comp| if (zig_text.eq(comp.name, name)) return comp;
    return null;
}

fn fieldType(g: *Generator, props: Type, name: []const u8) ?Type {
    const entry = model.info(g, props) orelse return null;
    for (entry.fields) |f| if (zig_text.eq(f.name, name)) return f.ty;
    return null;
}

fn fields(g: *Generator, comp: *const Component) []const model.Field {
    const props = comp.props orelse return &.{};
    return model.info(g, props).?.fields;
}

// ── inside a child component ──────────────────────────────────────────

/// Reads a prop of the component being generated: `self.props.todo`, or the
/// whole object for a non-destructured `props`.
pub fn readProp(g: *Generator, n: []const u8) Error!?Value {
    const comp = g.current_component orelse return null;
    const props = comp.props orelse return null;
    for (comp.destructure, 0..) |d, i| if (zig_text.eq(d.name, n)) {
        const ty = fieldType(g, props, n).?;
        const code = try zig_text.fmt(g, "self.props.{s}", .{try zig_text.id(g, n)});
        if (i < comp.destructure_defaults.len) if (comp.destructure_defaults[i]) |default| {
            return try @import("defaults.zig").prop(g, d, default, ty, code);
        };
        return .{ .ty = ty, .code = code };
    };
    if (comp.destructure.len == 0 and zig_text.eq(n, comp.param)) return .{ .ty = props, .code = "self.props" };
    return null;
}

/// `onToggle(todo.id)` / `props.onToggle(todo.id)` inside a child: calls the
/// callback prop. Null when the callee is not a callback prop.
/// `object.name` is a callback field of the props object.
fn propsCallback(g: *Generator, m: ast.Member) Error!bool {
    const object = (try readProp(g, m.object.data.ident)) orelse return false;
    const entry = model.info(g, object.ty) orelse return false;
    if (entry.kind != .object) return false;
    const ty = fieldType(g, object.ty, m.name.name) orelse return false;
    return callbackInfo(g, ty) != null;
}

pub fn tryCall(g: *Generator, c: *Context, loc: ast.Loc, call: ast.Call) Error!?Value {
    if (g.current_component == null) return null;
    const callee_is_prop = switch (call.callee.data) {
        .ident => |n| scope.local(c, n) == null and (try readProp(g, n)) != null,
        // `props.onToggle(...)`, not an array method on a prop (`items.filter(...)`)
        .member => |m| m.object.data == .ident and scope.local(c, m.object.data.ident) == null and
            try propsCallback(g, m),
        else => false,
    };
    if (!callee_is_prop) return null;
    const callee = try expr.emit(g, c, call.callee, null, false);
    const entry = callbackInfo(g, callee.ty) orelse return null;
    if (model.optionalChild(g, callee.ty) != null) {
        return core.fail(g, loc, "optional callback props may be null; check first: `if (onX) onX(...)`", .{});
    }
    if (c.render) return core.fail(g, loc, "callback props can only be called in event handlers", .{});
    if (call.args.len != entry.params.len) {
        return core.fail(g, loc, "the callback takes {d} arguments", .{entry.params.len});
    }
    var args: std.ArrayList([]const u8) = .empty;
    for (call.args, entry.params) |arg, t| try args.append(g.a, (try expr.emit(g, c, arg, t, false)).code);
    const joined = try std.mem.join(g.a, ", ", args.items);
    const code = if (actionKind(entry.params) != null)
        try zig_text.fmt(
            g,
            "{s}.func({s}.ctx{s}{s})",
            .{ callee.code, callee.code, if (args.items.len > 0) ", " else "", joined },
        )
    else
        try zig_text.fmt(g, "{s}.call(.{{ {s} }})", .{ callee.code, joined });
    return .{ .ty = .void_, .code = code };
}

/// Whether `e` is this component's callback prop (`onSave` or
/// `props.onSave`) with the signature of the ui event `kind`; other
/// signatures are an error.
pub fn forwardedProp(g: *Generator, e: *ast.Expr, kind: tags.Kind, loc: ast.Loc) Error!bool {
    if (g.current_component == null) return false;
    const ty = switch (e.data) {
        .ident => |n| ((try readProp(g, n)) orelse return false).ty,
        .member => |m| blk: {
            if (m.object.data != .ident) return false;
            const object = (try readProp(g, m.object.data.ident)) orelse return false;
            break :blk fieldType(g, object.ty, m.name.name) orelse return false;
        },
        else => return false,
    };
    const entry = callbackInfo(g, ty) orelse return false;
    if (model.optionalChild(g, ty) != null) {
        return core.fail(g, loc, "an optional callback prop cannot be passed to a ui event; make it required", .{});
    }
    if (actionKind(entry.params) != kind) {
        return core.fail(
            g,
            loc,
            "this callback prop does not have the event's signature {s}",
            .{events.actionSignature(kind)},
        );
    }
    return true;
}

// ── call sites ────────────────────────────────────────────────────────

/// Registers inline arrows passed to callback props (before emission, like
/// ui event handlers) and scans everything else.
pub fn scan(g: *Generator, j: *ast.Jsx, comp: *const Component) Error!void {
    for (try @import("spread.zig").literals(g, j.attrs)) |attr| {
        if (attr.spread) {
            try events.scanExpr(g, attr.value.?);
            continue;
        }
        const value = attr.value orelse continue;
        const ty = fieldType(g, comp.props orelse continue, attr.name.name) orelse continue;
        const entry = callbackInfo(g, ty) orelse {
            try events.scanExpr(g, value);
            continue;
        };
        const f = switch (value.data) {
            .arrow => |f| f,
            .ident => |n| core.inlineFunction(g, n) orelse continue,
            else => continue,
        };
        g.counter += 1;
        const name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_{s}{d}", .{ attr.name.name, g.counter }));
        if (f.params.len > entry.params.len) return core.fail(
            g,
            value.loc,
            "the callback passes {d} arguments",
            .{entry.params.len},
        );
        const index = g.methods.items.len;
        try g.methods.append(g.a, .{
            .name = name,
            .func = f,
            .action = actionKind(entry.params),
            .callback_params = entry.params,
            .capture_name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_Capture{d}", .{g.counter})),
        });
        try g.callback_events.append(g.a, .{ .expr = value, .method = index });
        try events.scanBody(g, f.body);
    }
}

/// `<TodoRow todo={t} onToggle={toggle} />` → `todoRow(.{ ... })`.
pub fn emitCall(g: *Generator, c: *Context, j: *ast.Jsx, comp: *const Component) Error!Value {
    var parts: std.ArrayList([]const u8) = .empty;
    var key: ?[]const u8 = null;
    // the key as written (a string or an integer) for the instance id
    var raw_key: ?[]const u8 = null;
    var given: std.ArrayList([]const u8) = .empty;
    for (try @import("spread.zig").flatten(g, c, j.attrs)) |attr| {
        if (zig_text.eq(attr.name.name, "key")) {
            const v = attr.value orelse return core.fail(g, attr.name.loc, "key needs a value", .{});
            const k = try expr.emit(g, c, v, null, false);
            // keys are strings; numbers (`key={t.id}`) are formatted
            const concrete = if (k.ty.numeric()) (try types.concrete(g, k)).code else k.code;
            key = if (k.ty == .string) k.code else if (k.ty.numeric())
                try zig_text.fmt(g, "tsx.fmt(\"{{d}}\", .{{{s}}})", .{concrete})
            else
                return core.fail(g, v.loc, "key must be a string or a number", .{});
            raw_key = concrete;
            continue;
        }
        const ty = (if (comp.props) |p| fieldType(g, p, attr.name.name) else null) orelse return core.fail(
            g,
            attr.name.loc,
            "unknown prop `{s}` on <{s}>; props: {s}",
            .{ attr.name.name, comp.name, try fieldList(g, comp) },
        );
        const bare_error = "bare attribute `{s}` is only valid for boolean props";
        const code = if (attr.value) |v|
            try propValue(g, c, ty, v)
        else if (ty == .boolean)
            "true"
        else
            return core.fail(g, attr.name.loc, bare_error, .{attr.name.name});
        try parts.append(g.a, try zig_text.fmt(g, ".{s} = {s},\n", .{ try zig_text.id(g, attr.name.name), code }));
        try given.append(g.a, attr.name.name);
    }
    if (try childrenValue(g, c, j, comp)) |code| {
        try parts.append(g.a, try zig_text.fmt(g, ".children = {s},\n", .{code}));
        try given.append(g.a, "children");
    }
    for (fields(g, comp)) |f| {
        if (f.optional) continue;
        for (given.items) |n| {
            if (zig_text.eq(n, f.name)) break;
        } else return core.fail(g, j.loc, "<{s}> needs the prop `{s}`", .{ comp.name, f.name });
    }
    const builder = comp.builder;
    if (comp.stateful and raw_key == null and g.loop_depth > 0) return core.fail(
        g,
        j.loc,
        "<{s}> keeps its own state; give each one in a list a key (key={{item.id}}) so the state follows the item",
        .{comp.name},
    );
    const id = if (comp.stateful) try instanceId(g, comp, raw_key) else "";
    var node = try zig_text.fmt(g, "{s}(.{{\n{s}}}{s})", .{ builder, try std.mem.join(g.a, "", parts.items), id });
    if (key) |k| node = try zig_text.fmt(g, "ui.withKey({s}, {s})", .{ node, k });
    return .{ .ty = .node, .code = node };
}

/// `, tsx.childId(parent, "TodoRow", key)`: the parent's instance id (the
/// root's is 0) mixed with the component name and its key; without a key,
/// `tsx.siteId(parent, "TodoRow", n)` numbers the call site.
fn instanceId(g: *Generator, comp: *const Component, key: ?[]const u8) Error![]const u8 {
    const parent = if (g.current_component != null) "self._id" else "0";
    if (key) |k| return zig_text.fmt(g, ", tsx.childId({s}, \"{s}\", {s})", .{ parent, comp.name, k });
    g.child_sites += 1;
    return zig_text.fmt(g, ", tsx.siteId({s}, \"{s}\", {d})", .{ parent, comp.name, g.child_sites });
}

fn fieldList(g: *Generator, comp: *const Component) Error![]const u8 {
    var names: std.ArrayList([]const u8) = .empty;
    for (fields(g, comp)) |f| try names.append(g.a, f.name);
    return if (names.items.len == 0) "none" else std.mem.join(g.a, ", ", names.items);
}

/// JSX children go to a `children` prop: one element for `ReactNode`, a
/// list for `ReactNode[]` (elements, `{cond && <X/>}` and arrays such as
/// `{todos.map(...)}` mixed).
fn childrenValue(g: *Generator, c: *Context, j: *ast.Jsx, comp: *const Component) Error!?[]const u8 {
    const declared = if (comp.props) |p| fieldType(g, p, "children") else null;
    if (declared) |ty| if ((model.optionalChild(g, ty) orelse ty) == .string) return textChildren(g, c, j, comp);
    const Part = struct { code: []const u8, ty: Type };
    var parts: std.ArrayList(Part) = .empty;
    for (j.children) |child| switch (child) {
        .text => |t| return core.fail(g, t.loc, "text must be inside <Text>", .{}),
        .element => |el| {
            const e = try g.a.create(ast.Expr);
            e.* = .{ .loc = el.loc, .data = .{ .jsx = el } };
            try parts.append(g.a, .{ .code = (try expr.emit(g, c, e, .node, false)).code, .ty = .node });
        },
        .expr => |x| if (x) |e| {
            const v = try expr.emit(g, c, e, null, true);
            if (v.ty != .node and v.ty != .optional_node and model.element(g, v.ty) != .node) {
                return core.fail(g, e.loc, "children must be elements", .{});
            }
            try parts.append(g.a, .{ .code = v.code, .ty = v.ty });
        },
    };
    if (parts.items.len == 0) return null;
    const ty = (if (comp.props) |p| fieldType(g, p, "children") else null) orelse
        return core.fail(g, j.loc, "<{s}> has no `children` prop", .{comp.name});
    const base = model.optionalChild(g, ty) orelse ty;
    if (base == .node) {
        if (parts.items.len != 1 or parts.items[0].ty != .node) {
            return core.fail(g, j.loc, "<{s}> takes one child element", .{comp.name});
        }
        return parts.items[0].code;
    }
    if (model.element(g, base) != .node) return core.fail(
        g,
        j.loc,
        "`children` must be typed ReactNode or ReactNode[]",
        .{},
    );
    var codes: std.ArrayList([]const u8) = .empty;
    var arrays = false;
    for (parts.items) |part| arrays = arrays or (part.ty != .node and part.ty != .optional_node);
    for (parts.items) |part| try codes.append(g.a, if (!arrays) part.code else switch (part.ty) {
        .node => try zig_text.fmt(g, "&[_]ui.Node{{{s}}}", .{part.code}),
        .optional_node => try zig_text.fmt(g, "tsx.nodes(&.{{{s}}})", .{part.code}),
        else => part.code,
    });
    const joined = try std.mem.join(g.a, ", ", codes.items);
    return if (arrays)
        try zig_text.fmt(g, "tsx.concat(ui.Node, &.{{ {s} }})", .{joined})
    else
        try zig_text.fmt(g, "tsx.nodes(&.{{ {s} }})", .{joined});
}

/// `children: string`: `<Badge>new {count}</Badge>` passes its text.
fn textChildren(g: *Generator, c: *Context, j: *ast.Jsx, comp: *const Component) Error!?[]const u8 {
    var f = zig_text.Format{ .g = g };
    var any = false;
    for (j.children) |child| switch (child) {
        .text => |t| {
            try f.text(t.value);
            any = true;
        },
        .expr => |x| if (x) |e| {
            try f.expression(c, try @import("nesting.zig").textExpression(g, e));
            any = true;
        },
        .element => |el| return core.fail(
            g,
            el.loc,
            "<{s}> can't be inside <{s}>; its children are text (`children: string`)",
            .{ if (el.name.len == 0) "a fragment" else el.name, comp.name },
        ),
    };
    return if (any) try f.finish(false) else null;
}

fn propValue(g: *Generator, c: *Context, ty: Type, value: *ast.Expr) Error![]const u8 {
    if (callbackInfo(g, ty)) |entry| {
        if (value.data == .null_lit and model.optionalChild(g, ty) != null) return "null";
        return callbackValue(g, c, entry, value);
    }
    return (try expr.emit(g, c, value, ty, false)).code;
}

/// A function, an inline arrow or a callback prop of this component, bound
/// as the callback type of the target prop.
fn registered(g: *Generator, value: *const ast.Expr) ?usize {
    for (g.callback_events.items) |ev| if (ev.expr == value) return ev.method;
    return null;
}

fn callbackValue(g: *Generator, c: *Context, entry: *const model.Info, value: *ast.Expr) Error![]const u8 {
    var method_index: usize = undefined;
    switch (value.data) {
        .arrow => {
            for (g.callback_events.items) |ev| {
                if (ev.expr == value) {
                    method_index = ev.method;
                    break;
                }
            } else unreachable; // registered by `scan`
            try captures.prepare(g, c, method_index);
        },
        .ident => |n| if (registered(g, value)) |index| {
            method_index = index;
            try captures.prepare(g, c, method_index);
        } else {
            if (try readProp(g, n)) |prop| {
                // forwarding this component's own callback prop
                const same = callbackInfo(g, prop.ty);
                if (same == null or !std.mem.eql(Type, same.?.params, entry.params)) {
                    return core.fail(g, value.loc, "`{s}` has a different signature", .{n});
                }
                return prop.code;
            }
            method_index = scope.sourceMethod(g, n) orelse return core.fail(
                g,
                value.loc,
                "callback props take a component function, an inline arrow or a callback prop",
                .{},
            );
            const f = g.methods.items[method_index].func orelse return core.fail(
                g,
                value.loc,
                "pass an arrow for setters: (v) => {s}(v)",
                .{n},
            );
            if (f.params.len != entry.params.len) return core.fail(
                g,
                value.loc,
                "`{s}` must take {d} arguments",
                .{ n, entry.params.len },
            );
            for (f.params, entry.params) |p, t| {
                if (try events.parameterType(g, p, null) != t) return core.fail(
                    g,
                    p.name.loc,
                    "parameter type does not match the callback prop",
                    .{},
                );
            }
        },
        // `props.onPick` (or any value of the callback's type): forwarded as is
        else => {
            const v = try expr.emit(g, c, value, null, false);
            const same = callbackInfo(g, v.ty) orelse return core.fail(
                g,
                value.loc,
                "callback props take a component function, an inline arrow or a callback prop",
                .{},
            );
            if (!std.mem.eql(Type, same.params, entry.params)) {
                return core.fail(g, value.loc, "this callback has a different signature", .{});
            }
            return v.code;
        },
    }
    const m = g.methods.items[method_index];
    const receiver = if (m.captures.items.len > 0) try captures.value(g, m) else "self";
    const target = try zig_text.fmt(g, "{s}.{s}", .{ try zig_text.id(g, g.name), try zig_text.id(g, m.name) });
    if (actionKind(entry.params)) |kind| {
        return zig_text.fmt(g, "ui.{s}({s}, {s})", .{ events.actionBuilder(kind), receiver, target });
    }
    var list: std.ArrayList([]const u8) = .empty;
    for (entry.params) |t| try list.append(g.a, t.zig(g));
    return zig_text.fmt(
        g,
        "tsx.callback(fn ({s}) void, {s}, {s})",
        .{ try std.mem.join(g.a, ", ", list.items), receiver, target },
    );
}
