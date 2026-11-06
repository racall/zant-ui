const std = @import("std");

pub const Span = extern struct {
    text: [*]const u8,
    length: usize,
    font_size: f32,
    weight: c_int,
    color: u32,
    italic: c_int,
    family: c_int,
    background: u32,
    /// Bit 0: underline, bit 1: line-through.
    decoration: c_int = 0,
    placeholder_width: f32 = 0,
    placeholder_ascent: f32 = 0,
    placeholder_descent: f32 = 0,
};

pub const Rect = extern struct { x: f32, y: f32, width: f32, height: f32 };
extern fn zant_text_layout_placeholder_rects(layout: ?*anyopaque, out: [*]Rect, max: c_int) c_int;

const Metrics = extern struct {
    width: f32,
    height: f32,
    max_intrinsic_width: f32,
    longest_line: f32,
    first_baseline: f32,
    line_count: c_int,
};

extern fn zant_rich_text_layout_create(spans: [*]const Span, count: usize, max_width: f32, line_height: f32) ?*anyopaque;
extern fn zant_text_layout_metrics(layout: ?*anyopaque) Metrics;
extern fn zant_canvas_draw_text_layout(canvas: ?*anyopaque, layout: ?*anyopaque, x: f32, y: f32) void;
extern fn zant_text_layout_destroy(layout: ?*anyopaque) void;
extern fn zant_text_layout_index_at(layout: ?*anyopaque, x: f32, y: f32) usize;

pub const Layout = struct {
    handle: ?*anyopaque,
    width: f32 = 0,
    height: f32 = 0,
    baseline: f32 = 0,

    pub fn create(spans: []const Span, max_width: f32, line_height: f32) !Layout {
        const stats = @import("text.zig").layout_stats;
        const t0 = if (stats.enabled) @import("../platform/platform.zig").monotonicUs() else 0;
        const handle = zant_rich_text_layout_create(spans.ptr, spans.len, max_width, line_height) orelse return error.LayoutFailed;
        if (stats.enabled) stats.add(@import("../platform/platform.zig").monotonicUs() - t0);
        const m = zant_text_layout_metrics(handle);
        return .{ .handle = handle, .width = m.width, .height = m.height, .baseline = m.first_baseline };
    }

    pub fn deinit(self: *Layout) void {
        zant_text_layout_destroy(self.handle);
        self.handle = null;
    }

    /// UTF-8 byte offset (into the concatenated span text) nearest to a point
    /// relative to the layout origin.
    pub fn indexAt(self: Layout, x: f32, y: f32) usize {
        return zant_text_layout_index_at(self.handle, x, y);
    }

    /// Placeholder boxes in span order (relative to the layout origin).
    pub fn placeholderRects(self: Layout, out: []Rect) []Rect {
        const n = zant_text_layout_placeholder_rects(self.handle, out.ptr, @intCast(out.len));
        return out[0..@intCast(@max(n, 0))];
    }

    pub fn paint(self: Layout, canvas: ?*anyopaque, x: f32, y: f32) void {
        zant_canvas_draw_text_layout(canvas, self.handle, x, y);
    }
};
