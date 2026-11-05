//! Document statistics and outline.
const std = @import("std");
const ast = @import("ast.zig");
const scan = @import("scan.zig");

pub const Stats = struct {
    /// Latin words + individual CJK characters (how 字数 is usually counted).
    words: usize = 0,
    /// Non-whitespace characters.
    characters: usize = 0,
    lines: usize = 0,
    /// Estimated reading time (300 CJK chars or 200 words per minute).
    reading_minutes: usize = 0,
};

pub fn compute(text: []const u8) Stats {
    var s: Stats = .{};
    if (text.len == 0) return s;
    s.lines = 1;
    var cjk: usize = 0;
    var latin_words: usize = 0;
    var in_word = false;
    var it = (std.unicode.Utf8View.init(text) catch return s).iterator();
    while (it.nextCodepoint()) |cp| {
        if (cp == '\n') s.lines += 1;
        if (scan.isWhitespaceCp(cp)) {
            in_word = false;
            continue;
        }
        s.characters += 1;
        if (scan.isCjk(cp)) {
            if (!scan.isUnicodePunct(cp)) cjk += 1;
            in_word = false;
        } else if (scan.isWordCp(cp)) {
            if (!in_word) latin_words += 1;
            in_word = true;
        } else {
            in_word = false;
        }
    }
    s.words = cjk + latin_words;
    const minutes = @as(f32, @floatFromInt(cjk)) / 300.0 + @as(f32, @floatFromInt(latin_words)) / 200.0;
    s.reading_minutes = @intFromFloat(@ceil(minutes));
    return s;
}

pub const OutlineEntry = struct { level: u8, text: []const u8, id: []const u8 };

/// Headings of a document (for a table of contents).
pub fn outline(allocator: std.mem.Allocator, doc: *const ast.Document) ![]OutlineEntry {
    var out: std.ArrayListUnmanaged(OutlineEntry) = .empty;
    for (doc.blocks) |block| switch (block) {
        .heading => |h| try out.append(allocator, .{ .level = h.level, .text = try ast.plainText(allocator, h.spans), .id = h.id }),
        else => {},
    };
    return out.toOwnedSlice(allocator);
}

test "mixed counts" {
    const s = compute("Hello world，你好世界。\nsecond line");
    try std.testing.expectEqual(@as(usize, 4 + 4), s.words);
    try std.testing.expectEqual(@as(usize, 2), s.lines);
}
