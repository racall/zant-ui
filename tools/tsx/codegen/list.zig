//! `<List>` (React Native `FlatList`) → `ui.list`, the virtual list: only the
//! rows on screen are built (docs/tsx.md, "长列表").
//!
//!     <List data={todos} renderItem={(t, i) => <Row …/>} keyExtractor={(t) => t.id} />
//!
//! becomes a hidden `_list1: ui.ListState` field, an item builder method that
//! renders `renderItem` for `cap._data[index]` (render locals it reads are
//! captured like an event handler's, the data slice as `_data`), an id method
//! from `keyExtractor`, and
//!
//!     _list1: { const _cap1 = tsx.capture(...); break :_list1 ui.list(&self._list1, .{ ... }); }
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const stmt = @import("stmt.zig");
const events = @import("events.zig");
const captures = @import("captures.zig");
const jsx = @import("jsx.zig");
const Fields = @import("jsx_fields.zig").Fields;
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = scope.Context;
const Type = types.Type;
const Value = types.Value;
const Error = core.Error;

/// One `<List>` of the component being generated.
pub const ListInfo = struct {
    jsx: *ast.Jsx,
    /// The hidden `ui.ListState` field and the block label (`_list1`).
    field: []const u8,
    item_method: usize,
    id_method: ?usize = null,
};

/// `ui.ListOptions` fields settable from TSX (camelCase); `data`,
/// `renderItem` and `keyExtractor` are handled here.
const option_props = [_]tags.Prop{
    .{ .name = "key", .field = "key", .kind = .string },
    .{ .name = "estimatedItemHeight", .field = "estimated_height", .kind = .number },
    .{ .name = "itemHeight", .field = "item_height", .kind = .number },
    .{ .name = "followTail", .field = "follow_tail", .kind = .boolean },
    .{ .name = "overdraw", .field = "overdraw", .kind = .number },
    .{ .name = "gap", .field = "gap", .kind = .number },
    .{ .name = "padding", .field = "padding", .kind = .edges, .sides = tags.Sides.all },
    .{ .name = "background", .field = "background", .kind = .color },
    .{ .name = "width", .field = "width", .kind = .number },
    .{ .name = "height", .field = "height", .kind = .number },
    .{ .name = "grow", .field = "grow", .kind = .number },
    .{ .name = "selectable", .field = "selectable", .kind = .boolean },
    .{ .name = "focusable", .field = "focusable", .kind = .boolean },
    .{ .name = "scrollbar", .field = "scrollbar", .kind = .boolean },
    .{ .name = "alignment", .field = "alignment", .kind = .enumeration, .values = &.{
        .{ .ts = "top", .zig = "top" },
        .{ .ts = "bottom", .zig = "bottom" },
    } },
};
const list_tag = tags.Tag{ .name = "List", .builder = "ui.list", .content = .none, .props = &option_props };

fn attr(j: *ast.Jsx, name: []const u8) ?ast.Attr {
    for (j.attrs) |a| if (zig_text.eq(a.name.name, name)) return a;
    return null;
}

fn arrow(g: *Generator, j: *ast.Jsx, name: []const u8, required: bool) Error!?*ast.Function {
    const a = attr(j, name) orelse {
        if (required) return core.fail(g, j.loc, "<List> needs `{s}`", .{name});
        return null;
    };
    const v = a.value orelse return core.fail(g, a.name.loc, "`{s}` needs a function", .{name});
    if (v.data != .arrow) return core.fail(g, v.loc, "`{s}` must be an arrow function: (item, index) => ...", .{name});
    return v.data.arrow;
}

/// Registers the item and id methods (before emission, like event handlers)
/// and scans their bodies for nested handlers.
pub fn scan(g: *Generator, j: *ast.Jsx) Error!void {
    if (attr(j, "data") == null) return core.fail(g, j.loc, "<List> needs `data`", .{});
    const render_item = (try arrow(g, j, "renderItem", true)).?;
    if (render_item.params.len == 0 or render_item.params.len > 2) {
        return core.fail(g, render_item.loc, "renderItem takes (item) or (item, index)", .{});
    }
    g.counter += 1;
    const n = g.counter;
    const item_index = g.methods.items.len;
    try g.methods.append(g.a, .{
        .name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_listItem{d}", .{n})),
        .func = render_item,
        .ret = .node,
        .capture_name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_ListCapture{d}", .{n})),
    });
    var info = ListInfo{
        .jsx = j,
        .field = try scope.uniqueMethod(g, try zig_text.fmt(g, "_list{d}", .{n})),
        .item_method = item_index,
    };
    try events.scanBody(g, render_item.body);
    if (try arrow(g, j, "keyExtractor", false)) |key_extractor| {
        if (key_extractor.params.len != 1) return core.fail(g, key_extractor.loc, "keyExtractor takes (item)", .{});
        info.id_method = g.methods.items.len;
        try g.methods.append(g.a, .{
            .name = try scope.uniqueMethod(g, try zig_text.fmt(g, "_listId{d}", .{n})),
            .func = key_extractor,
            .ret = .u64,
        });
    }
    for (j.attrs) |a| {
        const special = zig_text.eq(a.name.name, "renderItem") or zig_text.eq(a.name.name, "keyExtractor");
        if (!special) if (a.value) |v| try events.scanExpr(g, v);
    }
    try g.lists.append(g.a, info);
}

pub fn emit(g: *Generator, c: *Context, j: *ast.Jsx) Error!Value {
    const info = for (g.lists.items) |l| {
        if (l.jsx == j) break l;
    } else unreachable; // registered by `scan`
    const data_expr = attr(j, "data").?.value orelse return core.fail(g, j.loc, "`data` needs an array", .{});
    const data = try expr.emit(g, c, data_expr, null, false);
    const element = model.element(g, data.ty) orelse return core.fail(g, data_expr.loc, "`data` must be an array", .{});
    // the data slice travels in the capture struct as `_data`
    try g.methods.items[info.item_method].captures.append(g.a, .{
        .name = "\x00data",
        .field = "_data",
        .ty = data.ty,
        .source = data.code,
    });
    g.methods.items[info.item_method].emitted = try itemMethod(g, c, info, element);
    if (info.id_method) |id| g.methods.items[id].emitted = try idMethod(g, c, info, element);
    const cap = try zig_text.fmt(g, "_cap_{s}", .{info.field});
    var fields = Fields{ .g = g };
    for (j.attrs) |a| {
        const special = zig_text.eq(a.name.name, "data") or zig_text.eq(a.name.name, "renderItem") or
            zig_text.eq(a.name.name, "keyExtractor");
        if (!special) try jsx.attribute(g, c, &list_tag, &fields, a);
    }
    const struct_name = try zig_text.id(g, g.name);
    try fields.put("count", try zig_text.fmt(g, "{s}._data.len", .{cap}));
    try fields.put("item", try zig_text.fmt(g, "ui.listItem({s}, {s}.{s})", .{
        cap, struct_name, try zig_text.id(g, g.methods.items[info.item_method].name),
    }));
    if (info.id_method) |id| try fields.put("item_id", try zig_text.fmt(g, "ui.listItemId({s}, {s}.{s})", .{
        cap, struct_name, try zig_text.id(g, g.methods.items[id].name),
    }));
    const capture = try captures.value(g, g.methods.items[info.item_method]);
    return .{ .ty = .node, .code = try zig_text.fmt(
        g,
        "{s}: {{\nconst {s} = {s};\nbreak :{s} ui.list(&self.{s}, {s});\n}}",
        .{ info.field, cap, capture, info.field, try scope.stateField(g, info.field), try fields.emit() },
    ) };
}

/// Binds `renderItem`'s / `keyExtractor`'s params to `cap._data[index]` and
/// the index. Returns the method's index parameter name (allocated first, so
/// a user param called `index` gets renamed instead of shadowing it) and the
/// prologue declaring the params.
fn bindItem(g: *Generator, c: *Context, f: *ast.Function, element: Type) Error!struct { index: []const u8, prologue: []const u8 } {
    const index_param = (try scope.bind(c, "_index", .usize, false)).zig;
    var prologue: std.ArrayList(u8) = .empty;
    const item = f.params[0];
    if (item.ty) |ref| if (try types.typeRef(g, ref, false) != element) {
        return core.fail(g, item.name.loc, "the item type does not match `data`", .{});
    };
    const b = try scope.bind(c, item.name.name, element, false);
    try c.params.append(g.a, b);
    try prologue.appendSlice(g.a, try zig_text.fmt(g, "const {s}: {s} = cap._data[{s}];\n", .{
        b.zig, element.zig(g), index_param,
    }));
    if (f.params.len == 2) {
        const index = f.params[1];
        if (index.ty) |ref| if (try types.typeRef(g, ref, false) != .usize) {
            return core.fail(g, index.name.loc, "the index is a usize", .{});
        };
        const ib = try scope.bind(c, index.name.name, .usize, false);
        try c.params.append(g.a, ib);
        try prologue.appendSlice(g.a, try zig_text.fmt(g, "const {s}: usize = {s};\n", .{ ib.zig, index_param }));
    }
    return .{ .index = index_param, .prologue = prologue.items };
}

/// `fn _listItem1(cap: *_ListCapture1, index: usize) ui.Node`: renderItem in
/// render context (state reads, nested handlers capturing the item).
fn itemMethod(g: *Generator, source: *Context, info: ListInfo, element: Type) Error![]const u8 {
    const m = g.methods.items[info.item_method];
    const f = m.func.?;
    var c = Context{ .g = g, .render = true, .ret = .node, .capture_source = source, .capture_method = info.item_method };
    const bound = try bindItem(g, &c, f, element);
    const body = switch (f.body) {
        .expr => |e| try zig_text.fmt(g, "return {s};\n", .{(try expr.emit(g, &c, e, .node, false)).code}),
        .block => |ss| blk: {
            if (!stmt.returnsAll(ss)) return core.fail(g, f.loc, "renderItem must return an element on every path", .{});
            break :blk try stmt.statements(g, &c, ss, true);
        },
    };
    return zig_text.fmt(g, "fn {s}(cap: *{s}, {s}: usize) ui.Node {{\nconst self: *{s} = cap.self;\n{s}{s}}}\n", .{
        try zig_text.id(g, m.name), m.capture_name.?, bound.index,
        try zig_text.id(g, g.name), bound.prologue,   try scope.finish(&c, body),
    });
}

/// `fn _listId1(cap: *_ListCapture1, index: usize) u64` from keyExtractor:
/// integer keys as they are, floats truncated, strings hashed.
fn idMethod(g: *Generator, source: *Context, info: ListInfo, element: Type) Error![]const u8 {
    const m = g.methods.items[info.id_method.?];
    const f = m.func.?;
    var c = Context{ .g = g, .render = true, .ret = .u64, .capture_source = source, .capture_method = info.item_method };
    const bound = try bindItem(g, &c, f, element);
    const e = switch (f.body) {
        .expr => |x| x,
        .block => return core.fail(g, f.loc, "keyExtractor must be a single expression: (item) => item.id", .{}),
    };
    const v = try types.concrete(g, try expr.emit(g, &c, e, null, false));
    const id = if (v.ty == .string)
        try zig_text.fmt(g, "std.hash.Wyhash.hash(0, {s})", .{v.code})
    else if (v.ty.integer())
        try zig_text.fmt(g, "@intCast({s})", .{v.code})
    else if (v.ty.float())
        try zig_text.fmt(g, "@intFromFloat({s})", .{v.code})
    else
        return core.fail(g, e.loc, "keyExtractor must return a string or a number", .{});
    const capture_name = g.methods.items[info.item_method].capture_name.?;
    const body = try scope.finish(&c, try zig_text.fmt(g, "return {s};\n", .{id}));
    return zig_text.fmt(g, "fn {s}(cap: *{s}, {s}: usize) u64 {{\nconst self: *{s} = cap.self;\n{s}{s}}}\n", .{
        try zig_text.id(g, m.name), capture_name, bound.index, try zig_text.id(g, g.name), bound.prologue, body,
    });
}
