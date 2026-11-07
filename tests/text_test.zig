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

test "text relayout at another width matches a fresh layout" {
    const zant_text = @import("zant").text;
    const rich = @import("zant").node.RichSpan;
    _ = rich;
    const samples = [_]zant_text.Style{
        .{ .font_size = 15, .wrap = true },
        .{ .font_size = 13, .wrap = false, .text_align = .center },
        .{ .font_size = 14, .wrap = true, .max_lines = 2 },
    };
    const content = "Relayout keeps the shaped text: 中文混排 English words wrap differently at each width, 123456.";
    for (samples) |style| {
        var a = try zant_text.TextLayout.init(content, style, 300);
        defer a.deinit();
        a.relayout(140);
        var fresh = try zant_text.TextLayout.init(content, style, 140);
        defer fresh.deinit();
        try std.testing.expectApproxEqAbs(fresh.metrics.height, a.metrics.height, 0.01);
        try std.testing.expectApproxEqAbs(fresh.metrics.longest_line, a.metrics.longest_line, 0.01);
        try std.testing.expectEqual(fresh.metrics.line_count, a.metrics.line_count);
        try std.testing.expectEqual(fresh.indexAt(100, fresh.metrics.height - 5), a.indexAt(100, a.metrics.height - 5));
    }
}
