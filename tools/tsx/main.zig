//! zant-tsx: compiles a `.tsx` component file into Zig (`docs/tsx.md`).
//!
//!   zant-tsx <in.tsx> <out.zig>
//!
//! Run by the build (`addTsx` in build.zig); errors print as
//! `file:line:col: error: ...` and exit with status 1.
const std = @import("std");
const compile = @import("compile.zig");
const entry = @import("entry.zig");
const assets = @import("assets.zig");

pub fn main(init: std.process.Init) !void {
    const io = init.io;
    var arena_state = std.heap.ArenaAllocator.init(init.gpa);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const args = try init.minimal.args.toSlice(arena);
    if (args.len >= 4 and std.mem.eql(u8, args[1], "--out")) return project(io, arena, args[2], args[3..]);
    if (args.len != 3) {
        std.debug.print(
            \\用法: zant-tsx <in.tsx> <out.zig>
            \\      zant-tsx --out <dir> [--main <App>] [--src <dir>] [<a.tsx> <b.zig>...]
            \\
        , .{});
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

/// `--out <dir> [--main <App>] [--src <dir>] [--dep <file>] files...`:
/// compile TSX and copy Zig into one output directory. `--src` adds every
/// `.tsx` / `.zig` under a directory; `--main App` also writes the app's
/// entry `main.zig` (`entry.zig`); imported assets are copied to
/// `_assets/` and listed in the `--dep` file (`assets.zig`).
fn project(io: std.Io, arena: std.mem.Allocator, out_dir: []const u8, rest: []const [:0]const u8) !void {
    var paths: std.ArrayList([]const u8) = .empty;
    var main_entry: ?[]const u8 = null;
    var dep_file: ?[]const u8 = null;
    var i: usize = 0;
    while (i < rest.len) : (i += 1) {
        const arg: []const u8 = rest[i];
        if (std.mem.eql(u8, arg, "--dep") and i + 1 < rest.len) {
            i += 1;
            dep_file = rest[i];
        } else if ((std.mem.eql(u8, arg, "--main") or std.mem.eql(u8, arg, "--src")) and i + 1 < rest.len) {
            i += 1;
            if (std.mem.eql(u8, arg, "--main")) main_entry = rest[i] else try entry.collect(io, arena, rest[i], &paths);
        } else try paths.append(arena, arg);
    }
    const files = try arena.alloc(compile.File, paths.items.len);
    for (paths.items, 0..) |path, k| files[k] = .{ .path = path, .src = read(io, arena, path) };
    switch (try compile.compileProject(arena, files)) {
        .ok => |outputs| {
            try std.Io.Dir.cwd().createDirPath(io, out_dir);
            for (files, outputs) |file, zig| {
                const out = try std.fs.path.join(arena, &.{ out_dir, try file.outputName(arena) });
                try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = out, .data = zig });
            }
            try copyAssets(io, arena, out_dir, files, dep_file);
            if (main_entry) |name| {
                const source = entry.mainSource(arena, files, name) catch |err| {
                    std.debug.print("zant-tsx: --main {s}: {s}\n", .{ name, entry.describe(err) });
                    std.process.exit(1);
                };
                const out = try std.fs.path.join(arena, &.{ out_dir, "main.zig" });
                try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = out, .data = source });
            }
        },
        .err => |e| {
            const file = files[e.file];
            std.debug.print("{s}", .{try compile.renderDiag(arena, file.path, file.src, e.diag)});
            std.process.exit(1);
        },
    }
}

fn copyAssets(
    io: std.Io,
    arena: std.mem.Allocator,
    out_dir: []const u8,
    files: []const compile.File,
    dep_file: ?[]const u8,
) !void {
    const list = try assets.collect(arena, files);
    for (list) |asset| {
        const data = std.Io.Dir.cwd().readFileAlloc(io, asset.from, arena, .limited(64 << 20)) catch |err| {
            const diag = compile.Diag{ .loc = asset.loc, .msg = try std.fmt.allocPrint(
                arena,
                "cannot read the asset `{s}` ({s})",
                .{ asset.from, @errorName(err) },
            ) };
            std.debug.print("{s}", .{try compile.renderDiag(arena, asset.importer, asset.importer_src, diag)});
            std.process.exit(1);
        };
        const out = try std.fs.path.join(arena, &.{ out_dir, asset.key });
        try std.Io.Dir.cwd().createDirPath(io, std.fs.path.dirname(out).?);
        try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = out, .data = data });
    }
    if (dep_file) |dep| {
        try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = dep, .data = try assets.depFile(arena, out_dir, list) });
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
    _ = @import("jsx_entities_test.zig");
    _ = @import("tags.zig");
    _ = @import("codegen.zig");
    _ = @import("codegen/errors_test.zig");
    _ = @import("codegen/emission_test.zig");
    _ = @import("codegen/stateful.zig");
    _ = @import("codegen/hooks_test.zig");
    _ = @import("codegen/react_compat_test.zig");
    _ = @import("codegen/builtins_test.zig");
    _ = @import("codegen/loops_test.zig");
    _ = @import("codegen/widgets_test.zig");
    _ = @import("codegen/stores_test.zig");
    _ = @import("codegen/nesting_test.zig");
    _ = @import("codegen/functions_test.zig");
    _ = @import("codegen/spread_test.zig");
    _ = @import("codegen/app_hooks_test.zig");
    _ = @import("codegen/windows_test.zig");
    _ = @import("codegen/desktop_hooks_test.zig");
    _ = @import("codegen/zant_api_test.zig");
    _ = @import("codegen/strings_test.zig");
    _ = compile;
    _ = @import("project_test.zig");
    _ = entry;
    _ = assets;
    _ = @import("codegen/zig_imports.zig");
    _ = @import("golden_test.zig");
}
