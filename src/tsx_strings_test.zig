//! Runtime string semantics, including deliberate UTF-8 boundary behavior.
const std = @import("std");
const ui = @import("ui.zig");
const s = @import("tsx_strings.zig");
const eq = std.testing.expectEqualStrings;
const expect = std.testing.expect;

test "string queries do not need a UI frame and report UTF-16 offsets" {
    try expect(s.includes("Hello", "ell"));
    try expect(!s.includes("", "中"));
    try expect(s.includes("", ""));
    try expect(s.startsWith("中😀abc", "中😀"));
    try expect(!s.startsWith("abc", "bc"));
    try expect(s.startsWith("", ""));
    try expect(s.endsWith("中😀abc", "abc"));
    try expect(s.endsWith("", ""));
    try expect(!s.endsWith("abc", "ab"));
    try std.testing.expectEqual(@as(isize, 3), s.indexOf("中😀abc😀", "abc"));
    try std.testing.expectEqual(@as(isize, 6), s.lastIndexOf("中😀abc😀", "😀"));
    try std.testing.expectEqual(@as(isize, -1), s.indexOf("", "a"));
    try std.testing.expectEqual(@as(isize, -1), s.lastIndexOf("中😀", "a"));
    try std.testing.expectEqual(@as(isize, 0), s.indexOf("中😀", ""));
    try std.testing.expectEqual(@as(isize, 3), s.lastIndexOf("中😀", ""));
    try std.testing.expectEqual(@as(isize, 0), s.lastIndexOf("", ""));
    try std.testing.expectEqual(@as(isize, 2), s.lastIndexOf("aaaa", "aa"));
}

test "trim recognizes JS whitespace, excluding NEL and zero width space" {
    ui.beginFrame();
    defer ui.endFrame();
    const ws = " \t\r\n\x0b\x0c\u{a0}\u{feff}\u{2028}\u{2029}" ++
        "\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}" ++
        "\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{202f}\u{205f}\u{3000}";
    try eq("中😀 A", s.trim(ws ++ "中😀 A" ++ ws));
    try eq("中😀 " ++ ws, s.trimStart(ws ++ "中😀 " ++ ws));
    try eq(ws ++ " 中😀", s.trimEnd(ws ++ " 中😀" ++ ws));
    try eq("\u{85}\u{200b}", s.trim("\u{85}\u{200b}"));
    try eq("", s.trim(ws));
    try eq("", s.trimStart(""));
    try eq("", s.trimEnd(""));
}

test "ASCII case mapping preserves non-ASCII bytes" {
    ui.beginFrame();
    defer ui.endFrame();
    try eq("hello 中😀 Äé", s.toLowerCase("HeLLo 中😀 Äé"));
    try eq("HELLO 中😀 Äé", s.toUpperCase("HeLLo 中😀 Äé"));
    try eq("", s.toLowerCase(""));
    try eq("", s.toUpperCase(""));
}

test "slice and substring clamp, truncate, swap and round UTF-16 boundaries down" {
    ui.beginFrame();
    defer ui.endFrame();
    try eq("bc", s.slice("abcd", 1.9, 3.8));
    try eq("abcd", s.slice("abcd", -0.5, null));
    try eq("", s.slice("abcd", 0, -0.5));
    try eq("cd", s.slice("abcd", -2, null));
    try eq("bc", s.slice("abcd", -3, -1));
    try eq("", s.slice("abcd", 3, 1));
    try eq("中😀", s.slice("A中😀Z", 1, -1));
    try eq("", s.slice("😀", 0, 1));
    try eq("😀", s.slice("😀", 1, 2));
    try eq("", s.slice("", -1, null));
    try eq("abcd", s.slice("abcd", -100, 100));
    try eq("bc", s.substring("abcd", 3, 1));
    try eq("ab", s.substring("abcd", -3, 2));
    try eq("中😀Z", s.substring("A中😀Z", 1, null));
    try eq("", s.substring("😀", 1, 0));
    try eq("", s.substring("", -1, 99));
    try eq("a", s.substring("abc", std.math.nan(f64), 1));
    try eq("", s.slice("abc", std.math.inf(f64), null));
    try eq("abc", s.substring("abc", -std.math.inf(f64), std.math.inf(f64)));
}

test "padding and repeat use frame memory with UTF-16 target lengths" {
    ui.beginFrame();
    defer ui.endFrame();
    try eq("ababx", s.padStart("x", 5, "ab"));
    try eq("xabab", s.padEnd("x", 5.9, "ab"));
    try eq(" 中😀", s.padStart("中😀", 4, " "));
    try eq("中😀ab", s.padEnd("中😀", 5, "ab"));
    try eq("😀x", s.padStart("x", 4, "😀"));
    try eq("中", s.padEnd("中", -2, "ab"));
    try eq("中", s.padStart("中", 9, ""));
    try eq("", s.padStart("", 0, "x"));
    try eq("xx", s.padEnd("", 2, "x"));
    try eq("a中😀a中😀", s.repeat("a中😀", 2.9));
    try eq("", s.repeat("x", -0.5));
    try eq("", s.repeat("", 100));
    try eq("", s.repeat("x", std.math.nan(f64)));
}

test "replace handles literal patterns, empty matches and JS dollar substitutions" {
    ui.beginFrame();
    defer ui.endFrame();
    try eq("x-b-a", s.replace("a-b-a", "a", "x"));
    try eq("x-b-x", s.replaceAll("a-b-a", "a", "x"));
    try eq("[中]-[中]", s.replaceAll("中-中", "中", "[$&]"));
    try eq("中x中x", s.replaceAll("中😀中😀", "😀", "x"));
    try eq("aaa", s.replaceAll("aaaaa", "aa", "a"));
    try eq("abc", s.replace("abc", "z", "x"));
    try eq("a$-b-a-c-$1c", s.replace("abc", "b", "$$-$&-$`-$'-$1"));
    try eq("-a-b-", s.replaceAll("ab", "", "-"));
    try eq("--😀-", s.replaceAll("😀", "", "-"));
    try eq("x中😀", s.replace("中😀", "", "x"));
    try eq("", s.replace("", "a", "b"));
    try eq("x", s.replaceAll("", "", "x"));
}

test "split handles empty fields, CJK, emoji and empty separators by code point" {
    ui.beginFrame();
    defer ui.endFrame();
    const parts = s.split(",中,😀,", ",");
    try std.testing.expectEqual(@as(usize, 4), parts.len);
    try eq("", parts[0]);
    try eq("中", parts[1]);
    try eq("😀", parts[2]);
    try eq("", parts[3]);
    try std.testing.expectEqual(@as(usize, 0), s.split("", "").len);
    try eq("", s.split("", ",")[0]);
    try eq("abc", s.split("abc", "z")[0]);
    const cps = s.split("A中😀", "");
    try std.testing.expectEqual(@as(usize, 3), cps.len);
    try eq("A", cps[0]);
    try eq("中", cps[1]);
    try eq("😀", cps[2]);
}

test "charAt and at share rounded boundaries, at supports negative positions and null" {
    ui.beginFrame();
    defer ui.endFrame();
    try eq("中", s.charAt("A中😀", 1.9));
    try eq("", s.charAt("A中😀", -1));
    try eq("", s.charAt("A中😀", 4));
    try eq("", s.charAt("😀", 0));
    try eq("😀", s.charAt("😀", 1));
    try eq("", s.charAt("", 0));
    try eq("A", s.charAt("ABC", std.math.nan(f64)));
    try eq("", s.charAt("ABC", std.math.inf(f64)));
    try eq("😀", s.at("A中😀", -1).?);
    try eq("中", s.at("A中😀", -3).?);
    try eq("A", s.at("ABC", -0.5).?);
    try expect(s.at("A中😀", -5) == null);
    try expect(s.at("A", 1) == null);
    try expect(s.at("", 0) == null);
    try expect(s.at("ABC", -std.math.inf(f64)) == null);
}
