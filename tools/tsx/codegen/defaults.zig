//! Default values (docs/tsx.md, "参数默认值"):
//!
//!     function label(n: number, unit = "item"): string { … }   // label(2)
//!     function Badge({ text, size = 14 }: BadgeProps) { … }      // size?: number
//!
//! A parameter's default is used where a call leaves the argument out; a
//! prop's default where the parent leaves the optional prop unset. Defaults
//! are constant expressions (literals, file constants), evaluated where used.
const std = @import("std");
const ast = @import("../ast.zig");
const core = @import("codegen.zig");
const types = @import("types.zig");
const model = @import("model.zig");
const scope = @import("scope.zig");
const expr = @import("expr.zig");
const text = @import("zig_text.zig");
const Generator = core.Generator;
const Value = types.Value;
const Type = types.Type;
const Error = core.Error;

/// The default as a value of `ty`.
pub fn value(g: *Generator, e: *ast.Expr, ty: Type) Error!Value {
    var c = scope.Context{ .g = g, .render = true, .ret = .void_, .constant = true };
    const v = try expr.emit(g, &c, e, ty, false);
    return types.coerce(g, v, ty, e.loc);
}

/// A parameter's type: its annotation, else its default's (`unit = "item"`,
/// `by = step`; a number literal is a TS number, f64).
pub fn paramType(g: *Generator, p: ast.Param, owner: []const u8) Error!?Type {
    if (p.ty) |ref| return try types.typeRef(g, ref, false);
    const d = p.default orelse return null;
    var c = scope.Context{ .g = g, .render = true, .ret = .void_, .constant = true };
    const v = try expr.emit(g, &c, d, null, false);
    if (v.ty == .null_ or v.ty == .void_) {
        return core.fail(g, d.loc, "parameter `{s}` of `{s}` needs a type for this default", .{ p.name.name, owner });
    }
    return v.ty.concrete();
}

/// Required arguments: those before the first optional or defaulted one.
pub fn required(params: []const ast.Param) usize {
    for (params, 0..) |p, i| if (p.optional or p.default != null) return i;
    return params.len;
}

/// The argument code for a parameter the call left out.
pub fn missing(g: *Generator, p: ast.Param, ty: Type) Error![]const u8 {
    const d = p.default orelse return "null"; // an optional parameter
    return (try value(g, d, ty)).code;
}

/// `{ size = 14 }`: reading `size` gives `props.size orelse 14`.
pub fn prop(g: *Generator, name: ast.Name, d: *ast.Expr, ty: Type, code: []const u8) Error!Value {
    const child = model.optionalChild(g, ty) orelse return core.fail(
        g,
        d.loc,
        "`{s}` is a required prop; a default applies to optional ones (`{s}?: …`)",
        .{ name.name, name.name },
    );
    const fallback = try value(g, d, child);
    return .{
        .ty = child,
        .code = try text.fmt(g, "({s} orelse {s})", .{ code, fallback.code }),
    };
}
