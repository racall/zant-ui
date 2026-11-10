//! A thumbnail cache stand-in for tests/tsx/media/App.tsx.
const std = @import("std");
const zant = @import("zant");

pub fn thumbnail(id: u64) ?zant.image.Image {
    _ = id;
    return null;
}

/// The width in a PNG's header (an imported asset reaches Zig as bytes).
pub fn pngWidth(png: zant.tsx.Bytes) u32 {
    if (png.len < 24 or !std.mem.eql(u8, png[1..4], "PNG")) return 0;
    return std.mem.readInt(u32, png[16..20], .big);
}
