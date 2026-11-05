const std = @import("std");
const node_mod = @import("../core/node.zig");
const tokens = @import("../theme/tokens.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");

pub const Node = node_mod.Node;
pub const Theme = tokens.Theme;

pub const horizontal_padding_middle: f32 = 15;
pub const horizontal_padding_small: f32 = 7;
pub const horizontal_padding_large: f32 = 15;

pub fn controlHeight(size: node_mod.ButtonSize) f32 {
    return switch (size) {
        .small => 24,
        .middle => 32,
        .large => 40,
    };
}

pub fn horizontalPadding(size: node_mod.ButtonSize, button_type: node_mod.ButtonType) f32 {
    if (button_type == .text or button_type == .link) return 0;
    return switch (size) {
        .small => horizontal_padding_small,
        .middle => horizontal_padding_middle,
        .large => horizontal_padding_large,
    };
}

pub fn fontSize(size: node_mod.ButtonSize) f32 {
    return switch (size) {
        .small => 12,
        .middle => 14,
        .large => 16,
    };
}

fn textColor(data: *const node_mod.ButtonData, enabled: bool, theme: Theme) u32 {
    if (!enabled) return theme.color_text_disabled;
    return switch (data.button_type) {
        .primary => theme.color_text_light,
        .default => theme.color_text,
        .text => theme.color_text,
        .link => theme.color_primary,
    };
}

pub fn measureWidth(node: *Node, theme: Theme, max_width: f32) f32 {
    _ = max_width;
    const data = &node.data.button;
    if (node.style.width) |w| return w;
    const color = textColor(data, node.enabled, theme);
    const layout = data.ensureLayout(color) catch return controlHeight(data.size) * 2;
    return @min(
        layout.metrics.longest_line + horizontalPadding(data.size, data.button_type) * 2,
        if (node.style.max_width) |m| m else std.math.inf(f32),
    );
}

pub fn measureHeight(node: *const Node) f32 {
    if (node.style.height) |h| return h;
    return controlHeight(node.data.button.size);
}

fn baseColors(data: *const node_mod.ButtonData, enabled: bool, theme: Theme) struct {
    bg: u32,
    hover_bg: u32,
    active_bg: u32,
    border: u32,
    hover_border: u32,
    active_border: u32,
} {
    if (!enabled) {
        return .{
            .bg = theme.color_fill_secondary,
            .hover_bg = theme.color_fill_secondary,
            .active_bg = theme.color_fill_secondary,
            .border = theme.color_border,
            .hover_border = theme.color_border,
            .active_border = theme.color_border,
        };
    }
    return switch (data.button_type) {
        .primary => .{
            .bg = theme.color_primary,
            .hover_bg = theme.color_primary_hover,
            .active_bg = theme.color_primary_active,
            .border = theme.color_primary,
            .hover_border = theme.color_primary_hover,
            .active_border = theme.color_primary_active,
        },
        .default => .{
            .bg = theme.color_bg_container,
            .hover_bg = theme.color_bg_container,
            .active_bg = theme.color_bg_container,
            .border = theme.color_border,
            .hover_border = theme.color_primary_hover,
            .active_border = theme.color_primary_active,
        },
        .text => .{
            .bg = 0x00000000,
            .hover_bg = theme.color_fill_secondary,
            .active_bg = theme.color_primary_bg,
            .border = 0x00000000,
            .hover_border = 0x00000000,
            .active_border = 0x00000000,
        },
        .link => .{
            .bg = 0x00000000,
            .hover_bg = 0x00000000,
            .active_bg = 0x00000000,
            .border = 0x00000000,
            .hover_border = 0x00000000,
            .active_border = 0x00000000,
        },
    };
}

pub fn paint(node: *Node, canvas: Canvas, theme: Theme) void {
    const data = &node.data.button;
    if (!node.visible) return;
    const enabled = node.enabled;
    const colors = baseColors(data, enabled, theme);
    const hover_t = if (enabled) data.hover_t else 0;
    const press_t = if (enabled) data.press_t else 0;
    const animated_bg = tokens.lerp(tokens.lerp(colors.bg, colors.hover_bg, hover_t), colors.active_bg, press_t);
    const animated_border = tokens.lerp(tokens.lerp(colors.border, colors.hover_border, hover_t), colors.active_border, press_t);
    const bg = node.visual.background orelse animated_bg;
    const border = node.visual.border_color orelse animated_border;
    const rect = node.bounds;

    if (tokens.alpha(bg) > 0) {
        canvas.fillRoundRect(rect, node.visual.radius, bg);
    }
    if (data.button_type == .default and tokens.alpha(border) > 0) {
        canvas.strokeRoundRect(
            geometry.Rect.init(rect.x + 0.5, rect.y + 0.5, @max(0, rect.width - 1), @max(0, rect.height - 1)),
            node.visual.radius,
            border,
            1,
        );
    }

    const color = data.text_color orelse textColor(data, enabled, theme);
    if (data.ensureLayout(color)) |layout| {
        const x = switch (data.text_align) {
            .left => rect.x + if (data.label_padding > 0) data.label_padding else horizontalPadding(data.size, data.button_type),
            .right => rect.right() - layout.metrics.longest_line - if (data.label_padding > 0) data.label_padding else horizontalPadding(data.size, data.button_type),
            .center => rect.x + (rect.width - layout.metrics.longest_line) * 0.5,
        };
        const y = rect.y + (rect.height - layout.metrics.height) * 0.5;
        layout.paint(canvas, x, y);
    } else |_| {}

    if (data.focused and enabled) {
        const ring = geometry.Rect.init(rect.x - 2, rect.y - 2, rect.width + 4, rect.height + 4);
        canvas.strokeRoundRect(ring, node.visual.radius + 2, tokens.withAlpha(theme.color_primary, 0x66), 2);
    }
}

fn approach(current: f32, target: f32, delta: f32) f32 {
    if (current < target) return @min(current + delta, target);
    return @max(current - delta, target);
}

/// Advances hover/pressed transitions. Returns true while another frame is needed.
pub fn animate(data: *node_mod.ButtonData, dt_ms: f32, theme: Theme) bool {
    if (data.hover_t == 0 and data.press_t == 0 and !data.hovered and !data.pressed) return false;
    const target_hover: f32 = if (data.hovered) 1 else 0;
    const target_press: f32 = if (data.pressed) 1 else 0;
    const hover_step = dt_ms / @as(f32, @floatFromInt(theme.motion_duration_mid));
    const press_step = dt_ms / @as(f32, @floatFromInt(theme.motion_duration_fast));
    data.hover_t = approach(data.hover_t, target_hover, hover_step);
    data.press_t = approach(data.press_t, target_press, press_step);
    return data.hover_t != target_hover or data.press_t != target_press;
}

pub fn setEnabled(node: *Node, enabled: bool, theme: Theme) void {
    if (node.enabled == enabled) return;
    node.enabled = enabled;
    const data = &node.data.button;
    if (!enabled) {
        data.hovered = false;
        data.pressed = false;
        data.focused = false;
        data.hover_t = 0;
        data.press_t = 0;
    }
    const color = textColor(data, enabled, theme);
    data.ensureLayout(color) catch {};
}
