//! Project language conventions shared by create, sync and the CLI.
const std = @import("std");

pub const Language = enum {
    zig,
    ts,

    pub fn parse(value: []const u8) ?Language {
        if (std.mem.eql(u8, value, "zig")) return .zig;
        if (std.mem.eql(u8, value, "ts")) return .ts;
        return null;
    }
};

/// A mixed project with a handwritten entry remains a Zig project.
pub fn detect(allocator: std.mem.Allocator, io: std.Io, root: []const u8) !Language {
    const app = try std.fs.path.join(allocator, &.{ root, "src", "App.tsx" });
    defer allocator.free(app);
    const main = try std.fs.path.join(allocator, &.{ root, "src", "main.zig" });
    defer allocator.free(main);
    return if (try exists(io, app) and !try exists(io, main)) .ts else .zig;
}

test "language choices" {
    try std.testing.expectEqual(Language.zig, Language.parse("zig").?);
    try std.testing.expectEqual(Language.ts, Language.parse("ts").?);
    try std.testing.expect(Language.parse("javascript") == null);
}

fn exists(io: std.Io, path: []const u8) !bool {
    std.Io.Dir.cwd().access(io, path, .{}) catch |err| switch (err) {
        error.FileNotFound => return false,
        else => return err,
    };
    return true;
}

test "detect TS only when App.tsx exists without a handwritten main.zig" {
    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();
    const io = std.testing.io;
    const allocator = std.testing.allocator;
    const root = try std.fmt.allocPrint(allocator, ".zig-cache/tmp/{s}", .{tmp.sub_path});
    defer allocator.free(root);
    try tmp.dir.createDirPath(io, "src");
    try std.testing.expectEqual(Language.zig, try detect(allocator, io, root));
    try tmp.dir.writeFile(io, .{ .sub_path = "src/App.tsx", .data = "" });
    try std.testing.expectEqual(Language.ts, try detect(allocator, io, root));
    try tmp.dir.writeFile(io, .{ .sub_path = "src/main.zig", .data = "" });
    try std.testing.expectEqual(Language.zig, try detect(allocator, io, root));
}
