//! Id-taking reflected builders: scalar props use the ordinary JSX pipeline;
//! only instance identity and Markdown's text children need special handling.
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const scope = @import("scope.zig");
const types = @import("types.zig");
const expr = @import("expr.zig");
const jsx = @import("jsx.zig");
const zig_text = @import("zig_text.zig");
const Fields = @import("jsx_fields.zig").Fields;
const values = @import("jsx_values.zig");

/// The reflected signature, rather than a second list of widget names,
/// determines which tags require per-instance state.
pub fn needsId(name: []const u8) bool {
    const tag = tags.find(name) orelse return false;
    return tag.instance_id;
}

/// Emits a stable instance id and flattened options. Event scanning already
/// uses the reflected props, so actions require no widget-specific scanner.
pub fn emit(g: *core.Generator, c: *scope.Context, j: *ast.Jsx, tag: *const tags.Tag) core.Error!types.Value {
    var fields = Fields{ .g = g };
    var raw_key: ?[]const u8 = null;
    for (try @import("spread.zig").flatten(g, c, j.attrs)) |attr| {
        try jsx.attribute(g, c, tag, &fields, attr);
        if (zig_text.eq(attr.name.name, "key")) raw_key = try keyValue(g, c, attr.value, attr.name.loc);
        if (zig_text.eq(attr.name.name, "style")) {
            // attribute() has already checked that this is a literal object.
            for (attr.value.?.data.object) |p| {
                if (zig_text.eq(p.key.name, "key")) raw_key = try keyValue(g, c, p.value, p.key.loc);
            }
        }
    }
    if (zig_text.eq(tag.name, "Markdown")) {
        try markdownChildren(g, c, j, &fields);
    } else {
        for (j.children) |child| switch (child) {
            .expr => |e| if (e) |v| return core.fail(g, v.loc, "<{s}> has no children", .{tag.name}),
            .text => |t| return core.fail(g, t.loc, "<{s}> has no children", .{tag.name}),
            .element => |el| return core.fail(g, el.loc, "<{s}> has no children", .{tag.name}),
        };
    }
    const owner = try zig_text.fmt(g, "<{s}>", .{tag.name});
    try values.required(g, tag.props, &fields, j.loc, owner, "prop");
    if (raw_key == null and g.loop_depth > 0) return core.fail(
        g,
        j.loc,
        "<{s}> keeps its own state; give each one in a list a key (key={{item.id}}) so the state follows the item",
        .{tag.name},
    );
    const parent = if (g.current_component != null) "self._id" else "0";
    const id = if (raw_key) |key|
        try zig_text.fmt(g, "tsx.childId({s}, \"{s}\", {s})", .{ parent, tag.name, key })
    else blk: {
        g.child_sites += 1;
        break :blk try zig_text.fmt(g, "tsx.siteId({s}, \"{s}\", {d})", .{ parent, tag.name, g.child_sites });
    };
    return .{ .ty = .node, .code = try zig_text.fmt(g, "{s}({s}, {s})", .{ tag.builder, id, try fields.emit() }) };
}

fn keyValue(g: *core.Generator, c: *scope.Context, e: ?*ast.Expr, loc: ast.Loc) core.Error![]const u8 {
    const value = e orelse return core.fail(g, loc, "key needs a value", .{});
    const k = try expr.emit(g, c, value, null, false);
    if (k.ty == .string) return k.code;
    if (k.ty.numeric()) return (try types.concrete(g, k)).code;
    return core.fail(g, value.loc, "key must be a string or a number", .{});
}

fn markdownChildren(g: *core.Generator, c: *scope.Context, j: *ast.Jsx, fields: *Fields) core.Error!void {
    const message = "Markdown takes its text as `source` or as its children";
    var text = zig_text.Format{ .g = g };
    var has_text = false;
    for (j.children) |child| switch (child) {
        .text => |t| {
            try text.text(t.value);
            has_text = true;
        },
        .expr => |e| if (e) |v| {
            const value = try expr.emit(g, c, try @import("nesting.zig").textExpression(g, v), null, false);
            if (value.ty != .string) return core.fail(g, v.loc, message, .{});
            try text.value(value, v.loc);
            has_text = true;
        },
        .element => |el| return core.fail(g, el.loc, message, .{}),
    };
    if (has_text) {
        if (fields.has("source")) return core.fail(
            g,
            j.loc,
            "<Markdown> cannot have both `source` and text children",
            .{},
        );
        try fields.put("source", try text.finish(false));
    }
}
