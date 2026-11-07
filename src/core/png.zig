//! Minimal PNG reader / writer and an ICO builder for app icons (no external
//! tools): decodes 8-bit greyscale / RGB / palette / grey+alpha / RGBA PNGs
//! (non-interlaced), resizes with an area-average filter in premultiplied
//! alpha, writes RGBA PNGs and .ico files whose entries are PNGs (Windows
//! Vista+).
const std = @import("std");
const flate = std.compress.flate;

pub const Error = error{ NotPng, Unsupported, Corrupt } || std.mem.Allocator.Error;

pub const Image = struct {
    width: u32,
    height: u32,
    /// straight (not premultiplied) RGBA, row-major
    rgba: []u8,

    pub fn deinit(self: Image, allocator: std.mem.Allocator) void {
        allocator.free(self.rgba);
    }
};

const signature = "\x89PNG\r\n\x1a\n";

pub const Info = struct { width: u32, height: u32 };

/// Width and height from the IHDR chunk (no decoding).
pub fn info(data: []const u8) Error!Info {
    if (data.len < 33 or !std.mem.eql(u8, data[0..8], signature)) return error.NotPng;
    if (!std.mem.eql(u8, data[12..16], "IHDR")) return error.Corrupt;
    return .{ .width = std.mem.readInt(u32, data[16..20], .big), .height = std.mem.readInt(u32, data[20..24], .big) };
}

pub fn decode(allocator: std.mem.Allocator, data: []const u8) Error!Image {
    const head = try info(data);
    const depth = data[24];
    const color = data[25];
    if (data[28] != 0) return error.Unsupported; // interlaced
    if (depth != 8) return error.Unsupported;
    const channels: usize = switch (color) {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        else => return error.Unsupported,
    };
    if (head.width == 0 or head.height == 0 or head.width > 8192 or head.height > 8192) return error.Unsupported;

    var idat: std.ArrayListUnmanaged(u8) = .empty;
    defer idat.deinit(allocator);
    var palette: []const u8 = &.{};
    var trns: []const u8 = &.{};
    var pos: usize = 8;
    while (pos + 12 <= data.len) {
        const len = std.mem.readInt(u32, data[pos..][0..4], .big);
        const kind = data[pos + 4 .. pos + 8];
        if (pos + 12 + len > data.len) return error.Corrupt;
        const body = data[pos + 8 .. pos + 8 + len];
        if (std.mem.eql(u8, kind, "IDAT")) try idat.appendSlice(allocator, body);
        if (std.mem.eql(u8, kind, "PLTE")) palette = body;
        if (std.mem.eql(u8, kind, "tRNS")) trns = body;
        if (std.mem.eql(u8, kind, "IEND")) break;
        pos += 12 + len;
    }

    const stride = head.width * channels;
    const raw_len = (stride + 1) * head.height;
    const raw = try allocator.alloc(u8, raw_len);
    defer allocator.free(raw);
    var in: std.Io.Reader = .fixed(idat.items);
    var window: [flate.max_window_len]u8 = undefined;
    var dec: flate.Decompress = .init(&in, .zlib, &window);
    dec.reader.readSliceAll(raw) catch return error.Corrupt;

    // undo the per-row filters in place
    const bpp = channels;
    var prev: []u8 = &.{};
    var y: usize = 0;
    while (y < head.height) : (y += 1) {
        const row = raw[y * (stride + 1) + 1 ..][0..stride];
        const filter = raw[y * (stride + 1)];
        for (row, 0..) |*b, x| {
            const a: u8 = if (x >= bpp) row[x - bpp] else 0;
            const up: u8 = if (prev.len > 0) prev[x] else 0;
            const c: u8 = if (prev.len > 0 and x >= bpp) prev[x - bpp] else 0;
            b.* +%= switch (filter) {
                0 => 0,
                1 => a,
                2 => up,
                3 => @intCast((@as(u16, a) + up) / 2),
                4 => paeth(a, up, c),
                else => return error.Corrupt,
            };
        }
        prev = row;
    }

    const out = try allocator.alloc(u8, @as(usize, head.width) * head.height * 4);
    errdefer allocator.free(out);
    y = 0;
    while (y < head.height) : (y += 1) {
        const row = raw[y * (stride + 1) + 1 ..][0..stride];
        var x: usize = 0;
        while (x < head.width) : (x += 1) {
            const o = out[(y * head.width + x) * 4 ..][0..4];
            const p = row[x * channels ..];
            switch (color) {
                0 => o.* = .{ p[0], p[0], p[0], 255 },
                2 => o.* = .{ p[0], p[1], p[2], 255 },
                4 => o.* = .{ p[0], p[0], p[0], p[1] },
                6 => o.* = .{ p[0], p[1], p[2], p[3] },
                3 => {
                    const i: usize = p[0];
                    if (i * 3 + 2 >= palette.len) return error.Corrupt;
                    o.* = .{ palette[i * 3], palette[i * 3 + 1], palette[i * 3 + 2], if (i < trns.len) trns[i] else 255 };
                },
                else => unreachable,
            }
        }
    }
    return .{ .width = head.width, .height = head.height, .rgba = out };
}

fn paeth(a: u8, b: u8, c: u8) u8 {
    const p = @as(i16, a) + b - c;
    const pa = @abs(p - a);
    const pb = @abs(p - b);
    const pc = @abs(p - c);
    if (pa <= pb and pa <= pc) return a;
    if (pb <= pc) return b;
    return c;
}

/// Area-average resize (each output pixel averages the source area it
/// covers), in premultiplied alpha so transparent edges do not darken.
pub fn resize(allocator: std.mem.Allocator, src: Image, width: u32, height: u32) Error!Image {
    const out = try allocator.alloc(u8, @as(usize, width) * height * 4);
    const sx = @as(f64, @floatFromInt(src.width)) / @as(f64, @floatFromInt(width));
    const sy = @as(f64, @floatFromInt(src.height)) / @as(f64, @floatFromInt(height));
    var oy: u32 = 0;
    while (oy < height) : (oy += 1) {
        const y0 = @as(f64, @floatFromInt(oy)) * sy;
        const y1 = y0 + sy;
        var ox: u32 = 0;
        while (ox < width) : (ox += 1) {
            const x0 = @as(f64, @floatFromInt(ox)) * sx;
            const x1 = x0 + sx;
            var acc = [4]f64{ 0, 0, 0, 0 };
            var total: f64 = 0;
            var y: u32 = @intFromFloat(@floor(y0));
            while (@as(f64, @floatFromInt(y)) < y1 and y < src.height) : (y += 1) {
                const wy = @min(y1, @as(f64, @floatFromInt(y + 1))) - @max(y0, @as(f64, @floatFromInt(y)));
                var x: u32 = @intFromFloat(@floor(x0));
                while (@as(f64, @floatFromInt(x)) < x1 and x < src.width) : (x += 1) {
                    const wx = @min(x1, @as(f64, @floatFromInt(x + 1))) - @max(x0, @as(f64, @floatFromInt(x)));
                    const w = wx * wy;
                    const p = src.rgba[(@as(usize, y) * src.width + x) * 4 ..][0..4];
                    const a = @as(f64, @floatFromInt(p[3])) / 255.0;
                    acc[0] += @as(f64, @floatFromInt(p[0])) * a * w;
                    acc[1] += @as(f64, @floatFromInt(p[1])) * a * w;
                    acc[2] += @as(f64, @floatFromInt(p[2])) * a * w;
                    acc[3] += a * w;
                    total += w;
                }
            }
            const o = out[(@as(usize, oy) * width + ox) * 4 ..][0..4];
            const alpha = if (total > 0) acc[3] / total else 0;
            if (alpha <= 0) {
                o.* = .{ 0, 0, 0, 0 };
                continue;
            }
            for (0..3) |c| o[c] = @intFromFloat(@round(std.math.clamp(acc[c] / total / alpha, 0, 255)));
            o[3] = @intFromFloat(@round(std.math.clamp(alpha * 255.0, 0, 255)));
        }
    }
    return .{ .width = width, .height = height, .rgba = out };
}

/// RGBA PNG (filter 0 rows, zlib level 6).
pub fn encode(allocator: std.mem.Allocator, img: Image) Error![]u8 {
    const stride = @as(usize, img.width) * 4;
    const raw = try allocator.alloc(u8, (stride + 1) * img.height);
    defer allocator.free(raw);
    for (0..img.height) |y| {
        raw[y * (stride + 1)] = 0;
        @memcpy(raw[y * (stride + 1) + 1 ..][0..stride], img.rgba[y * stride ..][0..stride]);
    }
    var z = try std.Io.Writer.Allocating.initCapacity(allocator, 4096);
    defer z.deinit();
    {
        var window: [flate.max_window_len]u8 = undefined;
        var comp = flate.Compress.init(&z.writer, &window, .zlib, .default) catch return error.OutOfMemory;
        comp.writer.writeAll(raw) catch return error.OutOfMemory;
        comp.finish() catch return error.OutOfMemory;
    }
    var out: std.ArrayListUnmanaged(u8) = .empty;
    errdefer out.deinit(allocator);
    try out.appendSlice(allocator, signature);
    var ihdr: [13]u8 = undefined;
    std.mem.writeInt(u32, ihdr[0..4], img.width, .big);
    std.mem.writeInt(u32, ihdr[4..8], img.height, .big);
    ihdr[8..13].* = .{ 8, 6, 0, 0, 0 };
    try chunk(allocator, &out, "IHDR", &ihdr);
    try chunk(allocator, &out, "IDAT", z.written());
    try chunk(allocator, &out, "IEND", "");
    return out.toOwnedSlice(allocator);
}

fn chunk(allocator: std.mem.Allocator, out: *std.ArrayListUnmanaged(u8), kind: *const [4]u8, body: []const u8) !void {
    var len: [4]u8 = undefined;
    std.mem.writeInt(u32, &len, @intCast(body.len), .big);
    try out.appendSlice(allocator, &len);
    try out.appendSlice(allocator, kind);
    try out.appendSlice(allocator, body);
    var crc = std.hash.Crc32.init();
    crc.update(kind);
    crc.update(body);
    var c: [4]u8 = undefined;
    std.mem.writeInt(u32, &c, crc.final(), .big);
    try out.appendSlice(allocator, &c);
}

/// Sizes in a Windows app .ico (Microsoft's app icon guidance: 16–256, with
/// the in-between sizes used at 125–200% display scaling).
pub const ico_sizes = [_]u32{ 16, 20, 24, 32, 40, 48, 64, 256 };

/// .ico with one PNG entry per size, resized from `src`.
pub fn buildIco(allocator: std.mem.Allocator, src: Image) Error![]u8 {
    var entries: [ico_sizes.len][]u8 = undefined;
    var made: usize = 0;
    defer for (entries[0..made]) |e| allocator.free(e);
    for (ico_sizes, 0..) |size, i| {
        const img = if (src.width == size and src.height == size) src else try resize(allocator, src, size, size);
        defer if (img.rgba.ptr != src.rgba.ptr) img.deinit(allocator);
        entries[i] = try encode(allocator, img);
        made += 1;
    }
    var out: std.ArrayListUnmanaged(u8) = .empty;
    errdefer out.deinit(allocator);
    var header: [6]u8 = .{ 0, 0, 1, 0, 0, 0 };
    std.mem.writeInt(u16, header[4..6], ico_sizes.len, .little);
    try out.appendSlice(allocator, &header);
    var offset: u32 = 6 + 16 * ico_sizes.len;
    for (ico_sizes, entries) |size, png| {
        var dir: [16]u8 = @splat(0);
        dir[0] = if (size >= 256) 0 else @intCast(size);
        dir[1] = dir[0];
        std.mem.writeInt(u16, dir[4..6], 1, .little); // planes
        std.mem.writeInt(u16, dir[6..8], 32, .little); // bits per pixel
        std.mem.writeInt(u32, dir[8..12], @intCast(png.len), .little);
        std.mem.writeInt(u32, dir[12..16], offset, .little);
        try out.appendSlice(allocator, &dir);
        offset += @intCast(png.len);
    }
    for (entries) |png| try out.appendSlice(allocator, png);
    return out.toOwnedSlice(allocator);
}

test "png round trip, resize and ico" {
    const gpa = std.testing.allocator;
    var px: [8 * 8 * 4]u8 = undefined;
    for (0..64) |i| px[i * 4 ..][0..4].* = .{ @intCast(i * 4), 128, 255, if (i % 2 == 0) 255 else 0 };
    const img: Image = .{ .width = 8, .height = 8, .rgba = &px };
    const png = try encode(gpa, img);
    defer gpa.free(png);
    const back = try decode(gpa, png);
    defer back.deinit(gpa);
    try std.testing.expectEqualSlices(u8, &px, back.rgba);
    const small = try resize(gpa, back, 4, 4);
    defer small.deinit(gpa);
    try std.testing.expectEqual(@as(u8, 128), small.rgba[3]); // half the 2×2 block is opaque
    const ico = try buildIco(gpa, back);
    defer gpa.free(ico);
    try std.testing.expectEqual(@as(u16, ico_sizes.len), std.mem.readInt(u16, ico[4..6], .little));
}
