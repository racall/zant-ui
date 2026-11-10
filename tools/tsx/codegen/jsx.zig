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
    const e = attr.value orelse {
        if (prop.kind != .boolean) return core.fail(
            g,
            attr.name.loc,
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
        .string => try fields.put(prop.field, (try expr.emit(g, c, e, .string, false)).code),
        .enumeration => try fields.put(prop.field, try jsx.enumValue(g, c, prop, e)),
        .color => try fields.put(prop.field, try zig_text.color(g, c, e)),
        .dimension => try dimensionProp(g, c, prop, e, fields),
        .edges => try edgesProp(g, c, prop, e, fields),
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
            const action = try events.eventValue(g, e, prop.kind);
            try fields.put(prop.field, action);
        },
    }
}

/// Require an integer expression and coerce it to the field integer type.
fn integerProp(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr, fields: *Fields) Error!void {
    const t = try types.typeRef(g, .{ .name = prop.int_type, .loc = e.loc }, false);
    try types.checkLiteral(g, e, t);
    const v = try expr.emit(g, c, e, if (types.numericLiterals(e) and e.data != .number) t else null, false);
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

/// Apply an edges prop only to the sides selected by its tag-table entry.
fn edgesProp(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr, fields: *Fields) Error!void {
    const v = (try expr.emit(g, c, e, .f32, false)).code;
    const sides = [_]bool{ prop.sides.top, prop.sides.right, prop.sides.bottom, prop.sides.left };
    const entry = try fields.entry(prop.field);
    for (sides, 0..) |set_, i| if (set_) {
        entry.edges[i] = .{ .code = v, .source = e };
    };
}

/// Lower container children or joined text according to the tag table content contract.
pub fn emit(g: *Generator, c: *Context, j: *ast.Jsx) Error!Value {
    if (j.name.len == 0) return core.fail(g, j.loc, "a fragment is only allowed as a child; wrap it in <View>", .{});
    const tag = tags.find(j.name) orelse return core.fail(
        g,
        j.loc,
        "unknown tag <{s}>; valid tags: {s}",
        .{ j.name, try jsx.validTags(g) },
    );
    var fields = Fields{ .g = g };
    var child_list = ChildList{ .g = g, .c = c };
    for (j.attrs) |attr| try jsx.attribute(g, c, tag, &fields, attr);
    var text_arg: []const u8 = "";
    switch (tag.content) {
        .children => try nodeChildren(g, j, &fields, &child_list),
        .text_arg, .text_field => text_arg = try textContent(g, c, j, tag, &fields),
        .none => try noChildren(g, j, tag),
    }
    if (tag.content == .text_field) fields.contentFirst(tag.content_field);
    const node = try zig_text.fmt(g, "{s}({s}{s})", .{ tag.builder, text_arg, try fields.emit() });
    return .{ .ty = .node, .code = try child_list.wrap(node) };
}

pub fn hasFragment(e: *ast.Expr) bool {
    return switch (e.data) {
        .jsx => |j| j.name.len == 0,
        .cond => |v| jsx.hasFragment(v.then) or jsx.hasFragment(v.else_),
        .binary => |b| jsx.hasFragment(b.lhs) or jsx.hasFragment(b.rhs),
        else => false,
    };
}

/// Build a children array and filter null entries only when necessary.
fn nodeChildren(g: *Generator, j: *ast.Jsx, fields: *Fields, child_list: *ChildList) Error!void {
    try child_list.append(j.children, null);
    if (child_list.values.items.len > 0) {
        var codes: std.ArrayList([]const u8) = .empty;
        var optional = false;
        for (child_list.values.items) |v| {
            try codes.append(g.a, v.code);
            optional = optional or v.ty == .optional_node or v.ty == .null_;
        }
        const code = try zig_text.fmt(g, "&.{{\n{s},\n}}", .{try std.mem.join(g.a, ",\n", codes.items)});
        try fields.put("children", if (optional) try zig_text.fmt(g, "tsx.nodes({s})", .{code}) else code);
    }
}

/// Join scalar text children and reject a competing text prop.
fn textContent(g: *Generator, c: *Context, j: *ast.Jsx, tag: *const tags.Tag, fields: *Fields) Error![]const u8 {
    var f = Format{ .g = g };
    var content = false;
    for (j.children) |ch| switch (ch) {
        .text => |t| {
            try f.text(t.value);
            content = true;
        },
        .expr => |x| if (x) |e| {
            try f.expression(c, e);
            content = true;
        },
        .element => |el| return core.fail(g, el.loc, "text content cannot contain elements", .{}),
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
            if (e != null) return core.fail(g, e.?.loc, "<{s}> does not accept children", .{tag.name});
        },
        .text => |t| return core.fail(g, t.loc, "<{s}> does not accept children", .{tag.name}),
        .element => |el| return core.fail(g, el.loc, "<{s}> does not accept children", .{tag.name}),
    };
}
