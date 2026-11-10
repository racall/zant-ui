//! Window for the TSX component gallery (`examples/tsx/gallery.tsx`).
const std = @import("std");
const app = @import("app.zig");
const Gallery = @import("gallery_tsx").Gallery;

pub fn main(init: std.process.Init) !void {
    try app.run(Gallery, init, .{
        .title = "TSX Gallery",
        .window = .{ .width = 960, .height = 760, .min_width = 900, .min_height = 560 },
    });
}
