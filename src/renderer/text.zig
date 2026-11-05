const std = @import("std");
const geometry = @import("../core/geometry.zig");
const Canvas = @import("canvas.zig").Canvas;

pub const Weight = enum(c_int) {
    thin = 100,
    extra_light = 200,
    light = 300,
    regular = 400,
    medium = 500,
    semibold = 600,
    bold = 700,
    extra_bold = 800,
    black = 900,
};

pub const Family = enum(c_int) {
    sans = 0,
    serif = 1,
    mono = 2,
};

pub const Align = enum(c_int) {
    left = 0,
    center = 1,
    right = 2,
};

pub const Style = struct {
    font_size: f32 = 14,
    weight: Weight = .regular,
    text_align: Align = .left,
    wrap: bool = true,
    line_height: f32 = 0,
    color: u32 = 0xe0000000,
    max_lines: u16 = 0,
    family: Family = .sans,
};

pub const NativeStyle = extern struct {
    font_size: f32,
    weight: c_int,
    text_align: c_int,
    wrap: bool,
    line_height: f32,
    color: u32,
    max_lines: c_int,
    family: c_int,
};

pub const NativeMetrics = extern struct {
    width: f32,
    height: f32,
    max_intrinsic_width: f32,
    longest_line: f32,
    first_baseline: f32,
    line_count: c_int,
};

pub const NativeRect = extern struct {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
};

pub const Metrics = struct {
    width: f32,
    height: f32,
    intrinsic_width: f32,
    longest_line: f32,
    first_baseline: f32,
    line_count: usize,
};

extern fn zant_text_layout_create(text: [*]const u8, length: usize, style: *const NativeStyle, max_width: f32) ?*anyopaque;
extern fn zant_text_layout_destroy(layout: ?*anyopaque) void;
extern fn zant_text_layout_metrics(layout: ?*anyopaque) NativeMetrics;
extern fn zant_text_layout_caret_rect(layout: ?*anyopaque, byte_offset: usize, rect: *NativeRect) bool;
extern fn zant_text_layout_index_at(layout: ?*anyopaque, x: f32, y: f32) usize;
extern fn zant_text_layout_range_rects(layout: ?*anyopaque, start: usize, end: usize, rects: [*]NativeRect, max_rects: c_int) c_int;

pub const TextLayout = struct {
    handle: ?*anyopaque,
    metrics: Metrics,

    pub fn init(text: []const u8, style: Style, max_width: f32) !TextLayout {
        const native_style = NativeStyle{
            .font_size = style.font_size,
            .weight = @backingInt(style.weight),
            .text_align = @backingInt(style.text_align),
            .wrap = style.wrap,
            .line_height = style.line_height,
            .color = style.color,
            .max_lines = @intCast(style.max_lines),
            .family = @intFromEnum(style.family),
        };
        const handle = zant_text_layout_create(text.ptr, text.len, &native_style, max_width) orelse return error.TextLayoutFailed;
        const m = zant_text_layout_metrics(handle);
        return .{
            .handle = handle,
            .metrics = .{
                .width = m.width,
                .height = m.height,
                .intrinsic_width = m.max_intrinsic_width,
                .longest_line = m.longest_line,
                .first_baseline = m.first_baseline,
                .line_count = @intCast(@max(m.line_count, 0)),
            },
        };
    }

    pub fn deinit(self: *TextLayout) void {
        if (self.handle) |h| {
            zant_text_layout_destroy(h);
            self.handle = null;
        }
    }

    pub fn paint(self: *const TextLayout, canvas: Canvas, x: f32, y: f32) void {
        canvas.drawTextLayout(self, x, y);
    }

    pub fn caretRect(self: *const TextLayout, byte_offset: usize) ?geometry.Rect {
        var native: NativeRect = undefined;
        if (!zant_text_layout_caret_rect(self.handle, byte_offset, &native)) return null;
        return geometry.Rect.init(native.x, native.y, native.width, native.height);
    }

    pub fn indexAt(self: *const TextLayout, x: f32, y: f32) usize {
        return zant_text_layout_index_at(self.handle, x, y);
    }

    pub fn rangeRects(self: *const TextLayout, start: usize, end: usize, out: []NativeRect) usize {
        if (out.len == 0) return 0;
        const count = zant_text_layout_range_rects(self.handle, start, end, out.ptr, @intCast(out.len));
        return @intCast(@max(count, 0));
    }
};
