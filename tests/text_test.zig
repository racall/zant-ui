const std = @import("std");
const zant = @import("zant");

test "CJK and mixed text shaping and wrapping" {
    var layout = try zant.text.TextLayout.init(
        "Zant UI 中文排版与 English 混排，自动换行并参与 flex 布局。",
        .{
            .font_size = 16,
            .color = 0xe0000000,
            .wrap = true,
            .line_height = 1.5,
        },
        180,
    );
    defer layout.deinit();
    try std.testing.expect(layout.metrics.line_count >= 2);
    try std.testing.expect(layout.metrics.height > 20);
    try std.testing.expect(layout.metrics.longest_line <= 180.5);
    try std.testing.expect(layout.caretRect(0) != null);
    try std.testing.expect(layout.caretRect(3) != null);
}

test "UTF-8 cursor offset never splits Chinese input" {
    const s = "中文 Zant";
    var layout = try zant.text.TextLayout.init(s, .{
        .font_size = 14,
        .wrap = false,
    }, 1.0e6);
    defer layout.deinit();
    const caret = layout.caretRect(3) orelse return error.NoCaret;
    try std.testing.expect(caret.width > 0);
    const index = layout.indexAt(caret.x + 1, caret.y + 2);
    try std.testing.expect(index <= s.len);
}
