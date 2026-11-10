//! Inputs watched by `zant run --watch`, independent of the project language.
const std = @import("std");
const Io = std.Io;
const Dir = Io.Dir;

fn hashFileStamp(hash: *std.hash.Wyhash, io: Io, path: []const u8) void {
    const st = Dir.cwd().statFile(io, path, .{}) catch return;
    hash.update(path);
    hash.update(std.mem.asBytes(&st.mtime.nanoseconds));
    hash.update(std.mem.asBytes(&st.size));
}

fn hashTree(allocator: std.mem.Allocator, io: Io, hash: *std.hash.Wyhash, dir_path: []const u8) void {
    var dir = Dir.cwd().openDir(io, dir_path, .{ .iterate = true }) catch return;
    defer dir.close(io);
    var it = dir.iterate();
    while (it.next(io) catch null) |entry| {
        const path = std.fs.path.join(allocator, &.{ dir_path, entry.name }) catch continue;
        defer allocator.free(path);
        switch (entry.kind) {
            .directory => hashTree(allocator, io, hash, path),
            .file => {
                const ext = std.fs.path.extension(entry.name);
                if (std.mem.eql(u8, ext, ".zig") or std.mem.eql(u8, ext, ".tsx") or std.mem.eql(u8, ext, ".ts")) {
                    hashFileStamp(hash, io, path);
                }
            },
            else => {},
        }
    }
}

/// Fingerprint of everything that affects the app build: the config files
/// and every `.zig` / `.tsx` / `.ts` file under `src/` and `native/` (TS
/// apps' native modules and their specs), any depth.
pub fn stamp(allocator: std.mem.Allocator, io: Io, root: []const u8) !u64 {
    var hash = std.hash.Wyhash.init(0);
    const files = [_][]const u8{ "app.config.json", "build.zig", "build.zig.zon" };
    for (files) |rel| {
        const path = try std.fs.path.join(allocator, &.{ root, rel });
        defer allocator.free(path);
        hashFileStamp(&hash, io, path);
    }
    for ([_][]const u8{ "src", "native" }) |sub| {
        const dir = try std.fs.path.join(allocator, &.{ root, sub });
        defer allocator.free(dir);
        hashTree(allocator, io, &hash, dir);
    }
    return hash.final();
}

test "watch nested TSX edits, additions, removals and native/config inputs" {
    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();
    const io = std.testing.io;
    const allocator = std.testing.allocator;
    const root = try std.fmt.allocPrint(allocator, ".zig-cache/tmp/{s}", .{tmp.sub_path});
    defer allocator.free(root);
    // More than the old limit of eight levels; nested component edits must restart.
    const nested = "src/a/b/c/d/e/f/g/h/i/j/Child.tsx";
    try tmp.dir.createDirPath(io, "src/a/b/c/d/e/f/g/h/i/j");
    try tmp.dir.createDirPath(io, "native/engine");
    var previous = try stamp(allocator, io, root);
    const inputs = [_][]const u8{
        "src/App.tsx",    nested,                    "src/native.zig",  "app.config.json",
        "build.zig",      "build.zig.zon",           "native/clips.ts", "native/clips.zig",
        "native/lib.zig", "native/engine/store.zig",
    };
    for (inputs) |file| {
        try tmp.dir.writeFile(io, .{ .sub_path = file, .data = "first" });
        const added = try stamp(allocator, io, root);
        try std.testing.expect(added != previous);
        try tmp.dir.writeFile(io, .{ .sub_path = file, .data = "edited and longer" });
        const edited = try stamp(allocator, io, root);
        try std.testing.expect(edited != added);
        previous = edited;
    }
    try tmp.dir.deleteFile(io, nested);
    try std.testing.expect(try stamp(allocator, io, root) != previous);
}
