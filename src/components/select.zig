const std = @import("std");
const node_mod = @import("../core/node.zig");
const tokens = @import("../theme/tokens.zig");
const text = @import("../renderer/text.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");
const motion = @import("../motion/motion.zig");

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

pub fn ensureLayout(data: *SelectData, color: u32, option_color: u32) !*text.TextLayout {
    const label_ok = !data.layout_dirty and data.layout != null and data.layout_color == color;
    const options_ok = data.option_layouts.items.len == data.options.items.len and data.option_color == option_color;
    if (label_ok and options_ok) return &data.layout.?;
    if (!label_ok) {
        if (data.layout) |*old| old.deinit();
        data.layout = null;
        data.layout = try text.TextLayout.init(selectedLabel(data), .{
            .font_size = data.font_size,
            .color = color,
            .wrap = false,
        }, 1.0e6);
        data.layout_color = color;
    }
    if (!options_ok) {
        data.option_color = option_color;
        for (data.option_layouts.items) |*layout| layout.deinit();
        data.option_layouts.clearRetainingCapacity();
        for (data.options.items) |option| {
            const option_layout = try text.TextLayout.init(option.label, .{
                .font_size = data.font_size,
                .color = option_color,
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
    const hover_t = if (enabled) data.hover_t else 0;
    const focus_t = if (enabled) data.focus_t else 0;
    const default_border = if (!enabled)
        theme.color_border
    else
        tokens.lerp(tokens.lerp(theme.color_border, theme.color_primary_hover, hover_t), theme.color_primary, focus_t);
    const border = node.visual.border_color orelse default_border;
    const bg = node.visual.background orelse if (enabled) theme.color_bg_container else theme.color_fill_secondary;
    if (focus_t > 0.01) {
        canvas.fillRoundRect(
            geometry.Rect.init(rect.x - 3, rect.y - 3, rect.width + 6, rect.height + 6),
            node.visual.radius + 3,
            tokens.withAlpha(theme.color_primary, @intFromFloat(0x18 * focus_t)),
        );
    }
    canvas.fillRoundRect(rect, node.visual.radius, bg);
    canvas.strokeRoundRect(
        geometry.Rect.init(rect.x + 0.5, rect.y + 0.5, @max(0, rect.width - 1), @max(0, rect.height - 1)),
        node.visual.radius,
        border,
        1.0 + 0.5 * focus_t,
    );
    const color = if (enabled) theme.color_text else theme.color_text_disabled;
    if (ensureLayout(data, color, theme.color_text)) |layout| {
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
    if (data.options.items.len == 0) return;
    // antd slide-up: grows from the edge next to the field while fading in
    const fx = motion.presenceFx(.slide_up, data.open_t);
    if (fx.opacity <= 0.003) return;
    const rect = popupRect(node, viewport);
    const above = rect.y < node.bounds.y;
    const origin = motion.originPoint(if (above) .bottom else .top, rect.x, rect.y, rect.width, rect.height);
    canvas.save();
    canvas.translate(origin[0], origin[1]);
    canvas.scale(fx.scale_x, fx.scale_y);
    canvas.translate(-origin[0], -origin[1]);
    const layer = fx.opacity < 0.997;
    if (layer) canvas.saveLayerAlpha(geometry.Rect.init(rect.x - 24, rect.y - 24, rect.width + 48, rect.height + 48), fx.opacity);
    defer {
        if (layer) canvas.restore();
        canvas.restore();
    }
    var shadow = rect;
    shadow.y += 6;
    canvas.shadowRoundRect(shadow, 8, 0x1f000000, 16);
    canvas.fillRoundRect(rect, 8, theme.color_bg_elevated);
    canvas.strokeRoundRect(rect, 8, theme.color_border_secondary, 1);
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

/// Border / ring / popup transitions (antd 200ms). Returns true while
/// another frame is needed.
pub fn animate(data: *SelectData, now: i64, theme: Theme) bool {
    const mid: motion.Transition = .{ .duration_ms = @intCast(theme.motion_duration_mid), .easing = .ease_in_out };
    data.hover.retarget(if (data.hovered) 1 else 0, now, mid);
    data.focus_anim.retarget(if (data.focused or data.open) 1 else 0, now, mid);
    data.open_anim.retarget(if (data.open) 1 else 0, now, if (data.open) motion.presenceEnter(.slide_up) else motion.presenceExit(.slide_up));
    data.hover_t = data.hover.value(now);
    data.focus_t = data.focus_anim.value(now);
    data.open_t = data.open_anim.value(now);
    return !(data.hover.done(now) and data.focus_anim.done(now) and data.open_anim.done(now));
}
