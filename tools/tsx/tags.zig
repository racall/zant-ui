//! JSX tags `zant-tsx` knows and how their props map onto `ui.*` options.
//!
//! Phase 1 table, written by hand from `src/ui.zig` / `src/components/view.zig`
//! (`docs/tsx.md` lists it). Prop names follow React Native style names
//! (camelCase); aliases map onto the same Zig field.
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
};

pub const Content = enum {
    /// Child elements go to `.children` (text children are an error).
    children,
    /// Text children form the first argument: `ui.text(value, .{...})`.
    text_arg,
    /// Text children set the string field `content_field` (Button `text`,
    /// Checkbox `label`), same as the prop.
    text_field,
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

const justify_values: []const Value = &.{
    .{ .ts = "flex-start", .zig = "start" },
    .{ .ts = "start", .zig = "start" },
    .{ .ts = "center", .zig = "center" },
    .{ .ts = "flex-end", .zig = "end" },
    .{ .ts = "end", .zig = "end" },
    .{ .ts = "space-between", .zig = "space_between" },
    .{ .ts = "space_between", .zig = "space_between" },
    .{ .ts = "space-around", .zig = "space_around" },
    .{ .ts = "space_around", .zig = "space_around" },
    .{ .ts = "space-evenly", .zig = "space_evenly" },
    .{ .ts = "space_evenly", .zig = "space_evenly" },
};

const align_values: []const Value = &.{
    .{ .ts = "flex-start", .zig = "start" },
    .{ .ts = "start", .zig = "start" },
    .{ .ts = "center", .zig = "center" },
    .{ .ts = "flex-end", .zig = "end" },
    .{ .ts = "end", .zig = "end" },
    .{ .ts = "stretch", .zig = "stretch" },
};

const align_self_values: []const Value = &[_]Value{.{ .ts = "auto", .zig = "auto" }} ++ align_values;
const align_content_values: []const Value = align_values ++ &[_]Value{
    .{ .ts = "space-between", .zig = "space_between" },
    .{ .ts = "space_between", .zig = "space_between" },
    .{ .ts = "space-around", .zig = "space_around" },
    .{ .ts = "space_around", .zig = "space_around" },
};

const wrap_values: []const Value = &.{
    .{ .ts = "nowrap", .zig = "nowrap" },
    .{ .ts = "wrap", .zig = "wrap" },
    .{ .ts = "wrap-reverse", .zig = "wrap_reverse" },
    .{ .ts = "wrap_reverse", .zig = "wrap_reverse" },
};

const direction_values: []const Value = &.{
    .{ .ts = "row", .zig = "row" },
    .{ .ts = "column", .zig = "column" },
};

const position_values: []const Value = &.{
    .{ .ts = "absolute", .zig = "absolute" },
    .{ .ts = "relative", .zig = "" },
};

const overflow_values: []const Value = &.{
    .{ .ts = "hidden", .zig = "clip" },
    .{ .ts = "scroll", .zig = "scroll" },
    .{ .ts = "visible", .zig = "" },
};

const weight_values: []const Value = &.{
    .{ .ts = "thin", .zig = "thin" },
    .{ .ts = "extra-light", .zig = "extra_light" },
    .{ .ts = "extraLight", .zig = "extra_light" },
    .{ .ts = "extra_light", .zig = "extra_light" },
    .{ .ts = "light", .zig = "light" },
    .{ .ts = "regular", .zig = "regular" },
    .{ .ts = "normal", .zig = "regular" },
    .{ .ts = "medium", .zig = "medium" },
    .{ .ts = "semibold", .zig = "semibold" },
    .{ .ts = "semi-bold", .zig = "semibold" },
    .{ .ts = "bold", .zig = "bold" },
    .{ .ts = "extra-bold", .zig = "extra_bold" },
    .{ .ts = "extraBold", .zig = "extra_bold" },
    .{ .ts = "extra_bold", .zig = "extra_bold" },
    .{ .ts = "black", .zig = "black" },
    .{ .ts = "100", .zig = "thin" },
    .{ .ts = "200", .zig = "extra_light" },
    .{ .ts = "300", .zig = "light" },
    .{ .ts = "400", .zig = "regular" },
    .{ .ts = "500", .zig = "medium" },
    .{ .ts = "600", .zig = "semibold" },
    .{ .ts = "700", .zig = "bold" },
    .{ .ts = "800", .zig = "extra_bold" },
    .{ .ts = "900", .zig = "black" },
};

const text_align_values: []const Value = &.{
    .{ .ts = "left", .zig = "left" },
    .{ .ts = "center", .zig = "center" },
    .{ .ts = "right", .zig = "right" },
};

const family_values: []const Value = &.{
    .{ .ts = "sans", .zig = "sans" },
    .{ .ts = "serif", .zig = "serif" },
    .{ .ts = "mono", .zig = "mono" },
};

const variant_values: []const Value = &.{
    .{ .ts = "primary", .zig = "primary" },
    .{ .ts = "default", .zig = "default" },
    .{ .ts = "text", .zig = "text" },
    .{ .ts = "link", .zig = "link" },
};

const size_values: []const Value = &.{
    .{ .ts = "small", .zig = "small" },
    .{ .ts = "middle", .zig = "middle" },
    .{ .ts = "large", .zig = "large" },
};

const key_props: []const Prop = &.{
    .{ .name = "key", .field = "key", .kind = .string },
};

const box_props: []const Prop = key_props ++ &[_]Prop{
    .{ .name = "gap", .field = "gap", .kind = .number },
    .{ .name = "rowGap", .field = "row_gap", .kind = .number },
    .{ .name = "columnGap", .field = "column_gap", .kind = .number },
    .{ .name = "padding", .field = "padding", .kind = .edges, .sides = Sides.all },
    .{ .name = "paddingHorizontal", .field = "padding", .kind = .edges, .sides = Sides.horizontal },
    .{ .name = "paddingVertical", .field = "padding", .kind = .edges, .sides = Sides.vertical },
    .{ .name = "paddingTop", .field = "padding", .kind = .edges, .sides = .{ .top = true } },
    .{ .name = "paddingRight", .field = "padding", .kind = .edges, .sides = .{ .right = true } },
    .{ .name = "paddingBottom", .field = "padding", .kind = .edges, .sides = .{ .bottom = true } },
    .{ .name = "paddingLeft", .field = "padding", .kind = .edges, .sides = .{ .left = true } },
    .{ .name = "margin", .field = "margin", .kind = .edges, .sides = Sides.all },
    .{ .name = "marginHorizontal", .field = "margin", .kind = .edges, .sides = Sides.horizontal },
    .{ .name = "marginVertical", .field = "margin", .kind = .edges, .sides = Sides.vertical },
    .{ .name = "marginTop", .field = "margin", .kind = .edges, .sides = .{ .top = true } },
    .{ .name = "marginRight", .field = "margin", .kind = .edges, .sides = .{ .right = true } },
    .{ .name = "marginBottom", .field = "margin", .kind = .edges, .sides = .{ .bottom = true } },
    .{ .name = "marginLeft", .field = "margin", .kind = .edges, .sides = .{ .left = true } },
    .{ .name = "width", .field = "width", .kind = .dimension, .percent_field = "width_percent" },
    .{ .name = "height", .field = "height", .kind = .dimension, .percent_field = "height_percent" },
    .{ .name = "minWidth", .field = "min_width", .kind = .number },
    .{ .name = "minHeight", .field = "min_height", .kind = .number },
    .{ .name = "maxWidth", .field = "max_width", .kind = .number },
    .{ .name = "maxHeight", .field = "max_height", .kind = .number },
    .{ .name = "flex", .kind = .flex },
    .{ .name = "flexGrow", .field = "grow", .kind = .number },
    .{ .name = "grow", .field = "grow", .kind = .number },
    .{ .name = "flexShrink", .field = "shrink", .kind = .number },
    .{ .name = "shrink", .field = "shrink", .kind = .number },
    .{ .name = "flexBasis", .field = "basis", .kind = .number },
    .{ .name = "basis", .field = "basis", .kind = .number },
    .{ .name = "justifyContent", .field = "justify_content", .kind = .enumeration, .values = justify_values },
    .{ .name = "justify", .field = "justify", .kind = .enumeration, .values = justify_values },
    .{ .name = "alignItems", .field = "align_items", .kind = .enumeration, .values = align_values },
    .{ .name = "alignSelf", .field = "align_self", .kind = .enumeration, .values = align_self_values },
    .{ .name = "alignContent", .field = "align_content", .kind = .enumeration, .values = align_content_values },
    .{ .name = "flexWrap", .field = "wrap", .kind = .enumeration, .values = wrap_values },
    .{ .name = "wrap", .field = "wrap", .kind = .enumeration, .values = wrap_values },
    .{ .name = "position", .kind = .flag, .values = position_values },
    .{ .name = "top", .field = "top", .kind = .number },
    .{ .name = "right", .field = "right", .kind = .number },
    .{ .name = "bottom", .field = "bottom", .kind = .number },
    .{ .name = "left", .field = "left", .kind = .number },
    .{ .name = "overflow", .kind = .flag, .values = overflow_values },
    .{ .name = "backgroundColor", .field = "background", .kind = .color },
    .{ .name = "background", .field = "background", .kind = .color },
    .{ .name = "borderColor", .field = "border_color", .kind = .color },
    .{ .name = "borderWidth", .field = "border_width", .kind = .number },
    .{ .name = "borderRadius", .field = "radius", .kind = .number },
    .{ .name = "radius", .field = "radius", .kind = .number },
    .{ .name = "hoverBackground", .field = "hover_background", .kind = .color },
    .{ .name = "hoverBorderColor", .field = "hover_border_color", .kind = .color },
    .{ .name = "order", .field = "order", .kind = .integer, .int_type = "i32" },
};

const view_props: []const Prop = box_props ++ &[_]Prop{
    .{ .name = "flexDirection", .field = "direction", .kind = .enumeration, .values = direction_values },
    .{ .name = "direction", .field = "direction", .kind = .enumeration, .values = direction_values },
    .{ .name = "shadowColor", .field = "shadow_color", .kind = .color },
    .{ .name = "shadowBlur", .field = "shadow_blur", .kind = .number },
    .{ .name = "shadowRadius", .field = "shadow_blur", .kind = .number },
    .{ .name = "shadowOffsetY", .field = "shadow_offset_y", .kind = .number },
    .{ .name = "onClick", .field = "on_click", .kind = .action },
    .{ .name = "onPress", .field = "on_click", .kind = .action }, // React Native
};

const text_props: []const Prop = key_props ++ &[_]Prop{
    .{ .name = "fontSize", .field = "size", .kind = .number },
    .{ .name = "size", .field = "size", .kind = .number },
    .{ .name = "fontWeight", .field = "weight", .kind = .enumeration, .values = weight_values },
    .{ .name = "weight", .field = "weight", .kind = .enumeration, .values = weight_values },
    .{ .name = "color", .field = "color", .kind = .color },
    .{ .name = "textAlign", .field = "text_align", .kind = .enumeration, .values = text_align_values },
    .{ .name = "wrap", .field = "wrap", .kind = .boolean },
    .{ .name = "numberOfLines", .field = "max_lines", .kind = .integer, .int_type = "u16" },
    .{ .name = "maxLines", .field = "max_lines", .kind = .integer, .int_type = "u16" },
    .{ .name = "lineHeight", .field = "line_height", .kind = .number },
    .{ .name = "fontFamily", .field = "family", .kind = .enumeration, .values = family_values },
    .{ .name = "family", .field = "family", .kind = .enumeration, .values = family_values },
};

// Controls have numeric dimensions only: their options have no percent fields.
const button_props: []const Prop = key_props ++ &[_]Prop{
    .{ .name = "text", .field = "text", .kind = .string },
    .{ .name = "variant", .field = "variant", .kind = .enumeration, .values = variant_values },
    .{ .name = "size", .field = "size", .kind = .enumeration, .values = size_values },
    .{ .name = "disabled", .field = "disabled", .kind = .boolean },
    .{ .name = "width", .field = "width", .kind = .number },
    .{ .name = "fontSize", .field = "font_size", .kind = .number },
    .{ .name = "onClick", .field = "on_click", .kind = .action },
    .{ .name = "onPress", .field = "on_click", .kind = .action }, // React Native
};

const input_props: []const Prop = key_props ++ &[_]Prop{
    .{ .name = "value", .field = "value", .kind = .string },
    .{ .name = "placeholder", .field = "placeholder", .kind = .string },
    .{ .name = "disabled", .field = "disabled", .kind = .boolean },
    .{ .name = "readOnly", .field = "read_only", .kind = .boolean },
    .{ .name = "password", .field = "password", .kind = .boolean },
    .{ .name = "multiline", .field = "multiline", .kind = .boolean },
    .{ .name = "width", .field = "width", .kind = .number },
    .{ .name = "height", .field = "height", .kind = .number },
    .{ .name = "fontSize", .field = "font_size", .kind = .number },
    .{ .name = "onChange", .field = "on_change", .kind = .text_action },
    .{ .name = "onSubmit", .field = "on_submit", .kind = .action },
};

const toggle_props: []const Prop = key_props ++ &[_]Prop{
    .{ .name = "label", .field = "label", .kind = .string },
    .{ .name = "checked", .field = "checked", .kind = .boolean },
    .{ .name = "disabled", .field = "disabled", .kind = .boolean },
    .{ .name = "fontSize", .field = "font_size", .kind = .number },
    .{ .name = "onChange", .field = "on_change", .kind = .bool_action },
};

/// Every tag, in `docs/tsx.md` order.
pub const all: []const Tag = &.{
    .{ .name = "View", .builder = "ui.view", .content = .children, .props = view_props },
    .{ .name = "Column", .builder = "ui.column", .content = .children, .props = box_props },
    .{ .name = "Row", .builder = "ui.row", .content = .children, .props = box_props },
    .{ .name = "Text", .builder = "ui.text", .content = .text_arg, .props = text_props },
    .{ .name = "Button", .builder = "ui.button", .content = .text_field, .content_field = "text", .props = button_props },
    .{ .name = "Input", .builder = "ui.input", .content = .none, .props = input_props },
    .{ .name = "Checkbox", .builder = "ui.checkbox", .content = .text_field, .content_field = "label", .props = toggle_props },
    .{ .name = "Switch", .builder = "ui.@\"switch\"", .content = .text_field, .content_field = "label", .props = toggle_props },
};

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

test "find returns the eight tags in spec order and rejects unknown names" {
    const names = [_][]const u8{ "View", "Column", "Row", "Text", "Button", "Input", "Checkbox", "Switch" };
    try std.testing.expectEqual(names.len, all.len);
    for (names, all) |name, *tag| {
        try std.testing.expectEqualStrings(name, tag.name);
        try std.testing.expect(find(name).? == tag);
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
        try std.testing.expectEqual(Kind.string, findProp(tag, "key").?.kind);
        try std.testing.expect(findProp(tag, "unknown") == null);
    }
    const view = find("View").?;
    try std.testing.expectEqualStrings("background", findProp(view, "backgroundColor").?.field);
    try std.testing.expectEqualStrings("background", findProp(view, "background").?.field);
    try std.testing.expectEqualStrings("direction", findProp(view, "flexDirection").?.field);
    try std.testing.expect(findProp(find("Column").?, "flexDirection") == null);
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
