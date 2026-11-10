//! JSX tags `zant-tsx` knows and how their props map onto `ui.*` options
//! (`docs/tsx.md`, "全部 `ui.*` 组件"). The types below are the contract the
//! code generator relies on; `all` is the table it reads.
const std = @import("std");

pub const Kind = enum {
    /// f32 or ?f32 field: any number expression (ints convert).
    number,
    /// Integer field of type `int_type` (`numberOfLines` u16, `order` i32):
    /// an integer expression.
    integer,
    boolean,
    /// []const u8 or ?[]const u8 field: a string expression.
    string,
    /// Zig enum field: a string literal (or a `?:` of string literals) that
    /// must be one of `values` (its `.ts` spelling); emits `.zig`.
    enumeration,
    /// ?u32 ARGB field: "#rgb" / "#rrggbb" / "#rrggbbaa" string literals or a
    /// number expression (`0xff1677ff`).
    color,
    /// One or more sides of a `geometry.Edges` field (`padding`, `margin`):
    /// a number expression applied to `sides`. Several props on the same
    /// field combine; later props win per side.
    edges,
    /// f32/?f32 field that also takes a "50%" string literal, which sets
    /// `percent_field` instead.
    dimension,
    /// A static string literal that sets a bool field: `values` maps each
    /// accepted `.ts` spelling to the bool field (`.zig`) it sets to true;
    /// an empty `.zig` accepts the spelling and sets nothing.
    flag,
    /// RN `flex: n`: `grow = n, shrink = 1, basis = 0`.
    flex,
    /// `actions.Action` (`() => void`).
    action,
    /// `actions.BoolAction` (`(checked: boolean) => void`).
    bool_action,
    /// `actions.TextAction` (`(value: string) => void`).
    text_action,
    /// `actions.IndexAction` (`(index: usize) => void`).
    index_action,
    /// `ui.Node` field (required unless optional): an element expression.
    node,
    /// `?ui.Node` field: an element expression, or `null`.
    optional_node,
    /// `[]const ui.Node` field: the element's children (also settable as a
    /// prop holding an array of elements).
    children,
    /// `[]const []const u8` field: an array of strings.
    string_list,
    /// `[]const T` field with T a struct: an array of object literals whose
    /// keys are `item_props` (emits `&.{ .{ ... }, ... }`, element type
    /// `item_type`).
    struct_list,
};

pub const Sides = packed struct(u4) {
    top: bool = false,
    right: bool = false,
    bottom: bool = false,
    left: bool = false,

    pub const all: Sides = .{ .top = true, .right = true, .bottom = true, .left = true };
    pub const horizontal: Sides = .{ .right = true, .left = true };
    pub const vertical: Sides = .{ .top = true, .bottom = true };
};

pub const Value = struct {
    ts: []const u8,
    zig: []const u8,
};

pub const Prop = struct {
    /// TSX prop name (`backgroundColor`).
    name: []const u8,
    /// Zig options field (`background`).
    field: []const u8 = "",
    kind: Kind,
    /// `.enumeration` / `.flag` spellings.
    values: []const Value = &.{},
    /// `.edges`
    sides: Sides = .{},
    /// `.dimension`
    percent_field: []const u8 = "",
    /// `.integer`: the Zig integer type (`u16`, `i32`).
    int_type: []const u8 = "",
    /// The field has no default value: the prop must be given.
    required: bool = false,
    /// `.struct_list`: the element type as Zig code (`ui.TabItem`) and its
    /// fields as props (camelCase names, same kinds; nesting allowed).
    item_type: []const u8 = "",
    item_props: []const Prop = &.{},
    /// The field lives in a nested options struct that TSX flattens
    /// (`ClickableOptions.box`): emits `.parent = .{ .field = … }`. Empty for
    /// top-level fields.
    parent: []const u8 = "",
};

pub const Content = enum {
    /// Child elements go to `.children` (text children are an error).
    children,
    /// Text children form the first argument: `ui.text(value, .{...})`.
    text_arg,
    /// Text children set the string field `content_field` (Button `text`,
    /// Checkbox `label`), same as the prop.
    text_field,
    /// The only child element sets the `ui.Node` field `content_field`
    /// (`Tooltip` `child`), same as the prop.
    child,
    /// No children allowed.
    none,
};

pub const Tag = struct {
    /// JSX name (`View`).
    name: []const u8,
    /// Zig builder (`ui.view`).
    builder: []const u8,
    content: Content,
    content_field: []const u8 = "",
    props: []const Prop,
};

const generated = @import("ui_tags.zig");
const aliases = @import("tag_aliases.zig");

/// Omitted builders and reasons, emitted alongside the table for coverage checks.
pub const skipped = generated.skipped;

/// Replace matching names in declaration order, then append extra RN aliases.
/// Alias replacement is exact, including content overrides and enum spellings.
fn mergeProps(comptime native: []const Prop, comptime extra: []const Prop) []const Prop {
    var result: []const Prop = &.{};
    for (native) |prop| {
        var replacement = prop;
        for (extra) |alias| {
            if (std.mem.eql(u8, alias.name, prop.name)) replacement = alias;
        }
        result = result ++ [_]Prop{replacement};
    }
    for (extra) |alias| {
        var found = false;
        for (native) |prop| {
            if (std.mem.eql(u8, alias.name, prop.name)) found = true;
        }
        if (!found) result = result ++ [_]Prop{alias};
    }
    return result;
}

/// Every supported ui.* builder, in src/ui.zig declaration order.
pub const all: []const Tag = blk: {
    @setEvalBranchQuota(200_000);
    var result: []const Tag = &.{};
    for (generated.all) |native| {
        var tag = native;
        for (aliases.all) |alias| {
            if (std.mem.eql(u8, alias.name, tag.name)) {
                tag.props = mergeProps(tag.props, alias.props);
                if (alias.content) |content| {
                    tag.content = content;
                    tag.content_field = alias.content_field;
                }
            }
        }
        result = result ++ [_]Tag{tag};
    }
    break :blk result;
};

/// Fields of `zant.tokens.Theme` for `useTheme()` (`colorPrimary` →
/// `color_primary`), reflected with the tag table.
pub const theme_fields: []const Prop = generated.theme;

pub fn find(name: []const u8) ?*const Tag {
    for (all) |*tag| {
        if (std.mem.eql(u8, tag.name, name)) return tag;
    }
    return null;
}

pub fn findProp(tag: *const Tag, name: []const u8) ?*const Prop {
    for (tag.props) |*prop| {
        if (std.mem.eql(u8, prop.name, name)) return prop;
    }
    return null;
}

/// The Zig spelling for an `.enumeration` / `.flag` value, or null when the
/// value is not accepted.
pub fn findValue(prop: *const Prop, ts: []const u8) ?[]const u8 {
    for (prop.values) |v| {
        if (std.mem.eql(u8, v.ts, ts)) return v.zig;
    }
    return null;
}

test "find resolves every generated tag and rejects unknown names" {
    for (all) |*tag| {
        try std.testing.expect(find(tag.name).? == tag);
    }
    try std.testing.expect(find("") == null);
    try std.testing.expect(find("view") == null);
    try std.testing.expect(find("Unknown") == null);
}

test "findProp resolves every prop and keeps aliases tag specific" {
    for (all) |*tag| {
        for (tag.props) |*prop| {
            try std.testing.expect(findProp(tag, prop.name).? == prop);
        }
        if (findProp(tag, "key")) |key| try std.testing.expectEqual(Kind.string, key.kind);
        try std.testing.expect(findProp(tag, "unknown") == null);
    }
    const view = find("View").?;
    try std.testing.expectEqualStrings("background", findProp(view, "backgroundColor").?.field);
    try std.testing.expectEqualStrings("background", findProp(view, "background").?.field);
    try std.testing.expectEqualStrings("direction", findProp(view, "flexDirection").?.field);
    try std.testing.expectEqualStrings("flex_direction", findProp(find("Column").?, "flexDirection").?.field);
    try std.testing.expect(findProp(view, "BackgroundColor") == null);
    try std.testing.expectEqual(Kind.enumeration, findProp(view, "wrap").?.kind);
    try std.testing.expectEqual(Kind.boolean, findProp(find("Text").?, "wrap").?.kind);
}

test "findValue resolves enum aliases and distinguishes no-op flags from invalid values" {
    for (all) |*tag| {
        for (tag.props) |*prop| {
            for (prop.values) |value| {
                try std.testing.expectEqualStrings(value.zig, findValue(prop, value.ts).?);
            }
            try std.testing.expect(findValue(prop, "unknown") == null);
        }
    }
    const view = find("View").?;
    const justify = findProp(view, "justifyContent").?;
    try std.testing.expectEqualStrings("space_between", findValue(justify, "space-between").?);
    try std.testing.expectEqualStrings("space_between", findValue(justify, "space_between").?);
    const weight = findProp(find("Text").?, "fontWeight").?;
    try std.testing.expectEqualStrings("semibold", findValue(weight, "600").?);
    try std.testing.expectEqualStrings("extra_light", findValue(weight, "extraLight").?);
    try std.testing.expect(findValue(weight, "950") == null);
    const position = findProp(view, "position").?;
    try std.testing.expectEqualStrings("absolute", findValue(position, "absolute").?);
    try std.testing.expectEqualStrings("", findValue(position, "relative").?);
    try std.testing.expectEqualStrings("", findValue(findProp(view, "overflow").?, "visible").?);
    try std.testing.expect(findValue(position, "Relative") == null);
    try std.testing.expect(findValue(findProp(view, "gap").?, "0") == null);
}

// Run the generator's pure spelling tests with the compiler and reflection tests.
test {
    _ = @import("manifest/names.zig");
}
