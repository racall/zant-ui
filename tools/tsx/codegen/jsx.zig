//! JSX builders, ordered prop merging, text content, fragments, and optional children.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const jsx = @This();
const events = @import("events.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Value = types.Value;
const Context = scope.Context;
const Format = zig_text.Format;
const Error = core.Error;
const Fields = @import("jsx_fields.zig").Fields;
const ChildList = @import("jsx_children.zig").ChildList;
const values = @import("jsx_values.zig");
const edges = @import("jsx_edges.zig");
const nesting = @import("nesting.zig");

pub fn validTags(g: *Generator) Error![]const u8 {
    var names: std.ArrayList([]const u8) = .empty;
    for (tags.all) |tag| try names.append(g.a, tag.name);
    return std.mem.join(g.a, ", ", names.items);
}

pub fn validProps(g: *Generator, tag: *const tags.Tag) Error![]const u8 {
    var names: std.ArrayList([]const u8) = .empty;
    for (tag.props) |p| try names.append(g.a, p.name);
    return std.mem.join(g.a, ", ", names.items);
}

pub fn enumValue(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr) Error![]const u8 {
    if (e.data == .cond and prop.kind != .flag) {
        const v = e.data.cond;
        const cond = try expr.condition(g, c, v.cond);
        return zig_text.fmt(
            g,
            "if ({s}) {s} else {s}",
            .{ cond.code, try jsx.enumValue(g, c, prop, v.then), try jsx.enumValue(g, c, prop, v.else_) },
        );
    }
    const spelling = switch (e.data) {
        .string => |s| s,
        .number => |s| if (prop.kind == .enumeration) s else return core.fail(
            g,
            e.loc,
            "`{s}` requires a static string literal",
            .{prop.name},
        ),
        else => return core.fail(
            g,
            e.loc,
            "`{s}` requires a literal{s}",
            .{ prop.name, if (prop.kind == .flag) " string" else " or a conditional of literals" },
        ),
    };
    const value = tags.findValue(prop, spelling) orelse {
        var names: std.ArrayList([]const u8) = .empty;
        for (prop.values) |v| try names.append(g.a, v.ts);
        return core.fail(
            g,
            e.loc,
            "invalid `{s}` value `{s}`; expected: {s}",
            .{ prop.name, spelling, try std.mem.join(g.a, ", ", names.items) },
        );
    };
    return if (prop.kind == .flag) value else zig_text.fmt(g, ".{s}", .{try zig_text.id(g, value)});
}

/// Merge props and literal style objects in source order; later writes replace earlier fields.
pub fn attribute(g: *Generator, c: *Context, tag: *const tags.Tag, fields: *Fields, attr: ast.Attr) Error!void {
    if (zig_text.eq(attr.name.name, "style")) {
        const e = attr.value orelse return core.fail(g, attr.name.loc, "style must be an object literal", .{});
        if (e.data != .object) return core.fail(g, e.loc, "style must be an object literal", .{});
        for (e.data.object) |p| {
            if (p.spread) return core.fail(g, p.value.loc, "object spread is not supported yet", .{});
            if (zig_text.eq(p.key.name, "style")) return core.fail(
                g,
                p.key.loc,
                "nested style is not supported; put the properties in the outer object",
                .{},
            );
            try jsx.attribute(g, c, tag, fields, .{ .name = p.key, .value = p.value });
        }
        return;
    }
    const prop = tags.findProp(tag, attr.name.name) orelse return core.fail(
        g,
        attr.name.loc,
        "unknown prop `{s}` on <{s}>; valid props: {s}",
        .{ attr.name.name, tag.name, try jsx.validProps(g, tag) },
    );
    try property(g, c, prop, fields, attr.value, attr.name.loc);
}

/// Reuse scalar, node, and collection conversions for tag props and struct items.
pub fn property(
    g: *Generator,
    c: *Context,
    prop: *const tags.Prop,
    root: *Fields,
    value: ?*ast.Expr,
    loc: ast.Loc,
) Error!void {
    const fields = try root.group(prop.parent);
    const e = value orelse {
        if (prop.kind != .boolean) return core.fail(
            g,
            loc,
            "bare attribute `{s}` is only valid for boolean props",
            .{prop.name},
        );
        try fields.put(prop.field, "true");
        return;
    };
    switch (prop.kind) {
        .number => try fields.put(prop.field, (try expr.emit(g, c, e, .f32, false)).code),
        .integer => try integerProp(g, c, prop, e, fields),
        .boolean => try fields.put(prop.field, (try expr.emit(g, c, e, .boolean, false)).code),
        .string => try fields.put(prop.field, try stringProp(g, c, prop, e)),
        .enumeration => try fields.put(prop.field, try jsx.enumValue(g, c, prop, e)),
        .color => try fields.put(prop.field, try zig_text.color(g, c, e)),
        .dimension => try dimensionProp(g, c, prop, e, fields),
        .edges => try edges.apply(g, c, prop, e, fields),
        .node, .optional_node => try fields.put(prop.field, try values.node(g, c, prop, e)),
        .children => try fields.put(prop.field, try values.children(g, c, prop, e)),
        .string_list => try fields.put(prop.field, try values.strings(g, c, prop, e)),
        .struct_list => try fields.put(prop.field, try values.structs(g, c, prop, e)),
        .flag => {
            const field = try jsx.enumValue(g, c, prop, e);
            if (field.len > 0) try fields.put(field, "true");
        },
        .flex => {
            try fields.put("grow", (try expr.emit(g, c, e, .f32, false)).code);
            try fields.put("shrink", "1");
            try fields.put("basis", "0");
        },
        .action, .bool_action, .text_action, .index_action => {
            const action = try events.eventValue(g, c, e, prop.kind);
            try fields.put(prop.field, action);
        },
    }
}

/// Require an integer expression and coerce it to the field integer type.
fn integerProp(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr, fields: *Fields) Error!void {
    const t = try types.typeRef(g, .{ .name = prop.int_type, .loc = e.loc }, false);
    try types.checkLiteral(g, e, t);
    const v = try expr.emit(g, c, e, if (types.numericLiterals(e) and e.data != .number) t else null, false);
    if (v.ty == .f64 or v.ty == .f32) {
        // TS numbers: truncated and clamped to the field's range
        try fields.put(prop.field, try zig_text.fmt(g, "tsx.toInt({s}, {s})", .{ t.zig(g), v.code }));
        return;
    }
    if (!v.ty.integer() and v.ty != .int_literal) {
        return core.fail(g, e.loc, "`{s}` requires an integer expression", .{prop.name});
    }
    try fields.put(prop.field, (try types.coerce(g, v, t, e.loc)).code);
}

/// Keep the last fixed or percent dimension and remove its opposite field.
fn dimensionProp(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr, fields: *Fields) Error!void {
    if (e.data == .string) {
        const s = e.data.string;
        if (s.len < 2 or s[s.len - 1] != '%' or prop.percent_field.len == 0) return core.fail(
            g,
            e.loc,
            "dimension must be a number or a percentage literal such as \"50%\"",
            .{},
        );
        const n = std.fmt.parseFloat(f32, s[0 .. s.len - 1]) catch {
            return core.fail(g, e.loc, "invalid percentage dimension", .{});
        };
        if (!std.math.isFinite(n)) return core.fail(g, e.loc, "percentage dimension must be finite", .{});
        fields.remove(prop.field);
        try fields.put(prop.percent_field, try zig_text.fmt(g, "{d}", .{n}));
    } else {
        fields.remove(prop.percent_field);
        try fields.put(prop.field, (try expr.emit(g, c, e, .f32, false)).code);
    }
}

/// Lower container children or joined text according to the tag table content contract.
pub fn emit(g: *Generator, c: *Context, j: *ast.Jsx) Error!Value {
    if (j.name.len == 0) return core.fail(g, j.loc, "a fragment is only allowed as a child; wrap it in <View>", .{});
    if (@import("components.zig").find(g, j.name)) |comp| return @import("components.zig").emitCall(g, c, j, comp);
    if (zig_text.eq(j.name, "List")) {
        try nesting.notInScroll(g, j);
        return @import("list.zig").emit(g, c, j);
    }
    const tag = tags.find(j.name) orelse return core.fail(
        g,
        j.loc,
        "unknown tag <{s}>; valid tags: {s}",
        .{ j.name, try jsx.validTags(g) },
    );
    if (tag.instance_id) {
        // widgets that keep state per instance (and scroll by themselves)
        if (zig_text.eq(j.name, "PdfViewer")) try nesting.notInScroll(g, j);
        return @import("widgets.zig").emit(g, c, j, tag);
    }
    if (zig_text.eq(j.name, "Text") and @import("rich_text.zig").hasElements(j)) {
        return @import("rich_text.zig").emit(g, c, j);
    }
    try nesting.uniqueKeys(g, j.children);
    var fields = Fields{ .g = g };
    var child_list = ChildList{ .g = g, .c = c };
    for (j.attrs) |attr| try jsx.attribute(g, c, tag, &fields, attr);
    var text_arg: []const u8 = "";
    const scrolls = nesting.scrolls(j);
    if (scrolls) g.scroll_depth += 1;
    defer if (scrolls) {
        g.scroll_depth -= 1;
    };
    switch (tag.content) {
        .children => try nodeChildren(g, j, tag, &fields, &child_list),
        .text_arg, .text_field => text_arg = try textContent(g, c, j, tag, &fields),
        .none => try noChildren(g, j, tag),
        .child => try singleChild(g, j, tag, &fields, &child_list),
    }
    const owner = try zig_text.fmt(g, "<{s}>", .{j.name});
    try values.required(g, tag.props, &fields, j.loc, owner, "prop");
    if (tag.content == .text_field) fields.contentFirst(tag.content_field);
    var node = try zig_text.fmt(g, "{s}({s}{s})", .{ tag.builder, text_arg, try fields.emit() });
    // overlays cover the window wherever they are written
    if (nesting.overlay(j.name)) node = try zig_text.fmt(g, "ui.portal({s})", .{node});
    return .{ .ty = .node, .code = try child_list.wrap(node) };
}

pub fn hasFragment(e: *ast.Expr) bool {
    return switch (e.data) {
        .jsx => |j| j.name.len == 0,
        .non_null => |inner| jsx.hasFragment(inner),
        .cond => |v| jsx.hasFragment(v.then) or jsx.hasFragment(v.else_),
        .binary => |b| jsx.hasFragment(b.lhs) or jsx.hasFragment(b.rhs),
        else => false,
    };
}

/// Build a children array and filter null entries only when necessary.
fn nodeChildren(
    g: *Generator,
    j: *ast.Jsx,
    tag: *const tags.Tag,
    fields: *Fields,
    child_list: *ChildList,
) Error!void {
    try child_list.append(j.children, null);
    if (child_list.values.items.len == 0) return;
    const prop = tags.findProp(tag, "children") orelse {
        return core.fail(g, j.loc, "tag table is missing <{s}> children metadata", .{tag.name});
    };
    const target = try fields.group(prop.parent);
    try target.put(prop.field, try values.childrenCode(g, child_list));
}

/// A single-node content field accepts either a prop or exactly one JSX child.
fn singleChild(
    g: *Generator,
    j: *ast.Jsx,
    tag: *const tags.Tag,
    fields: *Fields,
    child_list: *ChildList,
) Error!void {
    const prop = tags.findProp(tag, tag.content_field) orelse {
        return core.fail(g, j.loc, "tag table is missing <{s}> child metadata", .{tag.name});
    };
    var has_children = false;
    for (j.children) |child| {
        if (child != .expr or child.expr != null) has_children = true;
    }
    if (fields.hasProp(prop)) {
        if (has_children) return core.fail(
            g,
            j.loc,
            "<{s}> cannot have both `{s}` and child elements",
            .{ tag.name, tag.content_field },
        );
        return;
    }
    try child_list.append(j.children, null);
    if (child_list.values.items.len != 1) return core.fail(
        g,
        j.loc,
        "<{s}> requires exactly one child element or the `{s}` prop",
        .{ tag.name, tag.content_field },
    );
    const child = child_list.values.items[0];
    if (child.ty != .node) return core.fail(g, j.loc, "<{s}> requires a non-optional node child", .{tag.name});
    const target = try fields.group(prop.parent);
    try target.put(prop.field, child.code);
}

/// Join scalar text children and reject a competing text prop.
fn textContent(g: *Generator, c: *Context, j: *ast.Jsx, tag: *const tags.Tag, fields: *Fields) Error![]const u8 {
    var f = Format{ .g = g };
    var content = false;
    for (j.children, 0..) |ch, i| switch (ch) {
        .text => |t| {
            // the button spaces its icon from the label itself
            var value = t.value;
            if (i > 0 and isIcon(j.children[i - 1])) value = std.mem.trimStart(u8, value, " ");
            if (i + 1 < j.children.len and isIcon(j.children[i + 1])) value = std.mem.trimEnd(u8, value, " ");
            try f.text(value);
            content = true;
        },
        .expr => |x| if (x) |e| {
            try f.expression(c, try nesting.textExpression(g, e));
            content = true;
        },
        .element => |el| {
            // `<Button><Icon name="check" /> Save</Button>`
            if (!zig_text.eq(tag.name, "Button") or !zig_text.eq(el.name, "Icon")) return nesting.misplaced(g, tag, el);
            try buttonIcon(g, c, el, fields);
        },
    };
    if (tag.content == .text_arg) return zig_text.fmt(g, "{s}, ", .{try f.finish(false)}) else if (content) {
        if (fields.has(tag.content_field)) return core.fail(
            g,
            j.loc,
            "<{s}> cannot have both `{s}` and text children",
            .{ tag.name, tag.content_field },
        );
        try fields.put(tag.content_field, try f.finish(false));
    }
    return "";
}

/// Reject all nonempty children on tags whose content mode is none.
fn noChildren(g: *Generator, j: *ast.Jsx, tag: *const tags.Tag) Error!void {
    for (j.children) |ch| switch (ch) {
        .expr => |e| {
            if (e != null) return core.fail(g, e.?.loc, "<{s}> has no children", .{tag.name});
        },
        .text => |t| return core.fail(g, t.loc, "<{s}> has no children", .{tag.name}),
        .element => |el| return nesting.misplaced(g, tag, el),
    };
}

fn isIcon(child: ast.Child) bool {
    return child == .element and zig_text.eq(child.element.name, "Icon");
}

/// One `<Icon>` inside a `<Button>` sets its `icon` (and `iconSize`).
fn buttonIcon(g: *Generator, c: *Context, el: *const ast.Jsx, fields: *Fields) Error!void {
    if (fields.has("icon")) return core.fail(g, el.loc, "a <Button> has one icon", .{});
    const button = tags.find("Button").?;
    for (el.attrs) |a| {
        const target = if (zig_text.eq(a.name.name, "name"))
            "icon"
        else if (zig_text.eq(a.name.name, "size"))
            "iconSize"
        else
            return core.fail(g, a.name.loc, "an icon in a <Button> takes `name` and `size`", .{});
        try jsx.attribute(g, c, button, fields, .{ .name = .{ .name = target, .loc = a.name.loc }, .value = a.value });
    }
    if (!fields.has("icon")) return core.fail(g, el.loc, "<Icon> needs `name`", .{});
}

/// Numeric keys use the same formatting rules as text substitutions.
fn stringProp(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr) Error![]const u8 {
    if (!zig_text.eq(prop.name, "key")) return (try expr.emit(g, c, e, .string, false)).code;
    const value = try expr.emit(g, c, e, null, false);
    if (value.ty == .string) return value.code;
    if (@import("model.zig").enumType(g, value.ty)) return (try types.coerce(g, value, .string, e.loc)).code;
    if (@import("model.zig").optionalChild(g, value.ty) != null) {
        return core.fail(g, e.loc, "optional key needs ?? or !", .{});
    }
    if (!value.ty.numeric()) return core.fail(g, e.loc, "key must be a string or number", .{});
    var format = Format{ .g = g };
    try format.value(value, e.loc);
    return format.finish(false);
}
