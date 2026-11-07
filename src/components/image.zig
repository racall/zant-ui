const std = @import("std");
const node_mod = @import("../core/node.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const geometry = @import("../core/geometry.zig");
const tokens = @import("../theme/tokens.zig");

pub const Node = node_mod.Node;
pub const ImageData = node_mod.ImageData;
pub const ImageFit = node_mod.ImageFit;

extern fn zant_image_load_file(path: [*:0]const u8) ?*anyopaque;
extern fn zant_image_destroy(image: ?*anyopaque) void;
extern fn zant_image_width(image: ?*anyopaque) c_int;
extern fn zant_image_height(image: ?*anyopaque) c_int;
extern fn zant_canvas_draw_image_rect(canvas: ?*anyopaque, image: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, radius: f32, fit: c_int) void;

pub fn load(data: *ImageData) void {
    if (data.handle) |old| zant_image_destroy(old);
    data.handle = null;
    data.intrinsic_width = 0;
    data.intrinsic_height = 0;
    const path = std.heap.c_allocator.allocSentinel(u8, data.path.len, 0) catch return;
    defer std.heap.c_allocator.free(path);
    @memcpy(path[0..data.path.len], data.path);
    data.handle = zant_image_load_file(path.ptr);
    if (data.handle != null) {
        data.intrinsic_width = zant_image_width(data.handle);
        data.intrinsic_height = zant_image_height(data.handle);
    }
}

extern fn zant_image_retain(image: ?*anyopaque) ?*anyopaque;

/// Shows an in-memory image (keeps its own reference to the pixels).
pub fn setSource(data: *ImageData, source: ?*anyopaque, id: u64) void {
    if (data.handle) |old| zant_image_destroy(old);
    data.handle = null;
    data.intrinsic_width = 0;
    data.intrinsic_height = 0;
    data.source_id = id;
    if (source == null) return;
    data.handle = zant_image_retain(source);
    if (data.handle != null) {
        data.intrinsic_width = zant_image_width(data.handle);
        data.intrinsic_height = zant_image_height(data.handle);
    }
}

pub fn destroy(data: *ImageData) void {
    if (data.handle) |handle| {
        zant_image_destroy(handle);
        data.handle = null;
    }
}

pub fn measureWidth(node: *const Node) f32 {
    if (node.style.width) |w| return w;
    return if (node.data.image.intrinsic_width > 0) @floatFromInt(node.data.image.intrinsic_width) else 160;
}

pub fn measureHeight(node: *const Node, width: f32) f32 {
    if (node.style.height) |h| return h;
    const data = &node.data.image;
    if (data.intrinsic_width > 0 and data.intrinsic_height > 0) {
        return width * @as(f32, @floatFromInt(data.intrinsic_height)) / @as(f32, @floatFromInt(data.intrinsic_width));
    }
    return 120;
}

pub fn paint(node: *const Node, canvas: Canvas, theme: tokens.Theme) void {
    const data = &node.data.image;
    if (data.handle == null) {
        // placeholder until the image loads (or when it cannot be decoded)
        canvas.fillRoundRect(node.bounds, node.visual.radius, theme.color_fill_secondary);
        return;
    }
    zant_canvas_draw_image_rect(canvas.ptr, data.handle, node.bounds.x, node.bounds.y, node.bounds.width, node.bounds.height, node.visual.radius, @backingInt(data.fit));
}
