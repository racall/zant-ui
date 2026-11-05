//! Port of remend `patterns.ts` and the character helpers of `utils.ts`.
//!
//! The original uses end-anchored regular expressions; each `TrailingPattern`
//! below reproduces one of them with an explicit leftmost-match search.
const std = @import("std");
const md_scan = @import("../scan.zig");

pub const Body = enum {
    /// `([^D]*?)$` — no delimiter after the marker
    none,
    /// `([^D]*\D?)$` — no delimiter except an optional final one (bold)
    optional_final,
    /// `([^D]+)D$` — half-complete closer: content then one delimiter
    half_complete,
};

pub const TrailingPattern = struct {
    delimiter: u8,
    marker: []const u8,
    body: Body,
};

pub const Match = struct {
    /// Full match (marker + content) — `match[0]`
    whole: []const u8,
    /// `match[1]`
    marker: []const u8,
    /// `match[2]`
    content: []const u8,
};

pub const bold: TrailingPattern = .{ .delimiter = '*', .marker = "**", .body = .optional_final };
pub const italic: TrailingPattern = .{ .delimiter = '_', .marker = "__", .body = .none };
pub const bold_italic: TrailingPattern = .{ .delimiter = '*', .marker = "***", .body = .none };
pub const single_asterisk: TrailingPattern = .{ .delimiter = '*', .marker = "*", .body = .none };
pub const single_underscore: TrailingPattern = .{ .delimiter = '_', .marker = "_", .body = .none };
pub const strikethrough: TrailingPattern = .{ .delimiter = '~', .marker = "~~", .body = .none };
pub const half_complete_underscore: TrailingPattern = .{ .delimiter = '_', .marker = "__", .body = .half_complete };
pub const half_complete_tilde: TrailingPattern = .{ .delimiter = '~', .marker = "~~", .body = .half_complete };

fn bodyMatches(rest: []const u8, delimiter: u8, body: Body) bool {
    switch (body) {
        .none => return std.mem.indexOfScalar(u8, rest, delimiter) == null,
        .optional_final => {
            const inner = if (rest.len > 0 and rest[rest.len - 1] == delimiter) rest[0 .. rest.len - 1] else rest;
            return std.mem.indexOfScalar(u8, inner, delimiter) == null;
        },
        .half_complete => {
            if (rest.len < 2 or rest[rest.len - 1] != delimiter) return false;
            return std.mem.indexOfScalar(u8, rest[0 .. rest.len - 1], delimiter) == null;
        },
    }
}

/// `matchTrailing(text, pattern)`: only the tail starting near the last
/// delimiter (excluding the final character) is examined.
pub fn matchTrailing(text: []const u8, pattern: TrailingPattern) ?Match {
    if (text.len == 0) return null;
    // text.lastIndexOf(delimiter, text.length - 2)
    var last: ?usize = null;
    if (text.len >= 2) {
        var i: usize = text.len - 1;
        while (i > 0) {
            i -= 1;
            if (text[i] == pattern.delimiter) {
                last = i;
                break;
            }
        }
    }
    const marker_end = last orelse text.len - 1;
    const begin = if (marker_end + 1 >= pattern.marker.len) marker_end + 1 - pattern.marker.len else 0;
    const slice = text[begin..];
    var p: usize = 0;
    while (p + pattern.marker.len <= slice.len) : (p += 1) {
        if (!std.mem.eql(u8, slice[p .. p + pattern.marker.len], pattern.marker)) continue;
        const rest = slice[p + pattern.marker.len ..];
        if (bodyMatches(rest, pattern.delimiter, pattern.body)) {
            return .{ .whole = slice[p..], .marker = slice[p .. p + pattern.marker.len], .content = rest };
        }
    }
    return null;
}

/// `/^[\s_~*`]*$/`
pub fn isWhitespaceOrMarkers(text: []const u8) bool {
    for (text) |c| {
        switch (c) {
            ' ', '\t', '\n', '\r', 0x0B, 0x0C, '_', '~', '*', '`' => {},
            else => return false,
        }
    }
    return true;
}

/// `/^[\s]*[-*+][\s]+$/`
pub fn isListItemPrefix(text: []const u8) bool {
    var i: usize = 0;
    while (i < text.len and std.ascii.isWhitespace(text[i])) i += 1;
    if (i >= text.len or (text[i] != '-' and text[i] != '*' and text[i] != '+')) return false;
    i += 1;
    if (i >= text.len) return false;
    while (i < text.len) : (i += 1) if (!std.ascii.isWhitespace(text[i])) return false;
    return true;
}

/// `/^\*{4,}$/`
pub fn isFourOrMoreAsterisks(text: []const u8) bool {
    if (text.len < 4) return false;
    for (text) |c| if (c != '*') return false;
    return true;
}

/// `isWordChar`: ASCII letters, digits, `_`, or any Unicode letter/number.
/// Takes the code point (0 = no character).
pub fn isWordChar(cp: u21) bool {
    if (cp == 0) return false;
    if (cp < 0x80) return std.ascii.isAlphanumeric(@intCast(cp)) or cp == '_';
    // \p{L}\p{N}: approximate as "not whitespace, not punctuation/symbol".
    return !md_scan.isWhitespaceCp(cp) and !md_scan.isUnicodePunct(cp) and !isSymbol(cp);
}

fn isSymbol(cp: u21) bool {
    return (cp >= 0x2100 and cp <= 0x2BFF) or // letterlike, arrows, math, technical, shapes
        (cp >= 0x1F000 and cp <= 0x1FAFF) or // emoji
        (cp >= 0xFE00 and cp <= 0xFE0F) or cp == 0x00A9 or cp == 0x00AE or cp == 0x00B0 or
        cp == 0x00D7 or cp == 0x00F7;
}

/// Previous character (code point) before byte `index`, 0 when none.
pub fn prevChar(text: []const u8, index: usize) u21 {
    return md_scan.cpBefore(text, index);
}

/// Character starting at byte `index`, 0 when none.
pub fn charAt(text: []const u8, index: usize) u21 {
    return md_scan.cpAt(text, index);
}

/// `isHorizontalRule(text, markerIndex, marker)`
pub fn isHorizontalRule(text: []const u8, marker_index: usize, marker: u8) bool {
    var line_start: usize = 0;
    var i = marker_index;
    while (i > 0) {
        i -= 1;
        if (text[i] == '\n') {
            line_start = i + 1;
            break;
        }
    }
    const line_end = std.mem.indexOfScalarPos(u8, text, @min(marker_index, text.len), '\n') orelse text.len;
    var count: usize = 0;
    for (text[line_start..line_end]) |c| {
        if (c == marker) {
            count += 1;
        } else if (c != ' ' and c != '\t') {
            return false;
        }
    }
    return count >= 3;
}

pub fn findMatchingOpeningBracket(text: []const u8, close_index: usize) ?usize {
    var depth: usize = 1;
    var i = close_index;
    while (i > 0) {
        i -= 1;
        if (text[i] == ']') {
            depth += 1;
        } else if (text[i] == '[') {
            depth -= 1;
            if (depth == 0) return i;
        }
    }
    return null;
}

pub fn findMatchingClosingBracket(text: []const u8, open_index: usize) ?usize {
    var depth: usize = 1;
    var i = open_index + 1;
    while (i < text.len) : (i += 1) {
        if (text[i] == '[') {
            depth += 1;
        } else if (text[i] == ']') {
            depth -= 1;
            if (depth == 0) return i;
        }
    }
    return null;
}

pub fn lastIndexOf(text: []const u8, needle: []const u8) ?usize {
    return std.mem.lastIndexOf(u8, text, needle);
}

test "matchTrailing mirrors the regexes" {
    try std.testing.expectEqualStrings("bold", matchTrailing("Text **bold", bold).?.content);
    try std.testing.expectEqualStrings("bold*", matchTrailing("**bold*", bold).?.content);
    try std.testing.expect(matchTrailing("**a** b", bold) != null); // parity decides later
    try std.testing.expect(matchTrailing("a b", bold) == null);
    try std.testing.expectEqualStrings("x", matchTrailing("__x_", half_complete_underscore).?.content[0..1]);
    try std.testing.expect(isListItemPrefix("  - "));
    try std.testing.expect(!isListItemPrefix("-"));
}
