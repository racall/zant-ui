//! Deterministic Zig field / enum spellings used by the manifest generator.
const std = @import("std");

/// Snake case becomes camelCase; existing camelCase stays intact.
pub fn camel(comptime name: []const u8) []const u8 {
    var result: []const u8 = "";
    var upper = false;
    for (name) |c| {
        if (c == '_') {
            upper = true;
        } else {
            result = result ++ [_]u8{if (upper) std.ascii.toUpper(c) else c};
            upper = false;
        }
    }
    return result;
}

/// Builder names become PascalCase TSX tags (including Zig keywords).
pub fn pascal(comptime name: []const u8) []const u8 {
    const value = camel(name);
    if (value.len == 0) return "";
    return [_]u8{std.ascii.toUpper(value[0])} ++ value[1..];
}

/// Native, kebab-case, then camelCase; retain only the first of duplicates.
pub fn spellings(comptime name: []const u8) []const []const u8 {
    var kebab: []const u8 = "";
    for (name) |c| kebab = kebab ++ [_]u8{if (c == '_') '-' else c};
    var result: []const []const u8 = &.{};
    for ([_][]const u8{ name, kebab, camel(name) }) |candidate| {
        var found = false;
        for (result) |earlier| {
            if (std.mem.eql(u8, earlier, candidate)) found = true;
        }
        if (!found) result = result ++ [_][]const u8{candidate};
    }
    return result;
}

test "snake case fields become camelCase" {
    inline for (.{ .{ "on_change", "onChange" }, .{ "width_percent", "widthPercent" }, .{ "radioGroup", "radioGroup" }, .{ "key", "key" }, .{ "", "" } }) |pair| {
        try std.testing.expectEqualStrings(pair[1], comptime camel(pair[0]));
    }
}

test "builders become PascalCase" {
    inline for (.{ .{ "radioGroup", "RadioGroup" }, .{ "switch", "Switch" }, .{ "pdfPreview", "PdfPreview" }, .{ "foo_bar", "FooBar" }, .{ "", "" } }) |pair| {
        try std.testing.expectEqualStrings(pair[1], comptime pascal(pair[0]));
    }
}

test "enum spellings are ordered and deduplicated" {
    const values = comptime spellings("chevron_down");
    try std.testing.expectEqual(@as(usize, 3), values.len);
    for (values, [_][]const u8{ "chevron_down", "chevron-down", "chevronDown" }) |actual, expected| {
        try std.testing.expectEqualStrings(expected, actual);
    }
    try std.testing.expectEqual(@as(usize, 1), (comptime spellings("none")).len);
    try std.testing.expectEqual(@as(usize, 1), (comptime spellings("alreadyCamel")).len);
}
