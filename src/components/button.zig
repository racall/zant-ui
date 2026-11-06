const std = @import("std");
const node_mod = @import("../core/node.zig");
const tokens = @import("../theme/tokens.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");
const icons = @import("../core/icons.zig");
const motion = @import("../motion/motion.zig");

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
    if (button_type == .text or button_type == .link or button_type == .plain or isCaption(button_type)) return 0;
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

fn isCaption(button_type: node_mod.ButtonType) bool {
    return button_type == .caption or button_type == .caption_close;
}

/// Windows 11 caption buttons: close hover #C42B1C, pressed at 90 %.
const caption_close_hover: u32 = 0xffc42b1c;
const caption_close_pressed: u32 = 0xe6c42b1c;

fn textColor(data: *const node_mod.ButtonData, enabled: bool, theme: Theme) u32 {
    if (!enabled) return theme.color_text_disabled;
    return switch (data.button_type) {
        .primary => theme.color_text_light,
        .default => theme.color_text,
        .text => theme.color_text,
        .link => theme.color_primary,
        .plain, .caption, .caption_close => theme.color_text,
    };
}

pub fn measureWidth(node: *Node, theme: Theme, max_width: f32) f32 {
    _ = max_width;
    const data = &node.data.button;
    if (node.style.width) |w| return w;
    // same colour as paint, so measuring does not re-shape the label
    const color = if (node.enabled) data.text_color orelse textColor(data, true, theme) else textColor(data, false, theme);
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
        // antd: disabled text / link buttons have no fill or border
        if (data.button_type == .text or data.button_type == .link or data.button_type == .plain or isCaption(data.button_type)) {
            const clear = tokens.withAlpha(theme.color_fill_secondary, 0);
            return .{ .bg = clear, .hover_bg = clear, .active_bg = clear, .border = 0, .hover_border = 0, .active_border = 0 };
        }
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
        // antd: colorBgTextHover = fillSecondary, colorBgTextActive = fill
        .text => .{
            .bg = tokens.withAlpha(theme.color_fill_secondary, 0),
            .hover_bg = theme.color_fill_secondary,
            .active_bg = theme.color_fill,
            .border = 0x00000000,
            .hover_border = 0x00000000,
            .active_border = 0x00000000,
        },
        // Windows caption buttons: a faint fill of the text colour
        .caption => .{
            .bg = tokens.withAlpha(theme.color_text, 0),
            .hover_bg = tokens.withAlpha(theme.color_text, 0x12),
            .active_bg = tokens.withAlpha(theme.color_text, 0x0c),
            .border = 0x00000000,
            .hover_border = 0x00000000,
            .active_border = 0x00000000,
        },
        .caption_close => .{
            .bg = tokens.withAlpha(caption_close_hover, 0),
            .hover_bg = caption_close_hover,
            .active_bg = caption_close_pressed,
            .border = 0x00000000,
            .hover_border = 0x00000000,
            .active_border = 0x00000000,
        },
        .link, .plain => .{
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
    // an empty plain button (hit area, dismiss layer) paints only an
    // explicit background
    if (data.button_type == .plain and data.icon == .none and data.label.len == 0) {
        if (node.visual.background) |bg| {
            if (tokens.alpha(bg) > 0) canvas.fillRoundRect(node.bounds, node.visual.radius, bg);
        }
        return;
    }
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

    // disabled buttons always use the disabled text colour (antd)
    var color = if (enabled) data.text_color orelse textColor(data, true, theme) else textColor(data, false, theme);
    // the close glyph turns white on its red fill
    if (data.button_type == .caption_close) color = tokens.lerp(color, 0xffffffff, @max(hover_t, press_t));
    if (data.icon != .none) {
        paintIcon(node, canvas, color);
    } else if (data.ensureLayout(color)) |layout| {
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
    if (data.wave_t < 1) paintWave(node, canvas, theme);
}

/// antd click wave: a primary-coloured ring spreading 6px and fading out.
fn paintWave(node: *const Node, canvas: Canvas, theme: Theme) void {
    const ms = node.data.button.wave_t * 600;
    const spread = 6 * motion.easing.apply(.ease_out_circ, @min(1, ms / 400));
    const fade = 0.2 * (1 - motion.easing.apply(.ease_out_circ, ms / 600));
    if (spread < 0.1 or fade < 0.004) return;
    const r = node.bounds;
    const half = spread * 0.5;
    const color = tokens.withAlpha(theme.color_primary, @intFromFloat(255.0 * fade));
    canvas.strokeRoundRect(geometry.Rect.init(r.x - half, r.y - half, r.width + spread, r.height + spread), node.visual.radius + half, color, spread);
}

/// Advances hover / press / wave transitions (antd: hover 200ms, active
/// 100ms, ease-in-out). Returns true while another frame is needed.
pub fn animate(data: *node_mod.ButtonData, now: i64, theme: Theme) bool {
    const mid: motion.Transition = .{ .duration_ms = @intCast(theme.motion_duration_mid), .easing = .ease_in_out };
    const fast: motion.Transition = .{ .duration_ms = @intCast(theme.motion_duration_fast), .easing = .ease_in_out };
    data.hover.retarget(if (data.hovered) 1 else 0, now, mid);
    data.press.retarget(if (data.pressed) 1 else 0, now, fast);
    data.hover_t = data.hover.value(now);
    data.press_t = data.press.value(now);
    data.wave_t = data.wave.value(now);
    return !(data.hover.done(now) and data.press.done(now) and data.wave.done(now));
}

/// Starts the click wave (primary / default buttons only).
pub fn startWave(data: *node_mod.ButtonData, now: i64) void {
    if (data.button_type != .primary and data.button_type != .default) return;
    data.wave = motion.Animated.start(0, 1, now, .{ .duration_ms = 600, .easing = .linear });
}

pub fn setEnabled(node: *Node, enabled: bool, theme: Theme) void {
    if (node.enabled == enabled) return;
    node.enabled = enabled;
    const data = &node.data.button;
    if (!enabled) {
        data.hovered = false;
        data.pressed = false;
        data.focused = false;
        data.hover = .at(0);
        data.press = .at(0);
        data.wave = .at(1);
        data.hover_t = 0;
        data.press_t = 0;
        data.wave_t = 1;
    }
    const color = textColor(data, enabled, theme);
    data.ensureLayout(color) catch {};
}

fn paintIcon(node: *Node, canvas: Canvas, color: u32) void {
    const rect = node.bounds;
    const data = &node.data.button;
    const size = @min(data.icon_size, @min(rect.width, rect.height));
    // caption glyphs: 1px hairlines on the pixel grid, like Segoe Fluent Icons
    const stroke: ?f32 = if (isCaption(data.button_type)) 1 else null;
    const cx = rect.x + rect.width * 0.5;
    const cy = rect.y + rect.height * 0.5;
    if (stroke != null) {
        icons.paint(canvas.ptr, data.icon, @floor(cx) + 0.5, @floor(cy) + 0.5, size, color, stroke);
    } else {
        icons.paint(canvas.ptr, data.icon, cx, cy, size, color, null);
    }
}
