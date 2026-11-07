//! Conformance suite for the remend port: every `expect(remend(x)).toBe(y)`
//! case from Streamdown's `packages/remend/__tests__` (see fixtures/README.md).
const std = @import("std");
const zant = @import("zant");

const Case = struct { file: []const u8, input: []const u8, expected: []const u8 };

test "remend conformance with streamdown test suite" {
    const gpa = std.testing.allocator;
    const parsed = try std.json.parseFromSlice([]Case, gpa, @embedFile("fixtures/remend_cases.json"), .{});
    defer parsed.deinit();
    try std.testing.expect(parsed.value.len > 500);
    var failures: usize = 0;
    for (parsed.value) |case| {
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        const out = try zant.markdown.remend.remend(arena.allocator(), case.input, .{});
        if (!std.mem.eql(u8, out, case.expected)) {
            failures += 1;
            if (failures <= 40) std.debug.print("[{s}] input={f}\n  expected={f}\n  actual  ={f}\n", .{
                case.file,
                std.json.fmt(case.input, .{}),
                std.json.fmt(case.expected, .{}),
                std.json.fmt(out, .{}),
            });
        }
    }
    if (failures > 0) std.debug.print("remend: {d}/{d} cases differ\n", .{ failures, parsed.value.len });
    try std.testing.expectEqual(@as(usize, 0), failures);
}

test "remend leaves brackets inside math alone (no link sentinel)" {
    const gpa = std.testing.allocator;
    const cases = [_][2][]const u8{
        // formula still streaming
        .{ "区间 \\([m,n", "区间 \\([m,n" },
        .{ "\\[\n[a,b", "\\[\n[a,b" },
        .{ "$$\n[a,b", "$$\n[a,b\n$$" },
        // closed formula followed by more text
        .{ "\\(f:[0,1]\\to\\mathbb{R}\\) 且 \\([m,n\\) 上", "\\(f:[0,1]\\to\\mathbb{R}\\) 且 \\([m,n\\) 上" },
        // a real link after a formula is still healed
        .{ "\\([a,b]\\) 见 [文档", "\\([a,b]\\) 见 [文档](streamdown:incomplete-link)" },
    };
    for (cases) |case| {
        var arena = std.heap.ArenaAllocator.init(gpa);
        defer arena.deinit();
        const out = try zant.markdown.remend.remend(arena.allocator(), case[0], .{});
        std.testing.expectEqualStrings(case[1], out) catch |err| {
            std.debug.print("input={f}\n", .{std.json.fmt(case[0], .{})});
            return err;
        };
    }
}
