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
