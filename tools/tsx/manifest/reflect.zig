//! Compile-time reflection rules. This module emits source, so the standalone
//! generator has no dependency on the table that it regenerates.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const names = @import("names.zig");

const Props = struct {
    source: []const u8 = "",
    omitted: []const u8 = "",
    invalid: bool = false,
    children: bool = false,
    child_count: usize = 0,
    child_field: []const u8 = "",
};

fn quote(comptime value: []const u8) []const u8 {
    return std.fmt.comptimePrint("\"{f}\"", .{std.zig.fmtString(value)});
}

fn expression(comptime name: []const u8) []const u8 {
    return std.fmt.comptimePrint("ui.{f}", .{std.zig.fmtId(name)});
}

fn unwrap(comptime T: type) type {
    return if (@typeInfo(T) == .optional) @typeInfo(T).optional.child else T;
}

/// Only public ui struct declarations can name object-array element types.
fn publicStruct(comptime T: type) ?[]const u8 {
    if (@typeInfo(T) != .@"struct") return null;
    for (@typeInfo(ui).@"struct".decl_names) |name| {
        if (@TypeOf(@field(ui, name)) == type) {
            if (@field(ui, name) == T) return name;
        }
    }
    return null;
}

/// Public ui.*Options structs are flattenable; callback structs and image
/// handles are not options, and optional structs never enter this branch.
fn isOptions(comptime T: type) bool {
    const name = publicStruct(T) orelse return false;
    return std.mem.endsWith(u8, name, "Options");
}

/// Classify exact supported types before inspecting their underlying shape.
/// Optional scalars retain their kind. Every `u32` in the ui options is an
/// ARGB color (`?u32` mostly, `ModalOptions.mask` plain); the color kind
/// also takes plain number expressions, so a future integer `u32` still works.
fn kind(comptime T: type) ?[]const u8 {
    if (T == ?u32 or T == u32) return "color";
    if (T == ?zant.actions.Action) return "action";
    if (T == ?zant.actions.BoolAction) return "bool_action";
    if (T == ?zant.actions.TextAction) return "text_action";
    if (T == ?zant.actions.IndexAction) return "index_action";
    const U = unwrap(T);
    if (U == ui.Node) return if (T == ?ui.Node) "optional_node" else "node";
    if (U == f32) return "number";
    if (@typeInfo(U) == .int) return "integer";
    if (U == bool) return "boolean";
    if (U == []const u8) return "string";
    if (@typeInfo(U) == .@"enum") return "enumeration";
    if (U == zant.geometry.Edges) return "edges";
    if (U == []const ui.Node) return "children";
    if (U == []const []const u8) return "string_list";
    if (@typeInfo(U) == .pointer) {
        const p = @typeInfo(U).pointer;
        if (p.size == .slice and p.attrs.@"const" and p.sentinel_ptr == null and publicStruct(p.child) != null) return "struct_list";
    }
    return null;
}

fn unsupported(comptime T: type) []const u8 {
    return "unsupported type " ++ @typeName(T);
}

fn omission(comptime path: []const u8, comptime reason: []const u8) []const u8 {
    return "// " ++ path ++ ": " ++ reason ++ "\n";
}

/// Leaf fields inherit defaults, not requiredness from their enclosing box.
/// A nested options struct is flattened, with top-level names taking priority.
/// An unsupported required leaf makes its containing tag / array unusable.
fn props(comptime T: type, comptime path: []const u8, comptime parent: []const u8, comptime Top: type) Props {
    var result: Props = .{};
    const info = @typeInfo(T).@"struct";
    for (info.field_names, info.field_types, info.field_attrs) |field, F, attr| {
        const prop_name = names.camel(field);
        const field_path = path ++ "." ++ field;
        if (parent.len > 0) {
            var clash = false;
            for (@typeInfo(Top).@"struct".field_names) |top_field| {
                if (std.mem.eql(u8, names.camel(top_field), prop_name)) clash = true;
            }
            if (clash) {
                result.omitted = result.omitted ++ omission(field_path, "flattened name conflicts with a top-level prop");
                continue;
            }
        }
        const required = attr.default_value_ptr == null and @typeInfo(F) != .optional;
        if (kind(F)) |k| {
            var extra: []const u8 = "";
            const U = unwrap(F);
            if (std.mem.eql(u8, k, "integer")) extra = ", .int_type = " ++ quote(@typeName(U));
            if (std.mem.eql(u8, k, "edges")) extra = ", .sides = Sides.all";
            if (std.mem.eql(u8, k, "enumeration")) {
                extra = ", .values = &.{\n";
                for (@typeInfo(U).@"enum".field_names) |value| {
                    for (names.spellings(value)) |spelling| {
                        extra = extra ++ ".{ .ts = " ++ quote(spelling) ++ ", .zig = " ++ quote(value) ++ " },\n";
                    }
                }
                extra = extra ++ "}";
            }
            if (std.mem.eql(u8, k, "struct_list")) {
                const Item = @typeInfo(U).pointer.child;
                const item = props(Item, field_path ++ "[]", "", Item);
                result.omitted = result.omitted ++ item.omitted;
                if (item.invalid) {
                    result.omitted = result.omitted ++ omission(field_path, "array item has unsupported required fields");
                    result.invalid = result.invalid or required;
                    continue;
                }
                extra = ", .item_type = " ++ quote(expression(publicStruct(Item).?)) ++ ", .item_props = &.{\n" ++ item.source ++ "}";
            }
            if (parent.len > 0) extra = extra ++ ", .parent = " ++ quote(parent);
            result.source = result.source ++ ".{ .name = " ++ quote(prop_name) ++ ", .field = " ++ quote(field) ++
                ", .kind = ." ++ k ++ ", .required = " ++ (if (required) "true" else "false") ++ extra ++ " },\n";
            if (F == []const ui.Node and std.mem.eql(u8, field, "children")) result.children = true;
            if (F == ui.Node and required) {
                result.child_count += 1;
                result.child_field = field;
            }
        } else if (isOptions(F) and parent.len == 0) {
            const nested = props(F, field_path, field, T);
            result.source = result.source ++ nested.source;
            result.omitted = result.omitted ++ nested.omitted;
            result.invalid = result.invalid or nested.invalid;
            result.children = result.children or nested.children;
            result.child_count += nested.child_count;
            if (nested.child_count > 0) result.child_field = nested.child_field;
        } else {
            result.omitted = result.omitted ++ omission(field_path, unsupported(F) ++ (if (required) " (required)" else ""));
            result.invalid = result.invalid or required;
        }
    }
    return result;
}

/// Explicit adapters / runtime-only builders are excluded even if a future
/// signature happens to match the ordinary options builder shape.
fn skipReason(comptime name: []const u8) ?[]const u8 {
    if (std.mem.eql(u8, name, "windowControls")) return "runtime-only window controls";
    if (std.mem.eql(u8, name, "list")) return "List uses *ListState and a compiler special case";
    for ([_][]const u8{ "styled", "withKey", "appendChildren", "textFmt", "memo" }) |skip| {
        if (std.mem.eql(u8, name, skip)) return "adapter / generic helper, not an options builder";
    }
    return null;
}

/// Discover public Node-returning functions in declaration order. No builder or
/// options map is maintained here; parameter types are the source of truth.
pub fn source() []const u8 {
    @setEvalBranchQuota(4_000_000);
    var table: []const u8 = "";
    var omitted: []const u8 = "";
    var skipped: []const u8 = "";
    for (@typeInfo(ui).@"struct".decl_names) |name| {
        const F = @TypeOf(@field(ui, name));
        if (@typeInfo(F) != .@"fn") continue;
        const f = @typeInfo(F).@"fn";
        if (f.return_type != ui.Node) continue;
        var reason = skipReason(name);
        var Options: ?type = null;
        const text_arg = f.param_types.len == 2 and f.param_types[0] == []const u8;
        if (reason == null and !f.attrs.varargs and !f.is_generic) {
            if (f.param_types.len == 1 or text_arg) {
                if (f.param_types[f.param_types.len - 1]) |T| {
                    if (@typeInfo(T) == .@"struct") Options = T;
                }
            }
        }
        if (reason == null and Options == null) reason = "unsupported builder signature";
        if (Options) |T| {
            const fields = props(T, expression(name), "", T);
            omitted = omitted ++ fields.omitted;
            if (fields.invalid) {
                reason = "options have unsupported required fields (see omitted props)";
            } else {
                const content = if (text_arg) "text_arg" else if (fields.children) "children" else if (fields.child_count == 1) "child" else "none";
                table = table ++ ".{ .name = " ++ quote(names.pascal(name)) ++ ", .builder = " ++ quote(expression(name)) ++
                    ", .content = ." ++ content ++ (if (std.mem.eql(u8, content, "child")) ", .content_field = " ++ quote(fields.child_field) else "") ++
                    ", .props = &.{\n" ++ fields.source ++ "} },\n";
            }
        }
        if (reason) |why| {
            skipped = skipped ++ ".{ .builder = " ++ quote(expression(name)) ++ ", .reason = " ++ quote(why) ++ " },\n";
            omitted = omitted ++ omission(expression(name), "tag omitted: " ++ why);
        }
    }
    return "//! Generated by `zig build tsx-tags` from src/ui.zig; do not edit.\n" ++
        "const tags = @import(\"tags.zig\");\nconst Tag = tags.Tag;\nconst Sides = tags.Sides;\n\n" ++
        "pub const all: []const Tag = &.{\n" ++ table ++ "};\n\n" ++
        "/// Builders intentionally excluded from the generated table.\n" ++
        "pub const skipped = [_]struct { builder: []const u8, reason: []const u8 }{\n" ++ skipped ++ "};\n\n" ++
        "/// Fields of `zant.tokens.Theme`, read in TSX through `useTheme()`.\n" ++
        "pub const theme: []const tags.Prop = &.{\n" ++ themeSource() ++ "};\n\n" ++
        "// Omitted props and tags (in declaration / field order):\n" ++ omitted;
}

/// One entry per theme field: colors (`color_*`, `u32`), durations and other
/// integers (`u32`) and sizes (`f32`).
fn themeSource() []const u8 {
    const info = @typeInfo(zant.tokens.Theme).@"struct";
    var out: []const u8 = "";
    for (info.field_names, info.field_types) |name, T| {
        const color = T == u32 and std.mem.startsWith(u8, name, "color");
        const k = if (color) "color" else if (T == u32) "integer, .int_type = \"u32\"" else if (T == f32)
            "number"
        else
            @compileError("theme field " ++ name ++ " has an unsupported type");
        out = out ++ ".{ .name = " ++ quote(names.camel(name)) ++ ", .field = " ++ quote(name) ++ ", .kind = ." ++ k ++ " },\n";
    }
    return out;
}
