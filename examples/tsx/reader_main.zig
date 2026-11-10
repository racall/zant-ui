//! Two-pane native document app (`zig build run-tsx-reader`).
const std = @import("std");
const app = @import("app.zig");
const Reader = @import("reader_tsx").Reader;

/// Runs the TSX reader, following the system's light/dark appearance.
pub fn main(init: std.process.Init) !void {
    try app.run(Reader, init, .{
        .title = "TSX Reader",
        .window = .{ .width = 1120, .height = 760, .min_width = 640, .min_height = 620 },
    });
}
