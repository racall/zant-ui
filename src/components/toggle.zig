const std = @import("std");
const node_mod = @import("../core/node.zig");
const tokens = @import("../theme/tokens.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");

pub const Node = node_mod.Node;
pub const ToggleData = node_mod.ToggleData;
pub const Theme = tokens.Theme;

pub const checkbox_size: f32 = 16;
pub const switch_width: f32 = 40;
pub const switch_height: f32 = 22;
pub const label_gap: f32 = 8;
pub const control_height: f32 = 24;

fn labelColor(node: *const Node, theme: Theme) u32 {
    return if (node.enabled) theme.color_text else theme.color_text_disabled;
}

pub fn measureWidth(node: *Node, theme: Theme, max_width: f32) f32 {
    _ = max_width;
    if (node.style.width) |w| return w;
    const data = &node.data.toggle;
    const layout = data.ensureLayout(labelColor(node, theme)) catch return 32;
    const indicator: f32 = switch (data.kind) {
        .checkbox, .radio => checkbox_size,
        .switch_control => switch_width,
    };
    const gap: f32 = if (data.label.len > 0) label_gap else 0;
    return indicator + gap + layout.metrics.longest_line;
}

pub fn measureHeight(node: *const Node) f32 {
    if (node.style.height) |h| return h;
    return control_height;
}

pub fn paint(node: *Node, canvas: Canvas, theme: Theme) void {
    if (!node.visible) return;
    const data = &node.data.toggle;
    const enabled = node.enabled;
    const rect = node.bounds;
    const color = labelColor(node, theme);
    const hover = if (enabled) data.hover_t else 0;
    const press = if (enabled) data.press_t else 0;
    const check = data.check_t;

    if (data.kind == .radio) {
        const box = geometry.Rect.init(rect.x, rect.y + (rect.height - checkbox_size) * 0.5, checkbox_size, checkbox_size);
        const border = if (enabled)
            tokens.lerp(tokens.lerp(theme.color_border, theme.color_primary_hover, hover), theme.color_primary, check)
        else
            theme.color_border;
        canvas.fillRoundRect(box, checkbox_size * 0.5, if (enabled) theme.color_bg_container else theme.color_fill_secondary);
        canvas.strokeRoundRect(box, checkbox_size * 0.5, border, 1.2);
        if (check > 0.02) {
            const dot = checkbox_size * 0.5 * check;
            const cx = box.x + box.width * 0.5;
            const cy = box.y + box.height * 0.5;
            canvas.fillRoundRect(geometry.Rect.init(cx - dot, cy - dot, dot * 2, dot * 2), dot, theme.color_primary);
        }
        if (data.focused and enabled) {
            canvas.strokeRoundRect(
                geometry.Rect.init(box.x - 2, box.y - 2, box.width + 4, box.height + 4),
                checkbox_size * 0.5 + 2,
                tokens.withAlpha(theme.color_primary, 0x66),
                2,
            );
        }
        if (data.ensureLayout(color)) |layout| {
            layout.paint(canvas, rect.x + checkbox_size + label_gap, rect.y + (rect.height - layout.metrics.height) * 0.5);
        } else |_| {}
    } else if (data.kind == .checkbox) {
        const box = geometry.Rect.init(rect.x, rect.y + (rect.height - checkbox_size) * 0.5, checkbox_size, checkbox_size);
        const fill = if (enabled)
            tokens.lerp(theme.color_bg_container, theme.color_primary, check)
        else
            theme.color_fill_secondary;
        const border = if (enabled)
            tokens.lerp(tokens.lerp(theme.color_border, theme.color_primary_hover, hover), theme.color_primary, check)
        else
            theme.color_border;
        canvas.fillRoundRect(box, theme.border_radius_sm * 0.5 + 2, fill);
        canvas.strokeRoundRect(box, theme.border_radius_sm * 0.5 + 2, border, 1);
        if (check > 0.02) {
            const check_color = tokens.withAlpha(theme.color_text_light, @intFromFloat(255.0 * check));
            const cx = box.x + box.width * 0.5;
            const cy = box.y + box.height * 0.5;
            canvas.drawLine(cx - 3.5, cy, cx - 1, cy + 2.5, check_color, 1.8);
            canvas.drawLine(cx - 1, cy + 2.5, cx + 4, cy - 3, check_color, 1.8);
        }
        if (data.focused and enabled) {
            canvas.strokeRoundRect(
                geometry.Rect.init(box.x - 2, box.y - 2, box.width + 4, box.height + 4),
                theme.border_radius_sm,
                tokens.withAlpha(theme.color_primary, 0x66),
                2,
            );
        }
        if (data.ensureLayout(color)) |layout| {
            const x = rect.x + checkbox_size + label_gap;
            const y = rect.y + (rect.height - layout.metrics.height) * 0.5;
            layout.paint(canvas, x, y);
        } else |_| {}
    } else {
        const track = geometry.Rect.init(rect.x, rect.y + (rect.height - switch_height) * 0.5, switch_width, switch_height);
        const base = if (enabled) tokens.lerp(tokens.withAlpha(0xffbfbfbf, 0xff), theme.color_primary_hover, hover) else theme.color_fill_secondary;
        const track_color = if (enabled) tokens.lerp(base, theme.color_primary, check) else base;
        canvas.fillRoundRect(track, switch_height * 0.5, track_color);
        const knob_x = track.x + 2 + (switch_width - switch_height) * check;
        const knob = geometry.Rect.init(knob_x, track.y + 2, switch_height - 4, switch_height - 4);
        canvas.fillRoundRect(knob, knob.width * 0.5, if (enabled) theme.color_bg_container else theme.color_bg_container);
        if (data.focused and enabled) {
            canvas.strokeRoundRect(
                geometry.Rect.init(track.x - 2, track.y - 2, track.width + 4, track.height + 4),
                (track.height + 4) * 0.5,
                tokens.withAlpha(theme.color_primary, 0x44),
                2,
            );
        }
        if (data.ensureLayout(color)) |layout| {
            const x = rect.x + switch_width + label_gap;
            const y = rect.y + (rect.height - layout.metrics.height) * 0.5;
            layout.paint(canvas, x, y);
        } else |_| {}
    }
    _ = press;
}

fn approach(current: f32, target: f32, delta: f32) f32 {
    if (current < target) return @min(current + delta, target);
    return @max(current - delta, target);
}

pub fn animate(data: *ToggleData, dt_ms: f32, theme: Theme) bool {
    const target_hover: f32 = if (data.hovered) 1 else 0;
    const target_press: f32 = if (data.pressed) 1 else 0;
    const target_check: f32 = if (data.checked) 1 else 0;
    const hover_step = dt_ms / @as(f32, @floatFromInt(theme.motion_duration_mid));
    const press_step = dt_ms / @as(f32, @floatFromInt(theme.motion_duration_fast));
    const check_step = dt_ms / @as(f32, @floatFromInt(theme.motion_duration_mid));
    data.hover_t = approach(data.hover_t, target_hover, hover_step);
    data.press_t = approach(data.press_t, target_press, press_step);
    data.check_t = approach(data.check_t, target_check, check_step);
    return data.hover_t != target_hover or data.press_t != target_press or data.check_t != target_check;
}
