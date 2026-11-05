const std = @import("std");
const node_mod = @import("../core/node.zig");
const text = @import("../renderer/text.zig");
const tokens = @import("../theme/tokens.zig");
const motion = @import("../motion/motion.zig");
const unicode = @import("../core/unicode.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");

pub const Node = node_mod.Node;
pub const InputData = node_mod.InputData;
pub const Theme = tokens.Theme;

pub const default_width: f32 = 220;
pub const default_height: f32 = 32;
pub const padding_x: f32 = 11;

/// Left edge of the text column (centred when `content_max_width` is set).
pub fn textLeft(data: *const InputData, node: *const Node) f32 {
    const full = @max(0, node.bounds.width - padding_x * 2);
    if (data.content_max_width > 0 and full > data.content_max_width) {
        return node.bounds.x + (node.bounds.width - data.content_max_width) * 0.5;
    }
    return node.bounds.x + padding_x;
}

/// Width of the text column.
pub fn textWidth(data: *const InputData, node: *const Node) f32 {
    const full = @max(0, node.bounds.width - padding_x * 2);
    if (data.content_max_width > 0) return @min(full, data.content_max_width);
    return full;
}
pub const blink_interval_ms: i64 = 530;

pub const Move = enum { left, right, home, end, word_left, word_right, up, down, line_start, line_end, doc_start, doc_end };

pub fn measureWidth(node: *Node) f32 {
    if (node.style.width) |w| return w;
    return default_width;
}

pub fn measureHeight(node: *const Node) f32 {
    if (node.style.height) |h| return h;
    return default_height;
}

pub fn selectionRange(data: *const InputData) struct { start: usize, end: usize } {
    return .{ .start = @min(data.cursor, data.anchor), .end = @max(data.cursor, data.anchor) };
}

pub fn hasSelection(data: *const InputData) bool {
    return data.cursor != data.anchor;
}

pub fn rebuildDisplay(allocator: std.mem.Allocator, data: *InputData) void {
    data.display_text.clearRetainingCapacity();
    const cursor = @min(data.cursor, data.text.items.len);
    if (data.password) {
        // One mask byte per source byte keeps caret/selection offsets aligned.
        var i: usize = 0;
        while (i < cursor) : (i += 1) data.display_text.append(allocator, '*') catch return;
        if (data.has_composition) {
            for (data.composition.items) |_| data.display_text.append(allocator, '*') catch return;
        }
        i = cursor;
        while (i < data.text.items.len) : (i += 1) data.display_text.append(allocator, '*') catch return;
    } else {
        data.display_text.appendSlice(allocator, data.text.items[0..cursor]) catch return;
        if (data.has_composition) {
            data.display_text.appendSlice(allocator, data.composition.items) catch return;
        }
        data.display_text.appendSlice(allocator, data.text.items[cursor..]) catch return;
    }
    data.invalidateLayout();
}

pub fn ensureLayout(data: *InputData) !*text.TextLayout {
    if (!data.layout_dirty and data.layout != null) return &data.layout.?;
    if (data.layout) |*old| old.deinit();
    data.layout = try text.TextLayout.init(data.display_text.items, .{
        .font_size = data.font_size,
        .weight = .regular,
        .color = data.text_color,
        .wrap = data.multiline,
        .text_align = .left,
        .line_height = data.line_height,
    }, if (data.multiline) @max(1, data.available_width) else 1.0e6);
    data.layout_dirty = false;
    return &data.layout.?;
}

fn ensurePlaceholder(data: *InputData, theme: Theme) ?*text.TextLayout {
    if (data.placeholder_layout == null) {
        data.placeholder_layout = text.TextLayout.init(data.placeholder, .{
            .font_size = data.font_size,
            .weight = .regular,
            .color = theme.color_text_disabled,
            .wrap = false,
            .line_height = data.line_height,
        }, 1.0e6) catch return null;
    }
    return &data.placeholder_layout.?;
}

fn replaceRange(allocator: std.mem.Allocator, data: *InputData, start: usize, end: usize, value: []const u8) bool {
    if (start > end or end > data.text.items.len) return false;
    data.text.replaceRange(allocator, start, end - start, value) catch return false;
    const cursor = start + value.len;
    data.cursor = @min(cursor, data.text.items.len);
    data.anchor = data.cursor;
    data.preferred_x = null;
    data.reveal_caret = true;
    data.has_composition = false;
    data.composition.clearRetainingCapacity();
    data.composition_cursor = 0;
    rebuildDisplay(allocator, data);
    return true;
}

pub fn insertText(allocator: std.mem.Allocator, data: *InputData, value: []const u8) bool {
    if (data.read_only or value.len == 0) return false;
    if (data.has_composition) {
        data.has_composition = false;
        data.composition.clearRetainingCapacity();
        data.composition_cursor = 0;
    }
    const range = selectionRange(data);
    return replaceRange(allocator, data, range.start, range.end, value);
}

pub fn deleteSelection(allocator: std.mem.Allocator, data: *InputData) bool {
    if (data.read_only) return false;
    if (!hasSelection(data)) return false;
    const range = selectionRange(data);
    return replaceRange(allocator, data, range.start, range.end, "");
}

pub fn deleteBackward(allocator: std.mem.Allocator, data: *InputData) bool {
    if (data.read_only) return false;
    if (hasSelection(data)) return deleteSelection(allocator, data);
    if (data.cursor == 0) return false;
    const start = unicode.prevGrapheme(data.text.items, data.cursor);
    return replaceRange(allocator, data, start, data.cursor, "");
}

pub fn deleteForward(allocator: std.mem.Allocator, data: *InputData) bool {
    if (data.read_only) return false;
    if (hasSelection(data)) return deleteSelection(allocator, data);
    if (data.cursor >= data.text.items.len) return false;
    const end = unicode.nextGrapheme(data.text.items, data.cursor);
    return replaceRange(allocator, data, data.cursor, end, "");
}

fn moveTo(data: *InputData, target: usize, extend: bool) void {
    const clamped = @min(target, data.text.items.len);
    data.reveal_caret = true;
    if (extend) {
        data.cursor = clamped;
    } else {
        data.cursor = clamped;
        data.anchor = clamped;
    }
}

pub fn moveLeft(data: *InputData, extend: bool) void {
    moveTo(data, unicode.prevGrapheme(data.text.items, data.cursor), extend);
}

pub fn moveRight(data: *InputData, extend: bool) void {
    moveTo(data, unicode.nextGrapheme(data.text.items, data.cursor), extend);
}

pub fn moveHome(data: *InputData, extend: bool) void {
    moveTo(data, 0, extend);
}

pub fn moveEnd(data: *InputData, extend: bool) void {
    moveTo(data, data.text.items.len, extend);
}

pub fn moveWordLeft(data: *InputData, extend: bool) void {
    moveTo(data, unicode.wordStart(data.text.items, data.cursor), extend);
}

pub fn moveWordRight(data: *InputData, extend: bool) void {
    moveTo(data, unicode.wordEnd(data.text.items, data.cursor), extend);
}

/// Moves the caret one visual line up or down, keeping its column.
pub fn moveVertical(data: *InputData, down: bool, extend: bool) void {
    const layout = ensureLayout(data) catch return;
    const r = layout.caretRect(data.cursor) orelse return;
    const x = data.preferred_x orelse r.x;
    data.preferred_x = x;
    const y = if (down) r.y + r.height + 1 else r.y - 1;
    if (y < 0) return moveTo(data, 0, extend);
    if (y >= layout.metrics.height) return moveTo(data, data.text.items.len, extend);
    moveTo(data, @min(layout.indexAt(x, y), data.text.items.len), extend);
}

/// Start (`end == false`) or end of the visual line holding the caret.
pub fn moveLineEdge(data: *InputData, end: bool, extend: bool) void {
    const layout = ensureLayout(data) catch return;
    const r = layout.caretRect(data.cursor) orelse return;
    const y = r.y + r.height * 0.5;
    moveTo(data, @min(layout.indexAt(if (end) 1.0e6 else 0, y), data.text.items.len), extend);
}

pub fn selectAll(data: *InputData) void {
    data.anchor = 0;
    data.cursor = data.text.items.len;
}

pub fn cursorFromPoint(data: *InputData, node: *const Node, x: f32, y: f32) void {
    const layout = ensureLayout(data) catch return;
    const inner_x = textLeft(data, node) - data.scroll_x;
    const inner_y = if (data.multiline) node.bounds.y + 8 else node.bounds.y + (node.bounds.height - layout.metrics.height) * 0.5;
    const byte = layout.indexAt(x - inner_x, y - inner_y);
    data.cursor = @min(byte, data.text.items.len);
    data.anchor = data.cursor;
    data.preferred_x = null;
    data.reveal_caret = true;
}

pub fn beginComposition(
    allocator: std.mem.Allocator,
    data: *InputData,
    marked: []const u8,
    selection_start_utf16: usize,
    selection_end_utf16: usize,
) void {
    _ = selection_start_utf16;
    if (!data.has_composition and hasSelection(data)) {
        _ = deleteSelection(allocator, data);
    }
    data.has_composition = marked.len > 0;
    data.composition.clearRetainingCapacity();
    data.composition.appendSlice(allocator, marked) catch return;
    data.composition_cursor = unicode.utf16ToByte(marked, selection_end_utf16);
    data.anchor = data.cursor;
    data.reveal_caret = true;
    rebuildDisplay(allocator, data);
}

pub fn cancelComposition(allocator: std.mem.Allocator, data: *InputData) void {
    if (!data.has_composition and data.composition.items.len == 0) return;
    data.has_composition = false;
    data.composition.clearRetainingCapacity();
    data.composition_cursor = 0;
    rebuildDisplay(allocator, data);
}

pub fn maxScrollY(data: *InputData, view_height: f32) f32 {
    const layout = ensureLayout(data) catch return 0;
    return @max(0, layout.metrics.height - @max(view_height, 1));
}

pub fn ensureCaretVisible(data: *InputData, available_width: f32, view_height: f32) void {
    const layout = ensureLayout(data) catch return;
    const caret_byte = if (data.has_composition) data.cursor + data.composition_cursor else data.cursor;
    const caret = layout.caretRect(caret_byte) orelse return;
    const width = @max(available_width, 1);
    // Only follow the caret after an edit / caret move / controlled
    // selection; otherwise just clamp, so the user can scroll away freely.
    if (!data.reveal_caret) {
        const max_x = @max(0, layout.metrics.longest_line - width + 2);
        data.scroll_x = @max(0, @min(data.scroll_x, max_x));
        if (data.multiline) data.scroll_y = @max(0, @min(data.scroll_y, maxScrollY(data, view_height)));
        return;
    }
    data.reveal_caret = false;
    if (caret.x - data.scroll_x > width - 2) {
        data.scroll_x = caret.x - width + 2;
    } else if (caret.x - data.scroll_x < 0) {
        data.scroll_x = @max(0, caret.x - 2);
    }
    const max_scroll = @max(0, layout.metrics.longest_line - width + 2);
    data.scroll_x = @max(0, @min(data.scroll_x, max_scroll));
    if (data.multiline) {
        const height = @max(view_height, 1);
        if (caret.y + caret.height - data.scroll_y > height) {
            data.scroll_y = caret.y + caret.height - height;
        } else if (caret.y - data.scroll_y < 0) {
            data.scroll_y = caret.y;
        }
        data.scroll_y = @max(0, @min(data.scroll_y, maxScrollY(data, height)));
    } else {
        data.scroll_y = 0;
    }
}

pub fn caretRect(data: *InputData, node: *const Node) ?geometry.Rect {
    const layout = ensureLayout(data) catch return null;
    const caret_byte = if (data.has_composition) data.cursor + data.composition_cursor else data.cursor;
    const local = layout.caretRect(caret_byte) orelse return null;
    const x = textLeft(data, node) - data.scroll_x + local.x;
    const line_top = (if (data.multiline) node.bounds.y + 8 - data.scroll_y else node.bounds.y + (node.bounds.height - layout.metrics.height) * 0.5) + local.y;
    // Skia reports the whole line box; with a tall line height (e.g. 2.0)
    // that would make the caret twice the text height. Keep the caret at
    // the font's own height and sit it on the baseline: the line box grows
    // proportionally around ascent/descent (~80% / 20%).
    const line_h = @max(local.height, data.font_size);
    const caret_h = @min(line_h, data.font_size * 1.3);
    const baseline = line_top + line_h * 0.8;
    return geometry.Rect.init(x, baseline - caret_h * 0.8, @max(local.width, 1.5), caret_h);
}

pub fn updateBlink(data: *InputData, node: *const Node, now_ms: i64) bool {
    if (!data.focused or !node.enabled) {
        data.blink_on = true;
        data.next_blink_ms = 0;
        return false;
    }
    if (data.next_blink_ms == 0) {
        data.blink_on = true;
        data.next_blink_ms = now_ms + blink_interval_ms;
        return false;
    }
    if (now_ms >= data.next_blink_ms) {
        data.blink_on = !data.blink_on;
        data.next_blink_ms = now_ms + blink_interval_ms;
        return true;
    }
    return false;
}

pub fn paint(node: *Node, canvas: Canvas, theme: Theme) void {
    const data = &node.data.input;
    const rect = node.bounds;
    if (!node.visible) return;
    const enabled = node.enabled;
    const desired_text_color = if (enabled) theme.color_text else theme.color_text_disabled;
    if (data.text_color != desired_text_color) {
        data.text_color = desired_text_color;
        data.invalidateLayout();
    }
    const bg = node.visual.background orelse if (enabled) theme.color_bg_container else theme.color_fill_secondary;
    // antd: border eases to primary-hover on hover and primary on focus,
    // with a soft outline ring fading in
    const hover_t = if (enabled) data.hover_t else 0;
    const focus_t = if (enabled) data.focus_t else 0;
    const default_border = if (!enabled)
        theme.color_border
    else
        tokens.lerp(tokens.lerp(theme.color_border, theme.color_primary_hover, hover_t), theme.color_primary, focus_t);
    const border = node.visual.border_color orelse default_border;

    if (focus_t > 0.01 and data.focus_ring) {
        const shadow = geometry.Rect.init(rect.x - 3, rect.y - 3, rect.width + 6, rect.height + 6);
        canvas.fillRoundRect(shadow, node.visual.radius + 3, tokens.withAlpha(theme.color_primary, @intFromFloat(0x18 * focus_t)));
    }
    canvas.fillRoundRect(rect, node.visual.radius, bg);
    canvas.strokeRoundRect(
        geometry.Rect.init(rect.x + 0.5, rect.y + 0.5, @max(0, rect.width - 1), @max(0, rect.height - 1)),
        node.visual.radius,
        border,
        1.0 + 0.5 * focus_t,
    );

    const new_width = textWidth(data, node);
    if (data.multiline and @abs(new_width - data.available_width) > 0.5) data.invalidateLayout();
    data.available_width = new_width;
    const inner = geometry.Rect.init(
        textLeft(data, node),
        rect.y + 1,
        data.available_width,
        @max(0, rect.height - 2),
    );
    canvas.save();
    canvas.clipRect(inner);

    const layout = ensureLayout(data) catch {
        canvas.restore();
        return;
    };
    ensureCaretVisible(data, inner.width, inner.height);
    const text_y = if (data.multiline) rect.y + 8 - data.scroll_y else rect.y + (rect.height - layout.metrics.height) * 0.5;
    const origin_x = textLeft(data, node) - data.scroll_x;

    if (data.text.items.len == 0 and !data.has_composition) {
        if (ensurePlaceholder(data, theme)) |placeholder| {
            placeholder.paint(canvas, textLeft(data, node), text_y);
        }
    } else {
        if (!data.has_composition and hasSelection(data)) {
            const range = selectionRange(data);
            // only the visible part of the selection (long selections in the
            // editor would otherwise exceed the rect buffer)
            const vis_start = if (data.multiline) layout.indexAt(0, data.scroll_y - 2) else 0;
            const vis_end = if (data.multiline) layout.indexAt(1.0e6, data.scroll_y + inner.height + 2) else data.text.items.len;
            const sel_start = @max(range.start, @min(vis_start, range.end));
            const sel_end = @min(range.end, @max(vis_end, sel_start));
            var rects: [256]text.NativeRect = undefined;
            const count = if (sel_end > sel_start) layout.rangeRects(sel_start, sel_end, &rects) else 0;
            for (rects[0..count]) |r| {
                if (r.width <= 0 or r.height <= 0) continue;
                canvas.fillRect(
                    geometry.Rect.init(origin_x + r.x, text_y + r.y, r.width, r.height),
                    tokens.withAlpha(theme.color_primary, 0x2e),
                );
            }
        }
        layout.paint(canvas, origin_x, text_y);

        if (data.has_composition and data.composition.items.len > 0) {
            var rects: [256]text.NativeRect = undefined;
            const start = data.cursor;
            const end = data.cursor + data.composition.items.len;
            const count = layout.rangeRects(start, end, &rects);
            for (rects[0..count]) |r| {
                canvas.drawLine(
                    origin_x + r.x,
                    text_y + r.y + r.height - 1,
                    origin_x + r.x + r.width,
                    text_y + r.y + r.height - 1,
                    theme.color_text,
                    1,
                );
            }
        }
    }

    if (data.focused and enabled and data.blink_on) {
        if (caretRect(data, node)) |caret| {
            canvas.fillRect(geometry.Rect.init(caret.x, caret.y, 1.5, caret.height), theme.color_text);
        }
    }

    canvas.restore();

    // overlay scroll indicator on the input's right edge
    if (data.multiline) {
        const view = @max(1, rect.height - 16);
        const max_scroll = maxScrollY(data, view);
        if (max_scroll > 0.5) {
            const content = view + max_scroll;
            const track = rect.height - 8;
            const thumb = @max(28, track * view / content);
            const y = rect.y + 4 + (track - thumb) * (data.scroll_y / max_scroll);
            canvas.fillRoundRect(geometry.Rect.init(rect.right() - 7, y, 3.5, thumb), 1.75, tokens.withAlpha(theme.color_text, 0x40));
        }
    }
}

/// Border / focus-ring transitions (antd 200ms ease-in-out). Returns true
/// while another frame is needed. The caret blink is driven separately.
pub fn animate(data: *InputData, now: i64, theme: Theme) bool {
    const mid: motion.Transition = .{ .duration_ms = @intCast(theme.motion_duration_mid), .easing = .ease_in_out };
    data.hover.retarget(if (data.hovered) 1 else 0, now, mid);
    data.focus_anim.retarget(if (data.focused) 1 else 0, now, mid);
    data.hover_t = data.hover.value(now);
    data.focus_t = data.focus_anim.value(now);
    return !(data.hover.done(now) and data.focus_anim.done(now));
}
