//! Dropdown menu panel in Ant Design's style (Dropdown + Menu tokens):
//! 8px radius, 4px padding, 32px items with 12px padding and 4px radius,
//! hover `fill_secondary`, press `primary_bg`, icon + label + extra
//! (shortcut) per item, dividers, group titles, disabled and danger items.
//!
//! Position it with `ui.styled(.., .{ .absolute = true, .top/.right = … })`
//! and put `ui.dismissLayer` before it so outside clicks close it.
const std = @import("std");
const ui = @import("../ui.zig");
const motion = @import("../motion/motion.zig");
const actions = @import("../core/actions.zig");
const icons = @import("../core/icons.zig");
const tokens = @import("../theme/tokens.zig");

pub const MenuItemKind = enum { item, divider, group };

pub const MenuItem = struct {
    kind: MenuItemKind = .item,
    key: ?[]const u8 = null,
    label: []const u8 = "",
    icon: icons.Icon = .none,
    /// Right-aligned secondary text (keyboard shortcut).
    extra: []const u8 = "",
    disabled: bool = false,
    danger: bool = false,
    /// Highlighted (selected) item.
    selected: bool = false,
    /// Nesting level (indents the label by 12px per level).
    indent: u8 = 0,
    on_click: ?actions.Action = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    /// Custom row content instead of icon/label/extra (still 32px high).
    content: ?ui.Node = null,

    pub const divider: MenuItem = .{ .kind = .divider };

    pub fn group(label: []const u8) MenuItem {
        return .{ .kind = .group, .label = label };
    }
};

pub const MenuOptions = struct {
    key: ?[]const u8 = "dropdown-menu",
    items: []const MenuItem,
    /// Panel width (antd: content-sized with a 120px minimum).
    width: f32 = 220,
    max_height: ?f32 = null,
    font_size: f32 = 14,
    /// Colour overrides (default: active theme tokens).
    background: ?u32 = null,
    text_color: ?u32 = null,
    extra_color: ?u32 = null,
    border_color: ?u32 = null,
    /// Enter / exit transition (antd dropdown: slide-up from the top edge).
    motion: motion.Presence = .slide_up,
    /// Corner the panel grows from (the side its trigger is on).
    motion_origin: motion.Origin = .top_right,
};

fn itemKey(arena: std.mem.Allocator, base: []const u8, item: MenuItem, i: usize) []const u8 {
    if (item.key) |k| return k;
    return std.fmt.allocPrint(arena, "{s}-{d}", .{ base, i }) catch base;
}

pub fn dropdownMenu(o: MenuOptions) ui.Node {
    const theme = ui.currentTheme();
    const arena = ui.frameAllocator();
    const fg = o.text_color orelse theme.color_text;
    const extra_fg = o.extra_color orelse theme.color_text_secondary;
    const inner_width = o.width - 8;
    const base_key = o.key orelse "dropdown-menu";
    var rows: std.ArrayListUnmanaged(ui.Node) = .empty;
    for (o.items, 0..) |item, i| {
        const key = itemKey(arena, base_key, item, i);
        switch (item.kind) {
            .divider => rows.append(arena, ui.divider(.{ .key = key, .color = theme.color_split, .margin = .{ .top = 4, .bottom = 4 } })) catch {},
            .group => rows.append(arena, ui.row(.{
                .key = key,
                .width = inner_width,
                .height = 28,
                .padding = .{ .left = 12, .right = 12 },
                .align_items = .center,
                .children = &.{ui.text(item.label, .{ .size = o.font_size - 2, .color = theme.color_text_disabled })},
            })) catch {},
            .item => {
                const label_color = if (item.disabled) theme.color_text_disabled else if (item.danger) theme.color_error else fg;
                var parts: std.ArrayListUnmanaged(ui.Node) = .empty;
                // the hit target sits behind the content so hover/press
                // backgrounds are painted under the label
                parts.append(arena, ui.styled(ui.button(.{
                    .key = std.fmt.allocPrint(arena, "{s}-hit", .{key}) catch key,
                    .text = "",
                    .variant = .text,
                    .disabled = item.disabled,
                    .focusable = false,
                    .on_click = item.on_click,
                    .index_action = item.index_action,
                    .index = item.index,
                }), .{
                    .absolute = true,
                    .left = 0,
                    .top = 0,
                    .width_percent = 100,
                    .height_percent = 100,
                    .radius = 4,
                    .background = if (item.selected) theme.color_primary_bg else null,
                })) catch {};
                if (item.content) |content| {
                    parts.append(arena, content) catch {};
                } else {
                    if (item.indent > 0) parts.append(arena, ui.space(.{ .width = @as(f32, @floatFromInt(item.indent)) * 12 })) catch {};
                    if (item.icon != .none) parts.append(arena, ui.icon(.{ .name = item.icon, .size = o.font_size, .color = if (item.selected) theme.color_primary else label_color })) catch {};
                    parts.append(arena, ui.styled(ui.text(item.label, .{
                        .size = o.font_size,
                        .color = if (item.selected) theme.color_primary else label_color,
                        .max_lines = 1,
                    }), .{ .grow = 1, .shrink = 1 })) catch {};
                    if (item.extra.len > 0) parts.append(arena, ui.text(item.extra, .{ .size = o.font_size - 1, .color = if (item.disabled) theme.color_text_disabled else extra_fg })) catch {};
                }
                rows.append(arena, ui.row(.{
                    .key = key,
                    .width = inner_width,
                    .height = 32,
                    .padding = .{ .left = 12, .right = 12 },
                    .gap = 8,
                    .align_items = .center,
                    .children = parts.items,
                })) catch {};
            },
        }
    }
    return ui.styled(ui.column(.{
        .key = base_key,
        .width = o.width,
        .padding = ui.inset(4),
        .children = rows.items,
        .scroll = o.max_height != null,
        .height = o.max_height,
    }), .{
        .background = o.background orelse theme.color_bg_elevated,
        .border_color = o.border_color orelse tokens.withAlpha(theme.color_split, 0x10),
        .border_width = 1,
        .radius = 8,
        .shadow_color = 0x1f000000,
        .shadow_blur = 16,
        .shadow_offset_y = 6,
        .clip = true,
        .block_pointer = true,
        .motion = o.motion,
        .motion_origin = o.motion_origin,
    });
}
