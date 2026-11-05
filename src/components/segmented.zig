//! Segmented control in Ant Design's style: a `bg_layout` track (2px
//! padding, 6px radius) with a raised `bg_elevated` thumb that slides to the
//! selected option (antd: 300ms ease-in-out).
const std = @import("std");
const ui = @import("../ui.zig");
const actions = @import("../core/actions.zig");
const icons = @import("../core/icons.zig");

pub const SegmentedOption = struct {
    label: []const u8 = "",
    icon: icons.Icon = .none,
    /// Hover hint (useful for icon-only options).
    tooltip: ?[]const u8 = null,
};

pub const SegmentedOptions = struct {
    key: ?[]const u8 = "segmented",
    options: []const SegmentedOption,
    selected: usize = 0,
    /// Receives the chosen option index.
    on_change: ?actions.IndexAction = null,
    /// Item height (antd: 28 small, 32 middle).
    height: f32 = 28,
    font_size: f32 = 13,
    item_width: ?f32 = null,
};

pub fn segmented(o: SegmentedOptions) ui.Node {
    const theme = ui.currentTheme();
    const arena = ui.frameAllocator();
    const base = o.key orelse "segmented";
    var items: std.ArrayListUnmanaged(ui.Node) = .empty;
    // the thumb is the first child (painted below the options) and follows
    // the selected option through `anchor`; FLIP animates the slide
    if (o.options.len > 0) items.append(arena, ui.styled(ui.column(.{
        .key = std.fmt.allocPrint(arena, "{s}-thumb", .{base}) catch base,
        .children = &.{},
    }), .{
        .absolute = true,
        .anchor = @intCast(@min(o.selected, o.options.len - 1)),
        .background = theme.color_bg_elevated,
        .radius = 4,
        .shadow_color = 0x14000000,
        .shadow_blur = 4,
        .shadow_offset_y = 1,
        .layout_transition = .{ .duration_ms = 300, .easing = .ease_in_out },
    })) catch {};
    for (o.options, 0..) |option, i| {
        const active = i == o.selected;
        const color = if (active) theme.color_text else theme.color_text_secondary;
        var content: std.ArrayListUnmanaged(ui.Node) = .empty;
        var hit = ui.styled(ui.button(.{
            .key = std.fmt.allocPrint(arena, "{s}-{d}-hit", .{ base, i }) catch base,
            .text = "",
            .variant = if (active) .plain else .text,
            .focusable = false,
            .index_action = o.on_change,
            .index = i,
        }), .{ .absolute = true, .left = 0, .top = 0, .width_percent = 100, .height_percent = 100, .radius = 4 });
        if (option.tooltip) |tip| hit = ui.tooltip(.{ .text = tip, .child = hit });
        content.append(arena, hit) catch {};
        if (option.icon != .none) content.append(arena, ui.icon(.{ .name = option.icon, .size = o.font_size + 1, .color = color })) catch {};
        if (option.label.len > 0) content.append(arena, ui.text(option.label, .{ .size = o.font_size, .color = color })) catch {};
        const item = ui.row(.{
            .key = std.fmt.allocPrint(arena, "{s}-{d}", .{ base, i }) catch base,
            .height = o.height - 4,
            .width = o.item_width,
            .padding = .{ .left = 11, .right = 11 },
            .gap = 6,
            .justify = .center,
            .align_items = .center,
            .children = content.items,
        });
        items.append(arena, item) catch {};
    }
    return ui.row(.{
        .key = base,
        .height = o.height,
        .padding = ui.inset(2),
        .gap = 2,
        .radius = 6,
        .background = theme.color_bg_layout,
        .align_items = .center,
        .children = items.items,
    });
}
