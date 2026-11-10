//! Golden outputs are also compiled against the real framework by test-tsx.
const std = @import("std");
const compiler = @import("compile.zig");

test "TSX golden files match formatted compiler output" {
    inline for (.{
        "counter",  "layout", "text", "events",      "conditional", "components", "todos", "arrays",
        "children", "theme",  "list", "local_state", "hooks",       "strings",    "store", "widgets",
    }) |name| {
        var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
        defer arena.deinit();
        const src = @embedFile("testdata/" ++ name ++ ".tsx");
        const expected = @embedFile("testdata/" ++ name ++ ".zig");
        const result = try compiler.compile(arena.allocator(), src, name ++ ".tsx");
        if (result == .err) {
            std.debug.print("{s}", .{try compiler.renderDiag(arena.allocator(), name ++ ".tsx", src, result.err)});
            return error.GoldenCompilationFailed;
        }
        if (!std.mem.eql(u8, expected, result.ok)) {
            var old_lines = std.mem.splitScalar(u8, expected, '\n');
            var new_lines = std.mem.splitScalar(u8, result.ok, '\n');
            var line: usize = 1;
            while (true) : (line += 1) {
                const old = old_lines.next();
                const new = new_lines.next();
                if (old == null and new == null) break;
                if (old == null or new == null or !std.mem.eql(u8, old.?, new.?)) {
                    std.debug.print(
                        "{s}.zig:{d}: golden mismatch\n- {s}\n+ {s}\n",
                        .{ name, line, old orelse "<end of file>", new orelse "<end of file>" },
                    );
                    break;
                }
            }
            return error.GoldenMismatch;
        }
    }
}

/// A boundary check prevents <IconButton> from accidentally counting as <Icon>.
fn hasTag(source: []const u8, prefix: []const u8) bool {
    var start: usize = 0;
    while (std.mem.indexOfPos(u8, source, start, prefix)) |at| {
        const end = at + prefix.len;
        if (end < source.len and std.mem.indexOfScalar(u8, " />\n\r\t", source[end]) != null) return true;
        start = end;
    }
    return false;
}

test "components golden covers every generated tag" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const source = @embedFile("testdata/components.tsx");
    for (@import("tags.zig").all) |tag| {
        const prefix = try std.fmt.allocPrint(arena.allocator(), "<{s}", .{tag.name});
        if (!hasTag(source, prefix) and !hasTag(@embedFile("testdata/widgets.tsx"), prefix)) {
            std.debug.print("components.tsx is missing <{s}>\n", .{tag.name});
            return error.MissingTagCoverage;
        }
    }
}
