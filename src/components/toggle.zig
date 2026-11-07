const std = @import("std");
const node_mod = @import("../core/node.zig");
const tokens = @import("../theme/tokens.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");
const motion = @import("../motion/motion.zig");

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
    const indicator: f32 = switch (data.kind) {
        .checkbox, .radio => checkbox_size,
        .switch_control => switch_width,
    };
    // no label: just the control (an empty paragraph's line width is not a
    // number, which made a bare switch measure 0 wide)
    if (data.label.len == 0) return indicator;
    const layout = data.ensureLayout(labelColor(node, theme)) catch return indicator;
    const line = layout.metrics.longest_line;
    return indicator + label_gap + (if (std.math.isFinite(line)) line else 0);
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
    const mark = data.mark_t;

    if (data.kind == .radio) {
        const box = geometry.Rect.init(rect.x, rect.y + (rect.height - checkbox_size) * 0.5, checkbox_size, checkbox_size);
        const border = if (enabled)
            tokens.lerp(tokens.lerp(theme.color_border, theme.color_primary_hover, hover), theme.color_primary, check)
        else
            theme.color_border;
        canvas.fillRoundRect(box, checkbox_size * 0.5, if (enabled) theme.color_bg_container else theme.color_fill_secondary);
        canvas.strokeRoundRect(box, checkbox_size * 0.5, border, 1.2);
        if (mark > 0.02) {
            // antd: the inner dot scales in (dotSize = 8px of 16)
            const dot = checkbox_size * 0.25 * mark;
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
        // disabled + checked keeps a dimmed primary fill so the mark stays visible
        const fill = if (enabled)
            tokens.lerp(theme.color_bg_container, theme.color_primary, check)
        else if (check > 0.5)
            tokens.withAlpha(theme.color_primary, 0x66)
        else
            theme.color_fill_secondary;
        const border = if (enabled)
            tokens.lerp(tokens.lerp(theme.color_border, theme.color_primary_hover, hover), theme.color_primary, check)
        else
            theme.color_border;
        canvas.fillRoundRect(box, theme.border_radius_sm * 0.5 + 2, fill);
        canvas.strokeRoundRect(box, theme.border_radius_sm * 0.5 + 2, border, 1);
        if (mark > 0.02) {
            // the tick pops in from the centre (antd ease-out-back)
            const check_color = tokens.withAlpha(theme.color_text_light, @intFromFloat(255.0 * std.math.clamp(mark, 0, 1)));
            const cx = box.x + box.width * 0.5;
            const cy = box.y + box.height * 0.5;
            const k = mark;
            canvas.drawLine(cx - 3.5 * k, cy, cx - 1 * k, cy + 2.5 * k, check_color, 1.8);
            canvas.drawLine(cx - 1 * k, cy + 2.5 * k, cx + 4 * k, cy - 3 * k, check_color, 1.8);
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
        // antd: unchecked track = colorTextQuaternary (darker on hover),
        // checked = primary (primary_hover on hover); disabled at 65% opacity
        const off = tokens.lerp(theme.color_text_disabled, theme.color_text_secondary, hover * 0.5);
        const on = tokens.lerp(theme.color_primary, theme.color_primary_hover, hover);
        var track_color = tokens.lerp(off, on, check);
        if (!enabled) track_color = tokens.withAlpha(track_color, @intFromFloat(@as(f32, @floatFromInt(tokens.alpha(track_color))) * 0.65));
        canvas.fillRoundRect(track, switch_height * 0.5, track_color);
        // pressing stretches the handle towards the centre (antd 18 → 24px)
        const base: f32 = switch_height - 4;
        const knob_w = base + 5.4 * press;
        const travel = switch_width - 4 - base;
        const left = track.x + 2 + travel * check;
        const knob_x = left - (knob_w - base) * check;
        const knob = geometry.Rect.init(knob_x, track.y + 2, knob_w, base);
        // the handle is always white (antd handleBg)
        canvas.fillRoundRect(knob, base * 0.5, 0xffffffff);
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
}

/// Advances the toggle's transitions (antd): checkbox fill 300ms ease and
/// tick 200ms ease-out-back (in) / 100ms ease-in-back (out); radio dot
/// 300ms ease-in-out-circ; switch 200ms ease-in-out. Returns true while
/// another frame is needed.
pub fn animate(data: *ToggleData, now: i64, theme: Theme) bool {
    const mid: motion.Transition = .{ .duration_ms = @intCast(theme.motion_duration_mid), .easing = .ease_in_out };
    const fast: motion.Transition = .{ .duration_ms = @intCast(theme.motion_duration_fast), .easing = .ease_in_out };
    const target: f32 = if (data.checked) 1 else 0;
    data.hover.retarget(if (data.hovered) 1 else 0, now, mid);
    data.press.retarget(if (data.pressed) 1 else 0, now, fast);
    switch (data.kind) {
        .checkbox => {
            data.check.retarget(target, now, .{ .duration_ms = @intCast(theme.motion_duration_slow), .easing = .ease });
            const mark_tr: motion.Transition = if (data.checked)
                .{ .duration_ms = 200, .easing = .ease_out_back, .delay_ms = 100 }
            else
                .{ .duration_ms = 100, .easing = .ease_in_back };
            data.mark.retarget(target, now, mark_tr);
        },
        .radio => {
            data.check.retarget(target, now, mid);
            data.mark.retarget(target, now, .{ .duration_ms = @intCast(theme.motion_duration_slow), .easing = .ease_in_out_circ });
        },
        .switch_control => {
            data.check.retarget(target, now, mid);
            data.mark.retarget(target, now, mid);
        },
    }
    data.hover_t = data.hover.value(now);
    data.press_t = data.press.value(now);
    data.check_t = data.check.value(now);
    data.mark_t = data.mark.value(now);
    return !(data.hover.done(now) and data.press.done(now) and data.check.done(now) and data.mark.done(now));
}
