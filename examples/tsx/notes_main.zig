//! Window for the two-file TSX notes app (`examples/tsx/notes/`).
const std = @import("std");
const app = @import("app.zig");
const Notes = @import("notes_tsx").Notes;

pub fn main(init: std.process.Init) !void {
    try app.run(Notes, init, .{
        .title = "TSX Notes",
        .window = .{ .width = 520, .height = 560, .min_width = 420, .min_height = 420 },
    });
}
