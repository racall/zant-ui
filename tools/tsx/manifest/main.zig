//! zant-tsx-tags <path> writes the reflected table; --check <path> verifies it.
//! Reflection runs at compile time; formatting and file I/O run once at startup.
const std = @import("std");
const reflect = @import("reflect.zig");

pub fn main(init: std.process.Init) !void {
    var arena_state = std.heap.ArenaAllocator.init(init.gpa);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const args = try init.minimal.args.toSlice(arena);
    const check = args.len == 3 and std.mem.eql(u8, args[1], "--check");
    if (!check and (args.len != 2 or std.mem.startsWith(u8, args[1], "--"))) {
        std.debug.print("usage: zant-tsx-tags [--check] <path>\n", .{});
        std.process.exit(2);
    }
    const raw = comptime reflect.source();
    var ast = try std.zig.Ast.parse(arena, raw ++ "", .{});
    defer ast.deinit(arena);
    if (ast.errors.len != 0) return error.InvalidGeneratedSource;
    const output = try ast.renderAlloc(arena);
    const path = args[if (check) 2 else 1];
    if (check) {
        const existing = std.Io.Dir.cwd().readFileAlloc(init.io, path, arena, .limited(16 << 20)) catch |err| switch (err) {
            error.FileNotFound => outdated(path),
            else => return err,
        };
        if (!std.mem.eql(u8, existing, output)) outdated(path);
    } else {
        try std.Io.Dir.cwd().writeFile(init.io, .{ .sub_path = path, .data = output });
    }
}

fn outdated(path: []const u8) noreturn {
    std.debug.print("zant-tsx-tags: {s} is out of date; run `zig build tsx-tags`\n", .{path});
    std.process.exit(1);
}

test {
    _ = @import("names.zig");
}
