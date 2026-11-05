const geometry = @import("../core/geometry.zig");

pub const Canvas = struct {
    ptr: ?*anyopaque,

    pub fn fromOpaque(ptr: ?*anyopaque) Canvas {
        return .{ .ptr = ptr };
    }

    extern fn zant_canvas_save(canvas: ?*anyopaque) void;
    extern fn zant_canvas_restore(canvas: ?*anyopaque) void;
    extern fn zant_canvas_translate(canvas: ?*anyopaque, x: f32, y: f32) void;
    extern fn zant_canvas_scale(canvas: ?*anyopaque, sx: f32, sy: f32) void;
    extern fn zant_canvas_clip_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32) void;
    extern fn zant_canvas_clear(canvas: ?*anyopaque, color: u32) void;
    extern fn zant_canvas_fill_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, color: u32) void;
    extern fn zant_canvas_fill_round_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, radius: f32, color: u32) void;
    extern fn zant_canvas_stroke_round_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, radius: f32, color: u32, stroke_width: f32) void;
    extern fn zant_canvas_draw_line(canvas: ?*anyopaque, x0: f32, y0: f32, x1: f32, y1: f32, color: u32, stroke_width: f32) void;
    extern fn zant_canvas_draw_text_layout(canvas: ?*anyopaque, layout: ?*anyopaque, x: f32, y: f32) void;

    pub fn save(self: Canvas) void {
        zant_canvas_save(self.ptr);
    }

    pub fn restore(self: Canvas) void {
        zant_canvas_restore(self.ptr);
    }

    pub fn translate(self: Canvas, x: f32, y: f32) void {
        zant_canvas_translate(self.ptr, x, y);
    }

    pub fn scale(self: Canvas, sx: f32, sy: f32) void {
        zant_canvas_scale(self.ptr, sx, sy);
    }

    pub fn clipRect(self: Canvas, rect: geometry.Rect) void {
        zant_canvas_clip_rect(self.ptr, rect.x, rect.y, rect.width, rect.height);
    }

    pub fn clear(self: Canvas, color: u32) void {
        zant_canvas_clear(self.ptr, color);
    }

    pub fn fillRect(self: Canvas, rect: geometry.Rect, color: u32) void {
        zant_canvas_fill_rect(self.ptr, rect.x, rect.y, rect.width, rect.height, color);
    }

    pub fn fillRoundRect(self: Canvas, rect: geometry.Rect, radius: f32, color: u32) void {
        zant_canvas_fill_round_rect(self.ptr, rect.x, rect.y, rect.width, rect.height, radius, color);
    }

    pub fn strokeRoundRect(self: Canvas, rect: geometry.Rect, radius: f32, color: u32, width: f32) void {
        zant_canvas_stroke_round_rect(self.ptr, rect.x, rect.y, rect.width, rect.height, radius, color, width);
    }

    pub fn drawLine(self: Canvas, x0: f32, y0: f32, x1: f32, y1: f32, color: u32, width: f32) void {
        zant_canvas_draw_line(self.ptr, x0, y0, x1, y1, color, width);
    }

    pub fn drawTextLayout(self: Canvas, layout: anytype, x: f32, y: f32) void {
        zant_canvas_draw_text_layout(self.ptr, layout.handle, x, y);
    }
};
