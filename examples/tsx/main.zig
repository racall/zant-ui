//! Window for the TSX counter (`examples/tsx/counter.tsx`, compiled to Zig
//! by `zant-tsx` during the build).
const std = @import("std");
const app = @import("app.zig");
const Counter = @import("counter_tsx").Counter;

pub fn main(init: std.process.Init) !void {
    try app.run(Counter, init, .{
        .title = "TSX Counter",
        .window = .{ .width = 520, .height = 480, .min_width = 420, .min_height = 420 },
    });
}
