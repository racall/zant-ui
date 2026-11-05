const std = @import("std");
const zant = @import("zant");

test "test harness starts" {
    try std.testing.expect(true);
}

test "unicode helpers" {
    try std.testing.expectEqual(@as(usize, 3), zant.unicode.nextGrapheme("中A", 0));
    try std.testing.expectEqual(@as(usize, 1), zant.unicode.byteToUtf16("中A", 3));
}

test {
    _ = @import("flex_test.zig");
    _ = @import("component_test.zig");
}

test {
    _ = @import("ui_test.zig");
}

test {
    _ = @import("store_test.zig");
}

test {
    _ = @import("text_test.zig");
}
