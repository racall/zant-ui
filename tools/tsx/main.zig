//! zant-tsx: compiles a `.tsx` component file into Zig (`docs/tsx.md`).
//!
//!   zant-tsx <in.tsx> <out.zig>
//!
//! Run by the build (`addTsx` in build.zig); errors print as
//! `file:line:col: error: ...` and exit with status 1.
const std = @import("std");
const compile = @import("compile.zig");

pub fn main(init: std.process.Init) !void {
    const io = init.io;
    var arena_state = std.heap.ArenaAllocator.init(init.gpa);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const args = try init.minimal.args.toSlice(arena);
    if (args.len != 3) {
        std.debug.print("用法: zant-tsx <in.tsx> <out.zig>\n", .{});
        std.process.exit(2);
    }
    const in_path = args[1];
    const src = std.Io.Dir.cwd().readFileAlloc(io, in_path, arena, .limited(16 << 20)) catch |err| {
        std.debug.print("zant-tsx: 不能读取 {s}（{s}）\n", .{ in_path, @errorName(err) });
        std.process.exit(1);
    };
    switch (try compile.compile(arena, src, std.fs.path.basename(in_path))) {
        .ok => |zig| try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = args[2], .data = zig }),
        .err => |diag| {
            std.debug.print("{s}", .{try compile.renderDiag(arena, in_path, src, diag)});
            std.process.exit(1);
        },
    }
}

test {
    _ = @import("lexer.zig");
    _ = @import("parser.zig");
    _ = @import("parser_test.zig");
    _ = @import("tags.zig");
    _ = @import("codegen.zig");
    _ = @import("codegen/errors_test.zig");
    _ = @import("codegen/emission_test.zig");
    _ = compile;
    _ = @import("golden_test.zig");
}
