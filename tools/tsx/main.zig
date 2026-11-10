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
    if (args.len >= 4 and std.mem.eql(u8, args[1], "--out")) return project(io, arena, args[2], args[3..]);
    if (args.len != 3) {
        std.debug.print("用法: zant-tsx <in.tsx> <out.zig>\n      zant-tsx --out <dir> <a.tsx> <b.tsx>...\n", .{});
        std.process.exit(2);
    }
    const in_path = args[1];
    const src = read(io, arena, in_path);
    switch (try compile.compile(arena, src, std.fs.path.basename(in_path))) {
        .ok => |zig| try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = args[2], .data = zig }),
        .err => |diag| {
            std.debug.print("{s}", .{try compile.renderDiag(arena, in_path, src, diag)});
            std.process.exit(1);
        },
    }
}

/// `--out <dir> files...`: compile TSX and copy Zig into one output directory.
fn project(io: std.Io, arena: std.mem.Allocator, out_dir: []const u8, paths: []const [:0]const u8) !void {
    const files = try arena.alloc(compile.File, paths.len);
    for (paths, 0..) |path, i| files[i] = .{ .path = path, .src = read(io, arena, path) };
    switch (try compile.compileProject(arena, files)) {
        .ok => |outputs| {
            try std.Io.Dir.cwd().createDirPath(io, out_dir);
            for (files, outputs) |file, zig| {
                const out = try std.fs.path.join(arena, &.{ out_dir, try file.outputName(arena) });
                try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = out, .data = zig });
            }
        },
        .err => |e| {
            const file = files[e.file];
            std.debug.print("{s}", .{try compile.renderDiag(arena, file.path, file.src, e.diag)});
            std.process.exit(1);
        },
    }
}

fn read(io: std.Io, arena: std.mem.Allocator, path: []const u8) []const u8 {
    return std.Io.Dir.cwd().readFileAlloc(io, path, arena, .limited(16 << 20)) catch |err| {
        std.debug.print("zant-tsx: 不能读取 {s}（{s}）\n", .{ path, @errorName(err) });
        std.process.exit(1);
    };
}

test {
    _ = @import("lexer.zig");
    _ = @import("parser.zig");
    _ = @import("parser_test.zig");
    _ = @import("tags.zig");
    _ = @import("codegen.zig");
    _ = @import("codegen/errors_test.zig");
    _ = @import("codegen/emission_test.zig");
    _ = @import("codegen/stateful.zig");
    _ = @import("codegen/hooks_test.zig");
    _ = compile;
    _ = @import("project_test.zig");
    _ = @import("codegen/zig_imports.zig");
    _ = @import("golden_test.zig");
}
