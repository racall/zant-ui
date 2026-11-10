//! Node and literal-list props, recursive item schemas, and required-field checks.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const expr = @import("expr.zig");
const jsx = @import("jsx.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = @import("scope.zig").Context;
const Fields = @import("jsx_fields.zig").Fields;
const ChildList = @import("jsx_children.zig").ChildList;
const Error = core.Error;

/// Optional nodes accept null; required nodes always retain a concrete ui.Node.
pub fn node(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr) Error![]const u8 {
    const value = try expr.emit(g, c, e, null, false);
    const optional = prop.kind == .optional_node;
    if (value.ty == .node) return value.code;
    if (optional and (value.ty == .optional_node or value.ty == .null_)) return value.code;
    return core.fail(
        g,
        e.loc,
        "prop `{s}` requires {s}; wrap text in <Text>",
        .{ prop.name, if (optional) "an element or null" else "an element" },
    );
}

/// Arrays remain restricted to prop positions until general arrays are implemented.
fn array(g: *Generator, prop: *const tags.Prop, e: *ast.Expr) Error![]const *ast.Expr {
    if (e.data == .array) return e.data.array;
    return core.fail(g, e.loc, "prop `{s}` requires an array literal", .{prop.name});
}

/// Reuse child flattening and optional filtering for an explicit children array.
pub fn children(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr) Error![]const u8 {
    var list = ChildList{ .g = g, .c = c };
    for (try array(g, prop, e)) |item| try list.expression(item, null);
    return list.wrap(try childrenCode(g, &list));
}

/// Keep ordinary child arrays allocation-free; optional entries use tsx.nodes.
pub fn childrenCode(g: *Generator, list: *ChildList) Error![]const u8 {
    if (list.values.items.len == 0) return "&.{}";
    var codes: std.ArrayList([]const u8) = .empty;
    var optional = false;
    for (list.values.items) |value| {
        try codes.append(g.a, value.code);
        optional = optional or value.ty == .optional_node or value.ty == .null_;
    }
    const code = try zig_text.fmt(g, "&.{{\n{s},\n}}", .{try std.mem.join(g.a, ",\n", codes.items)});
    return if (optional) zig_text.fmt(g, "tsx.nodes({s})", .{code}) else code;
}

/// A string-list field gives each expression its []const u8 result location.
pub fn strings(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr) Error![]const u8 {
    var codes: std.ArrayList([]const u8) = .empty;
    for (try array(g, prop, e)) |item| {
        try codes.append(g.a, (try expr.emit(g, c, item, .string, false)).code);
    }
    if (codes.items.len == 0) return "&.{}";
    return zig_text.fmt(g, "&.{{ {s} }}", .{try std.mem.join(g.a, ", ", codes.items)});
}

/// Typed item arrays also work when empty, without depending on Zig peer typing.
pub fn structs(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr) Error![]const u8 {
    var codes: std.ArrayList([]const u8) = .empty;
    for (try array(g, prop, e), 0..) |item, index| {
        const owner = try zig_text.fmt(g, "{s}[{d}]", .{ prop.name, index });
        try codes.append(g.a, try structItem(g, c, prop, item, owner));
    }
    const body = if (codes.items.len == 0)
        ""
    else
        try zig_text.fmt(g, "\n{s},\n", .{try std.mem.join(g.a, ",\n", codes.items)});
    return zig_text.fmt(g, "&[_]{s}{{{s}}}", .{ prop.item_type, body });
}

/// Item keys use the same prop conversions recursively, with index/key diagnostics.
fn structItem(
    g: *Generator,
    c: *Context,
    prop: *const tags.Prop,
    item: *ast.Expr,
    owner: []const u8,
) Error![]const u8 {
    if (item.data == .spread) return core.fail(g, item.loc, "{s}: array spread is not supported yet", .{owner});
    if (item.data != .object) return core.fail(g, item.loc, "{s} must be an object literal", .{owner});
    var fields = Fields{ .g = g };
    const schema = tags.Tag{ .name = owner, .builder = "", .content = .none, .props = prop.item_props };
    for (item.data.object) |property| {
        if (property.spread) return core.fail(
            g,
            property.value.loc,
            "{s}: object spread is not supported yet",
            .{owner},
        );
        const field = tags.findProp(&schema, property.key.name) orelse return core.fail(
            g,
            property.key.loc,
            "{s}: unknown key `{s}`; valid keys: {s}",
            .{ owner, property.key.name, try jsx.validProps(g, &schema) },
        );
        jsx.property(g, c, field, &fields, property.value, property.key.loc) catch |err| {
            if (err == error.Tsx) try itemDiagnostic(g, owner, property.key.name);
            return err;
        };
    }
    try required(g, prop.item_props, &fields, item.loc, owner, "key");
    return fields.emit();
}

/// Preserve the original source location while adding the enclosing item path.
fn itemDiagnostic(g: *Generator, owner: []const u8, key: []const u8) Error!void {
    try core.contextualize(g, try zig_text.fmt(g, "in {s}.{s}", .{ owner, key }));
}

/// Check emitted field paths so aliases and JSX content satisfy required props too.
pub fn required(
    g: *Generator,
    props: []const tags.Prop,
    fields: *Fields,
    loc: ast.Loc,
    owner: []const u8,
    noun: []const u8,
) Error!void {
    for (props) |*prop| {
        if (prop.required and !fields.hasProp(prop)) {
            return core.fail(g, loc, "{s} is missing required {s} `{s}`", .{ owner, noun, prop.name });
        }
    }
}
