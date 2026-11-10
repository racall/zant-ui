//! Asset imports (docs/tsx.md, "资源文件"):
//!
//!     import trayIcon from "../assets/tray.png";   // Uint8Array, embedded at compile time
//!
//! A relative import with an extension other than .tsx / .ts / .zig names a
//! file whose bytes are embedded. zant-tsx copies it next to the generated
//! Zig as `_assets/<hash>-<name>` (`@embedFile` only reads inside the module)
//! and lists it in the build's dep file, so editing the file rebuilds.
const std = @import("std");

pub fn isAsset(source: []const u8) bool {
    const relative = std.mem.startsWith(u8, source, "./") or std.mem.startsWith(u8, source, "../");
    if (!relative) return false;
    const ext = std.fs.path.extension(std.fs.path.basename(source));
    if (ext.len == 0) return false;
    for ([_][]const u8{ ".tsx", ".ts", ".zig", ".jsx", ".js" }) |code| if (std.mem.eql(u8, ext, code)) return false;
    return true;
}

/// The asset's path on disk: relative to the importing file.
pub fn path(arena: std.mem.Allocator, importer: []const u8, source: []const u8) ![]const u8 {
    const dir = std.fs.path.dirname(importer) orelse ".";
    return std.fs.path.join(arena, &.{ dir, source });
}

/// Where the copy lives in the output directory: one name per file on disk
/// (two imports of it share it), readable in generated code.
pub fn key(arena: std.mem.Allocator, importer: []const u8, source: []const u8) ![]const u8 {
    const full = try path(arena, importer, source);
    const normalized = try std.fs.path.resolvePosix(arena, &.{try std.mem.replaceOwned(u8, arena, full, "\\", "/")});
    const hash = std.hash.Wyhash.hash(0, normalized);
    return std.fmt.allocPrint(arena, "_assets/{x:0>8}-{s}", .{
        @as(u32, @truncate(hash)), std.fs.path.basename(source),
    });
}

test "asset imports by extension" {
    try std.testing.expect(isAsset("../assets/tray.png"));
    try std.testing.expect(isAsset("./data/words.txt"));
    try std.testing.expect(!isAsset("./components/Row"));
    try std.testing.expect(!isAsset("./Row.tsx"));
    try std.testing.expect(!isAsset("./sync.zig"));
    try std.testing.expect(!isAsset("zant"));
    try std.testing.expect(!isAsset("./.hidden"));
}

test "one key per file on disk" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const a = arena_state.allocator();
    const from_app = try key(a, "src/App.tsx", "../assets/tray.png");
    const from_child = try key(a, "src/components/Tray.tsx", "../../assets/tray.png");
    try std.testing.expectEqualStrings(from_app, from_child);
    try std.testing.expect(std.mem.endsWith(u8, from_app, "-tray.png"));
    try std.testing.expect(!std.mem.eql(u8, from_app, try key(a, "src/App.tsx", "./tray.png")));
}

pub const Asset = struct {
    /// The importing .tsx file and the import's position in it (errors).
    importer: []const u8,
    importer_src: []const u8,
    loc: u32,
    /// Path on disk and the copy's name in the output directory.
    from: []const u8,
    key: []const u8,
};

/// The asset imports of the project's .tsx files (already compiled, so they parse).
pub fn collect(arena: std.mem.Allocator, files: []const @import("compile.zig").File) ![]const Asset {
    var list: std.ArrayList(Asset) = .empty;
    for (files) |file| {
        if (file.isZig()) continue;
        var p = @import("parser.zig").Parser.init(arena, file.src);
        const module = p.parseModule() catch continue;
        for (module.imports) |imp| if (isAsset(imp.source)) try list.append(arena, .{
            .importer = file.path,
            .importer_src = file.src,
            .loc = imp.source_loc,
            .from = try path(arena, file.path, imp.source),
            .key = try key(arena, file.path, imp.source),
        });
    }
    return list.items;
}

/// A Makefile-style dep file for the build (`addDepFileOutputArg`): every
/// source read (`sources`) and asset embedded; editing one reruns zant-tsx,
/// and a deleted one is only a cache miss (unlike inputs listed when the
/// build is configured, which go stale).
pub fn depFile(arena: std.mem.Allocator, target: []const u8, sources: []const []const u8, list: []const Asset) ![]const u8 {
    var out: std.ArrayList(u8) = .empty;
    try out.appendSlice(arena, try escape(arena, target));
    try out.append(arena, ':');
    for (sources) |source| {
        try out.appendSlice(arena, " \\\n  ");
        try out.appendSlice(arena, try escape(arena, source));
    }
    for (list) |asset| {
        try out.appendSlice(arena, " \\\n  ");
        try out.appendSlice(arena, try escape(arena, asset.from));
    }
    try out.append(arena, '\n');
    return out.items;
}

fn escape(arena: std.mem.Allocator, value: []const u8) ![]const u8 {
    return std.mem.replaceOwned(u8, arena, value, " ", "\\ ");
}

test "dep file lists the assets" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const a = arena_state.allocator();
    const list = [_]Asset{.{ .importer = "", .importer_src = "", .loc = 0, .from = "my app/tray.png", .key = "" }};
    try std.testing.expectEqualStrings("out: \\\n  src/App.tsx \\\n  my\\ app/tray.png\n", try depFile(a, "out", &.{"src/App.tsx"}, &list));
}
