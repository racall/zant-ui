//! Hand-written React Native compatibility layer. Names here override reflected
//! props; native fields and new enum spellings continue to come from ui_tags.
const tags = @import("tags.zig");
const Prop = tags.Prop;
const Value = tags.Value;
const Sides = tags.Sides;

pub const Alias = struct {
    name: []const u8,
    props: []const Prop,
    content: ?tags.Content = null,
    content_field: []const u8 = "",
};

/// Alias enums inherit every reflected spelling before adding CSS / RN forms.
/// Keeping this lookup in the alias layer makes prop replacement exact without
/// maintaining a second list of native enum members.
fn enumValues(comptime tag_name: []const u8, comptime field: []const u8, comptime extra: []const Value) []const Value {
    const std = @import("std");
    for (@import("ui_tags.zig").all) |tag| {
        if (!std.mem.eql(u8, tag.name, tag_name)) continue;
        for (tag.props) |prop| {
            if (!std.mem.eql(u8, prop.field, field)) continue;
            var result = prop.values;
            for (extra) |value| {
                var found = false;
                for (result) |native| {
                    if (std.mem.eql(u8, native.ts, value.ts)) found = true;
                }
                if (!found) result = result ++ [_]Value{value};
            }
            return result;
        }
    }
    @compileError("Missing reflected enum " ++ tag_name ++ "." ++ field);
}

const css_align: []const Value = &.{
    .{ .ts = "flex-start", .zig = "start" },
    .{ .ts = "flex-end", .zig = "end" },
};
const justify_values = enumValues("Column", "justify", css_align);
const align_values = enumValues("Column", "align_items", css_align);
const align_self_values = enumValues("Column", "align_self", css_align);
const align_content_values = enumValues("Column", "align_content", css_align);
const wrap_values = enumValues("Column", "wrap", &.{});
const direction_values = enumValues("View", "direction", &.{});

const position_values: []const Value = &.{
    .{ .ts = "absolute", .zig = "absolute" },
    .{ .ts = "relative", .zig = "" },
};

const overflow_values: []const Value = &.{
    .{ .ts = "hidden", .zig = "clip" },
    .{ .ts = "scroll", .zig = "scroll" },
    .{ .ts = "visible", .zig = "" },
};

const weight_values = enumValues("Text", "weight", &.{
    .{ .ts = "normal", .zig = "regular" },
    .{ .ts = "semi-bold", .zig = "semibold" },
    .{ .ts = "100", .zig = "thin" },
    .{ .ts = "200", .zig = "extra_light" },
    .{ .ts = "300", .zig = "light" },
    .{ .ts = "400", .zig = "regular" },
    .{ .ts = "500", .zig = "medium" },
    .{ .ts = "600", .zig = "semibold" },
    .{ .ts = "700", .zig = "bold" },
    .{ .ts = "800", .zig = "extra_bold" },
    .{ .ts = "900", .zig = "black" },
});
const family_values = enumValues("Text", "family", &.{});

const box_props: []const Prop = &.{
    .{ .name = "paddingHorizontal", .field = "padding", .kind = .edges, .sides = Sides.horizontal },
    .{ .name = "paddingVertical", .field = "padding", .kind = .edges, .sides = Sides.vertical },
    .{ .name = "paddingTop", .field = "padding", .kind = .edges, .sides = .{ .top = true } },
    .{ .name = "paddingRight", .field = "padding", .kind = .edges, .sides = .{ .right = true } },
    .{ .name = "paddingBottom", .field = "padding", .kind = .edges, .sides = .{ .bottom = true } },
    .{ .name = "paddingLeft", .field = "padding", .kind = .edges, .sides = .{ .left = true } },
    .{ .name = "marginHorizontal", .field = "margin", .kind = .edges, .sides = Sides.horizontal },
    .{ .name = "marginVertical", .field = "margin", .kind = .edges, .sides = Sides.vertical },
    .{ .name = "marginTop", .field = "margin", .kind = .edges, .sides = .{ .top = true } },
    .{ .name = "marginRight", .field = "margin", .kind = .edges, .sides = .{ .right = true } },
    .{ .name = "marginBottom", .field = "margin", .kind = .edges, .sides = .{ .bottom = true } },
    .{ .name = "marginLeft", .field = "margin", .kind = .edges, .sides = .{ .left = true } },
    .{ .name = "width", .field = "width", .kind = .dimension, .percent_field = "width_percent" },
    .{ .name = "height", .field = "height", .kind = .dimension, .percent_field = "height_percent" },
    .{ .name = "flex", .kind = .flex },
    .{ .name = "flexGrow", .field = "grow", .kind = .number },
    .{ .name = "flexShrink", .field = "shrink", .kind = .number },
    .{ .name = "flexBasis", .field = "basis", .kind = .number },
    .{ .name = "justifyContent", .field = "justify_content", .kind = .enumeration, .values = justify_values },
    .{ .name = "justify", .field = "justify", .kind = .enumeration, .values = justify_values },
    .{ .name = "alignItems", .field = "align_items", .kind = .enumeration, .values = align_values },
    .{ .name = "alignSelf", .field = "align_self", .kind = .enumeration, .values = align_self_values },
    .{ .name = "alignContent", .field = "align_content", .kind = .enumeration, .values = align_content_values },
    .{ .name = "flexWrap", .field = "wrap", .kind = .enumeration, .values = wrap_values },
    .{ .name = "wrap", .field = "wrap", .kind = .enumeration, .values = wrap_values },
    .{ .name = "position", .kind = .flag, .values = position_values },
    .{ .name = "overflow", .kind = .flag, .values = overflow_values },
    .{ .name = "backgroundColor", .field = "background", .kind = .color },
    .{ .name = "borderRadius", .field = "radius", .kind = .number },
};

const view_props: []const Prop = box_props ++ &[_]Prop{
    .{ .name = "flexDirection", .field = "direction", .kind = .enumeration, .values = direction_values },
    .{ .name = "shadowRadius", .field = "shadow_blur", .kind = .number },
    .{ .name = "onPress", .field = "on_click", .kind = .action }, // React Native
};

const text_props: []const Prop = &.{
    .{ .name = "fontSize", .field = "size", .kind = .number },
    .{ .name = "fontWeight", .field = "weight", .kind = .enumeration, .values = weight_values },
    .{ .name = "weight", .field = "weight", .kind = .enumeration, .values = weight_values },
    .{ .name = "numberOfLines", .field = "max_lines", .kind = .integer, .int_type = "u16" },
    .{ .name = "fontFamily", .field = "family", .kind = .enumeration, .values = family_values },
};

const button_props: []const Prop = &.{
    .{ .name = "onPress", .field = "on_click", .kind = .action }, // React Native
};

/// Phase-1 names, CSS spellings, and text-child conventions.
pub const all: []const Alias = &.{
    .{ .name = "View", .props = view_props },
    .{ .name = "Column", .props = box_props },
    .{ .name = "Row", .props = box_props },
    .{ .name = "Text", .props = text_props },
    .{ .name = "Button", .props = button_props, .content = .text_field, .content_field = "text" },
    .{ .name = "Checkbox", .props = &.{}, .content = .text_field, .content_field = "label" },
    .{ .name = "Switch", .props = &.{}, .content = .text_field, .content_field = "label" },
};
