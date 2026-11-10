//! Window for the TSX todo app (`examples/tsx/todos.tsx`).
const std = @import("std");
const app = @import("app.zig");
const Todos = @import("todos_tsx").Todos;

pub fn main(init: std.process.Init) !void {
    try app.run(Todos, init, .{
        .title = "TSX Todos",
        .window = .{ .width = 560, .height = 640, .min_width = 420, .min_height = 420 },
    });
}
