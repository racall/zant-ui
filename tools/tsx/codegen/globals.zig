//! File-level constants and type declarations, resolved before component state.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const expr = @import("expr.zig");
const scope = @import("scope.zig");
const text = @import("zig_text.zig");

pub fn collect(g: *core.Generator, constants: []const ast.TopConst) core.Error![]const u8 {
    var code: std.ArrayList(u8) = .empty;
    var c = scope.Context{ .g = g, .render = true, .ret = .void_, .constant = true };
    for (constants) |constant| {
        if (scope.globalName(g, constant.name.name)) {
            return core.fail(g, constant.name.loc, "duplicate or reserved constant `{s}`", .{constant.name.name});
        }
        const expected = if (constant.ty) |ty| try types.typeRef(g, ty, false) else null;
        var value = try expr.emit(g, &c, constant.value, expected, false);
        value = try types.coerce(g, value, expected orelse value.ty.concrete(), constant.value.loc);
        const name = try text.id(g, constant.name.name);
        try code.appendSlice(g.a, try text.fmt(g, "{s}const {s}: {s} = {s};\n\n", .{
            if (constant.exported) "pub " else "", name, value.ty.zig(g), value.code,
        }));
        value.code = name;
        value.inference = .known;
        value.text = null;
        value.formatted = null;
        try g.constants.append(g.a, .{ .name = constant.name.name, .value = value });
    }
    return code.items;
}

/// `import icon from "../assets/icon.png"` → `const icon: []const u8 =
/// @embedFile("_assets/…-icon.png");` (zant-tsx copies the file there).
pub fn assets(g: *core.Generator, module: ast.Module, source_path: ?[]const u8) core.Error![]const u8 {
    const files = @import("../assets.zig");
    var code: std.ArrayList(u8) = .empty;
    for (module.imports) |imp| {
        if (!files.isAsset(imp.source)) continue;
        const name = imp.default_name.?; // the parser requires it
        const importer = source_path orelse return core.fail(
            g,
            imp.source_loc,
            "asset imports need a project build (addTsxApp / addTsxProject, or zant-tsx --out)",
            .{},
        );
        if (scope.globalName(g, name.name)) {
            return core.fail(g, name.loc, "duplicate or reserved name `{s}`", .{name.name});
        }
        const id = try text.id(g, name.name);
        try code.appendSlice(g.a, try text.fmt(g, "const {s}: []const u8 = @embedFile({s});\n\n", .{
            id, try text.quote(g, try files.key(g.a, importer, imp.source)),
        }));
        try g.constants.append(g.a, .{ .name = name.name, .value = .{
            .ty = try @import("model.zig").array(g, .u8),
            .code = id,
            .inference = .known,
        } });
    }
    return code.items;
}

/// Anonymous nested types have stable file-level names too.
pub fn declarations(g: *core.Generator) core.Error![]const u8 {
    var code: std.ArrayList(u8) = .empty;
    for (g.compounds.items) |entry| if (entry.declaration) |declaration| {
        try code.appendSlice(g.a, declaration);
    };
    return code.items;
}
