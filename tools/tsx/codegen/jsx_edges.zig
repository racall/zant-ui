//! Merge numeric and object edges one side at a time, preserving attribute order.
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const expr = @import("expr.zig");
const zig_text = @import("zig_text.zig");
const Generator = core.Generator;
const Context = @import("scope.zig").Context;
const Fields = @import("jsx_fields.zig").Fields;
const Error = core.Error;

/// Object edge keys share the same later-wins behavior as paddingTop aliases.
pub fn apply(g: *Generator, c: *Context, prop: *const tags.Prop, e: *ast.Expr, fields: *Fields) Error!void {
    if (e.data != .object) return sides(g, c, fields, prop.field, prop.sides, e);
    for (e.data.object) |property| {
        if (property.spread) return core.fail(g, property.value.loc, "object spread is not supported yet", .{});
        const selected = edgeSides(property.key.name) orelse return core.fail(
            g,
            property.key.loc,
            "unknown edge `{s}`; valid keys: top, right, bottom, left, horizontal, vertical",
            .{property.key.name},
        );
        const intersection = tags.Sides{
            .top = selected.top and prop.sides.top,
            .right = selected.right and prop.sides.right,
            .bottom = selected.bottom and prop.sides.bottom,
            .left = selected.left and prop.sides.left,
        };
        try sides(g, c, fields, prop.field, intersection, property.value);
    }
    // An empty object still writes an Edges value whose sides default to zero.
    _ = try fields.entry(prop.field);
}

/// One source expression applied to every side can later become ui.inset(value).
fn sides(
    g: *Generator,
    c: *Context,
    fields: *Fields,
    field: []const u8,
    selected: tags.Sides,
    e: *ast.Expr,
) Error!void {
    const value = (try expr.emit(g, c, e, .f32, false)).code;
    const entry = try fields.entry(field);
    const mask = [_]bool{ selected.top, selected.right, selected.bottom, selected.left };
    for (mask, 0..) |set, index| {
        if (set) entry.edges[index] = .{ .code = value, .source = e };
    }
}

/// Accept the documented side names rather than arbitrary object fields.
fn edgeSides(name: []const u8) ?tags.Sides {
    const names = .{
        .{ "top", tags.Sides{ .top = true } },
        .{ "right", tags.Sides{ .right = true } },
        .{ "bottom", tags.Sides{ .bottom = true } },
        .{ "left", tags.Sides{ .left = true } },
        .{ "horizontal", tags.Sides.horizontal },
        .{ "vertical", tags.Sides.vertical },
    };
    inline for (names) |pair| if (zig_text.eq(name, pair[0])) return pair[1];
    return null;
}
