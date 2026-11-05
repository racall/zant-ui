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
};

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

pub const Layout = struct {
    handle: ?*anyopaque,
    width: f32 = 0,
    height: f32 = 0,
    baseline: f32 = 0,

    pub fn create(spans: []const Span, max_width: f32, line_height: f32) !Layout {
        const handle = zant_rich_text_layout_create(spans.ptr, spans.len, max_width, line_height) orelse return error.LayoutFailed;
        const m = zant_text_layout_metrics(handle);
        return .{ .handle = handle, .width = m.width, .height = m.height, .baseline = m.first_baseline };
    }

    pub fn deinit(self: *Layout) void {
        zant_text_layout_destroy(self.handle);
        self.handle = null;
    }

    pub fn paint(self: Layout, canvas: ?*anyopaque, x: f32, y: f32) void {
        zant_canvas_draw_text_layout(canvas, self.handle, x, y);
    }
};
