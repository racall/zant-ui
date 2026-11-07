//! Every mhchem fixture output parses and lays out without unknown
//! commands or parse errors (the TeX layout engine covers what mhchem emits).
const std = @import("std");
const zant = @import("zant");
const math = zant.math;

const Case = struct { input: []const u8, machine: []const u8, expected: ?[]const u8 };

fn hasError(cmds: []const zant.vector.Command, error_color: u32) bool {
    for (cmds) |c| switch (c) {
        .text => |t| if (t.color == error_color) return true,
        else => {},
    };
    return false;
}

test "mhchem: all fixture cases lay out cleanly via \\ce / \\pu" {
    const gpa = std.testing.allocator;
    const parsed = try std.json.parseFromSlice([]const Case, gpa, math.mhchem.fixture_json, .{});
    defer parsed.deinit();
    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();
    const error_color: u32 = 0xffff00ff;
    var failures: usize = 0;
    for (parsed.value) |case| {
        if (case.expected == null) continue;
        // inputs KaTeX cannot render either: unknown macros, alignment
        // characters outside an environment, upstream's `\color{r}` bug
        const exp = case.expected.?;
        const katex_errors = [_][]const u8{ "\\foo", "\\cf{", "\\x{", "\\alphaH", "\\color{r}", "\\color{\\red}", "&", "\\hline", "{_}", "{^}" };
        var skip = false;
        for (katex_errors) |k| if (std.mem.indexOf(u8, exp, k) != null) {
            skip = true;
        };
        if (skip) continue;
        _ = arena.reset(.retain_capacity);
        const src = try std.fmt.allocPrint(arena.allocator(), "\\{s}{{{s}}}", .{ case.machine, case.input });
        const d = try math.layout(arena.allocator(), src, .{ .error_color = error_color, .color = 0xff000000 });
        if (hasError(d.commands, error_color)) {
            failures += 1;
            if (failures <= 25) std.debug.print("unrendered: {s}  ->  {s}\n", .{ src, case.expected.? });
        }
    }
    try std.testing.expectEqual(@as(usize, 0), failures);
}
