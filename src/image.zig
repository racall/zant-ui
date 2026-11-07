//! Decoded images held in memory, for `ui.image(.{ .image = .. })`.
//!
//! Use it for pixels that do not come from a file: rendered PDF pages,
//! images downloaded or decoded by the app, `data:` URIs, thumbnails.
//! Creating an image is thread-safe (decode on a worker thread, then hand
//! the `Image` to the UI thread and call `ui.postWake`). Nodes showing the
//! image keep their own reference, so `deinit` may run while it is shown.
const std = @import("std");

extern fn zant_image_from_pixels(pixels: [*]const u8, width: c_int, height: c_int, stride: c_int) ?*anyopaque;
extern fn zant_image_from_encoded(data: [*]const u8, size: usize) ?*anyopaque;
extern fn zant_image_retain(image: ?*anyopaque) ?*anyopaque;
extern fn zant_image_thumbnail(data: [*]const u8, size: usize, max_dim: c_int) ?*anyopaque;
extern fn zant_image_encode_png(image: ?*anyopaque, out_size: *usize) ?*anyopaque;
extern fn zant_bytes_free(bytes: ?*anyopaque) void;
extern fn zant_image_destroy(image: ?*anyopaque) void;
extern fn zant_image_width(image: ?*anyopaque) c_int;
extern fn zant_image_height(image: ?*anyopaque) c_int;

var next_id: std.atomic.Value(u64) = .init(1);

pub const Image = struct {
    handle: *anyopaque,
    /// Unique per image (a node compares ids to notice a new image).
    id: u64,
    width: u32,
    height: u32,

    fn wrap(handle: ?*anyopaque) ?Image {
        const h = handle orelse return null;
        return .{
            .handle = h,
            .id = next_id.fetchAdd(1, .monotonic),
            .width = @intCast(@max(0, zant_image_width(h))),
            .height = @intCast(@max(0, zant_image_height(h))),
        };
    }

    /// Copies premultiplied 32-bit pixels in the platform's native order
    /// (`native_order`: RGBA on macOS, BGRA on Windows).
    pub fn fromPixels(pixels: []const u8, width: u32, height: u32, stride: u32) ?Image {
        if (width == 0 or height == 0 or stride < width * 4 or pixels.len < @as(usize, stride) * height) return null;
        return wrap(zant_image_from_pixels(pixels.ptr, @intCast(width), @intCast(height), @intCast(stride)));
    }

    /// Decodes PNG, JPEG, GIF (first frame), BMP or ICO bytes.
    pub fn fromEncoded(bytes: []const u8) ?Image {
        if (bytes.len == 0) return null;
        return wrap(zant_image_from_encoded(bytes.ptr, bytes.len));
    }

    /// Decodes `bytes` scaled down so the larger side is at most `max_dim`
    /// pixels (list thumbnails without keeping full-size pixels).
    pub fn thumbnail(bytes: []const u8, max_dim: u32) ?Image {
        if (bytes.len == 0 or max_dim == 0) return null;
        return wrap(zant_image_thumbnail(bytes.ptr, bytes.len, @intCast(max_dim)));
    }

    /// PNG bytes of the image (allocated with `allocator`).
    pub fn encodePng(self: Image, allocator: std.mem.Allocator) ![]u8 {
        var len: usize = 0;
        const bytes = zant_image_encode_png(self.handle, &len) orelse return error.EncodeFailed;
        defer zant_bytes_free(bytes);
        return allocator.dupe(u8, @as([*]const u8, @ptrCast(bytes))[0..len]);
    }

    /// Another reference to the same pixels (released with its own `deinit`).
    pub fn retain(self: Image) Image {
        const copy = zant_image_retain(self.handle) orelse return self;
        return .{ .handle = copy, .id = self.id, .width = self.width, .height = self.height };
    }

    pub fn deinit(self: Image) void {
        zant_image_destroy(self.handle);
    }
};

pub const PixelOrder = enum { rgba, bgra };

/// Byte order `Image.fromPixels` expects on this platform.
pub const native_order: PixelOrder = if (@import("builtin").os.tag == .windows) .bgra else .rgba;

/// Decodes a `data:image/...;base64,...` URI (null for other URIs).
pub fn fromDataUri(allocator: std.mem.Allocator, uri: []const u8) ?Image {
    if (!std.mem.startsWith(u8, uri, "data:")) return null;
    const comma = std.mem.indexOfScalar(u8, uri, ',') orelse return null;
    const meta = uri[5..comma];
    if (!std.mem.endsWith(u8, meta, ";base64")) return null;
    const payload = uri[comma + 1 ..];
    const decoder = std.base64.standard.Decoder;
    const size = decoder.calcSizeForSlice(payload) catch return null;
    const bytes = allocator.alloc(u8, size) catch return null;
    defer allocator.free(bytes);
    decoder.decode(bytes, payload) catch return null;
    return Image.fromEncoded(bytes);
}
