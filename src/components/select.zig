const std = @import("std");
const node_mod = @import("../core/node.zig");
const tokens = @import("../theme/tokens.zig");
const text = @import("../renderer/text.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");

pub const Node = node_mod.Node;
pub const SelectData = node_mod.SelectData;
pub const Theme = tokens.Theme;

pub const field_height: f32 = 32;
pub const padding_x: f32 = 11;
pub const row_height: f32 = 30;
pub const popup_padding: f32 = 4;

pub fn measureWidth(node: *const Node) f32 {
    if (node.style.width) |w| return w;
    return 220;
}

pub fn measureHeight(node: *const Node) f32 {
    if (node.style.height) |h| return h;
    return field_height;
}

pub fn selectedLabel(data: *const SelectData) []const u8 {
    if (data.selected < data.options.items.len) return data.options.items[data.selected].label;
    return "Select…";
}

pub fn ensureLayout(data: *SelectData, color: u32) !*text.TextLayout {
    if (!data.layout_dirty and data.layout != null) return &data.layout.?;
    if (data.layout) |*old| old.deinit();
    data.layout = try text.TextLayout.init(selectedLabel(data), .{
        .font_size = data.font_size,
        .color = color,
        .wrap = false,
    }, 1.0e6);
    if (data.option_layouts.items.len != data.options.items.len) {
        for (data.option_layouts.items) |*layout| layout.deinit();
        data.option_layouts.clearRetainingCapacity();
        for (data.options.items) |option| {
            const option_layout = try text.TextLayout.init(option.label, .{
                .font_size = data.font_size,
                .color = tokens.antLight().color_text,
                .wrap = false,
            }, 1.0e6);
            try data.option_layouts.append(std.heap.c_allocator, option_layout);
        }
    }
    data.layout_dirty = false;
    return &data.layout.?;
}

pub fn popupRect(node: *const Node, viewport: geometry.Size) geometry.Rect {
    const data = &node.data.select;
    const height = @as(f32, @floatFromInt(data.options.items.len)) * row_height + popup_padding * 2;
    var rect = geometry.Rect.init(node.bounds.x, node.bounds.bottom() + 4, node.bounds.width, height);
    if (rect.bottom() > viewport.height - 8) {
        rect.y = node.bounds.y - rect.height - 4;
    }
    if (rect.y < 8) rect.y = 8;
    return rect;
}

pub fn optionAt(node: *const Node, x: f32, y: f32, viewport: geometry.Size) ?usize {
    const data = &node.data.select;
    const rect = popupRect(node, viewport);
    if (!rect.contains(x, y)) return null;
    const local = y - rect.y - popup_padding;
    if (local < 0) return null;
    const index: usize = @intFromFloat(@floor(local / row_height));
    if (index >= data.options.items.len) return null;
    return index;
}

pub fn paint(node: *Node, canvas: Canvas, theme: Theme) void {
    const data = &node.data.select;
    const rect = node.bounds;
    const enabled = node.enabled;
    const default_border = if (!enabled)
        theme.color_border
    else if (data.open or data.focused)
        theme.color_primary
    else if (data.hovered)
        theme.color_primary_hover
    else
        theme.color_border;
    const border = node.visual.border_color orelse default_border;
    const bg = node.visual.background orelse if (enabled) theme.color_bg_container else theme.color_fill_secondary;
    if (data.focused and enabled) {
        canvas.fillRoundRect(
            geometry.Rect.init(rect.x - 3, rect.y - 3, rect.width + 6, rect.height + 6),
            node.visual.radius + 3,
            tokens.withAlpha(theme.color_primary, 0x18),
        );
    }
    canvas.fillRoundRect(rect, node.visual.radius, bg);
    canvas.strokeRoundRect(
        geometry.Rect.init(rect.x + 0.5, rect.y + 0.5, @max(0, rect.width - 1), @max(0, rect.height - 1)),
        node.visual.radius,
        border,
        if (data.open or data.focused) 1.5 else 1.0,
    );
    const color = if (enabled) theme.color_text else theme.color_text_disabled;
    if (ensureLayout(data, color)) |layout| {
        layout.paint(canvas, rect.x + padding_x, rect.y + (rect.height - layout.metrics.height) * 0.5);
    } else |_| {}
    const cx = rect.right() - 14;
    const cy = rect.y + rect.height * 0.5;
    const dir: f32 = if (data.open) -1 else 1;
    canvas.drawLine(cx - 4, cy - 2 * dir, cx, cy + 2 * dir, theme.color_text_secondary, 1.2);
    canvas.drawLine(cx, cy + 2 * dir, cx + 4, cy - 2 * dir, theme.color_text_secondary, 1.2);
}

pub fn paintPopup(node: *const Node, canvas: Canvas, theme: Theme, viewport: geometry.Size) void {
    const data = &node.data.select;
    if (!data.open or data.options.items.len == 0) return;
    const rect = popupRect(node, viewport);
    canvas.fillRoundRect(rect, 6, 0xd9ffffff);
    canvas.strokeRoundRect(rect, 6, theme.color_border_secondary, 1);
    canvas.save();
    canvas.clipRect(rect);
    for (data.options.items, 0..) |option, i| {
        const row = geometry.Rect.init(
            rect.x + popup_padding,
            rect.y + popup_padding + @as(f32, @floatFromInt(i)) * row_height,
            rect.width - popup_padding * 2,
            row_height,
        );
        if (data.hovered_option == i) {
            canvas.fillRoundRect(row, 4, theme.color_fill_secondary);
        }
        if (i == data.selected) {
            canvas.fillRoundRect(geometry.Rect.init(row.x, row.y + 7, 2, row.height - 14), 1, theme.color_primary);
        }
        if (i < data.option_layouts.items.len) {
            const layout = &data.option_layouts.items[i];
            layout.paint(canvas, row.x + 8, row.y + (row.height - layout.metrics.height) * 0.5);
        }
        _ = option;
    }
    canvas.restore();
}
