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

comptime {
    _ = @import("db_test.zig");
}

comptime {
    _ = @import("markdown_test.zig");
}
comptime {
    _ = @import("remend_test.zig");
}
comptime {
    _ = @import("math_test.zig");
}
comptime {
    _ = @import("antd_components_test.zig");
}
comptime {
    _ = @import("lifecycle_test.zig");
}
comptime {
    _ = @import("motion_test.zig");
}
comptime {
    _ = @import("pdf_test.zig");
}
comptime {
    _ = @import("text_align_test.zig");
}
comptime {
    _ = @import("pointer_test.zig");
}
comptime {
    _ = @import("pdf_viewer_test.zig");
}
comptime {
    _ = @import("selection_test.zig");
}
comptime {
    _ = @import("mhchem_render_test.zig");
}
comptime {
    _ = @import("list_test.zig");
}
