const std = @import("std");
const Canvas = @import("canvas.zig").Canvas;

pub const Renderer = struct {
    handle: ?*anyopaque,
    width: c_int,
    height: c_int,

    extern fn zant_renderer_create_raster(width: c_int, height: c_int) ?*anyopaque;
    extern fn zant_renderer_destroy(renderer: ?*anyopaque) void;
    extern fn zant_renderer_resize(renderer: ?*anyopaque, width: c_int, height: c_int) bool;
    extern fn zant_renderer_begin(renderer: ?*anyopaque, scale: f32) ?*anyopaque;
    extern fn zant_renderer_end(renderer: ?*anyopaque) void;
    extern fn zant_renderer_write_png(renderer: ?*anyopaque, path: [*:0]const u8) bool;
    extern fn zant_renderer_backend_name(renderer: ?*anyopaque) [*:0]const u8;
    extern fn zant_renderer_is_gpu(renderer: ?*anyopaque) bool;
    extern fn zant_renderer_pixels(renderer: ?*anyopaque, width: *c_int, height: *c_int, stride: *c_int) ?[*]u8;

    pub fn createRaster(width: u32, height: u32) !Renderer {
        const handle = zant_renderer_create_raster(@intCast(width), @intCast(height)) orelse
            return error.RendererCreationFailed;
        return .{ .handle = handle, .width = @intCast(width), .height = @intCast(height) };
    }

    pub fn deinit(self: *Renderer) void {
        if (self.handle) |handle| {
            zant_renderer_destroy(handle);
            self.handle = null;
        }
    }

    pub fn resize(self: *Renderer, width: u32, height: u32) bool {
        const result = zant_renderer_resize(self.handle, @intCast(width), @intCast(height));
        if (result) {
            self.width = @intCast(width);
            self.height = @intCast(height);
        }
        return result;
    }

    pub fn begin(self: *Renderer, scale: f32) ?Canvas {
        return Canvas.fromOpaque(zant_renderer_begin(self.handle, scale));
    }

    pub fn end(self: *Renderer) void {
        zant_renderer_end(self.handle);
    }

    pub fn writePng(self: *const Renderer, path: [:0]const u8) bool {
        return zant_renderer_write_png(self.handle, path.ptr);
    }

    pub fn backendName(self: *const Renderer) []const u8 {
        return std.mem.span(zant_renderer_backend_name(self.handle));
    }

    pub fn isGpu(self: *const Renderer) bool {
        return zant_renderer_is_gpu(self.handle);
    }

    /// Pixel at (x, y) of a raster renderer as 0xAARRGGBB (premultiplied);
    /// null outside the surface or without CPU pixels (tests).
    pub fn pixelAt(self: *const Renderer, x: usize, y: usize) ?u32 {
        var w: c_int = 0;
        var h: c_int = 0;
        var stride: c_int = 0;
        const base = zant_renderer_pixels(self.handle, &w, &h, &stride) orelse return null;
        if (x >= @as(usize, @intCast(w)) or y >= @as(usize, @intCast(h))) return null;
        const p = base + y * @as(usize, @intCast(stride)) + x * 4;
        return (@as(u32, p[3]) << 24) | (@as(u32, p[0]) << 16) | (@as(u32, p[1]) << 8) | p[2];
    }
};
