const std = @import("std");

pub const Decoded = struct {
    codepoint: u21,
    len: usize,
};

pub fn decodeAt(text: []const u8, at: usize) Decoded {
    if (at >= text.len) return .{ .codepoint = 0, .len = 0 };
    const len = std.unicode.utf8ByteSequenceLength(text[at]) catch {
        return .{ .codepoint = text[at], .len = 1 };
    };
    if (at + len > text.len) return .{ .codepoint = text[at], .len = 1 };
    const cp = std.unicode.utf8Decode(text[at .. at + len]) catch {
        return .{ .codepoint = text[at], .len = 1 };
    };
    return .{ .codepoint = cp, .len = len };
}

pub fn nextCodepoint(text: []const u8, at: usize) usize {
    const d = decodeAt(text, at);
    return @min(text.len, at + @max(d.len, 1));
}

pub fn prevCodepoint(text: []const u8, at: usize) usize {
    if (at == 0) return 0;
    var i = at - 1;
    while (i > 0 and (text[i] & 0xC0) == 0x80) : (i -= 1) {}
    return i;
}

fn isCombining(cp: u21) bool {
    return (cp >= 0x0300 and cp <= 0x036F) or
        (cp >= 0x1AB0 and cp <= 0x1AFF) or
        (cp >= 0x1DC0 and cp <= 0x1DFF) or
        (cp >= 0x20D0 and cp <= 0x20FF) or
        (cp >= 0xFE00 and cp <= 0xFE0F) or
        (cp >= 0xFE20 and cp <= 0xFE2F) or
        (cp >= 0xE0100 and cp <= 0xE01EF) or
        cp == 0x200D;
}

fn isRegionalIndicator(cp: u21) bool {
    return cp >= 0x1F1E6 and cp <= 0x1F1FF;
}

fn isEmojiModifier(cp: u21) bool {
    return cp >= 0x1F3FB and cp <= 0x1F3FF;
}

fn isExtend(cp: u21) bool {
    return isCombining(cp) or isEmojiModifier(cp) or cp == 0x20E3;
}

/// A pragmatic grapheme-cluster iterator. It handles combining marks, emoji
/// variation selectors, ZWJ sequences, keycaps and regional-indicator flags.
/// It is intentionally small rather than a full UAX #29 implementation, but
/// never splits a UTF-8 code point.
pub fn nextGrapheme(text: []const u8, at: usize) usize {
    if (at >= text.len) return text.len;
    var i = nextCodepoint(text, at);
    var last_cp: u21 = 0x10FFFF;
    var ri_count: usize = 0;
    while (i < text.len) {
        const d = decodeAt(text, i);
        if (d.len == 0) break;
        const cp = d.codepoint;
        if (isExtend(cp)) {
            i += d.len;
            last_cp = cp;
            continue;
        }
        if (last_cp == 0x200D) {
            i += d.len;
            last_cp = cp;
            continue;
        }
        if (isRegionalIndicator(cp)) {
            if (ri_count == 1) {
                i += d.len;
                ri_count = 0;
                last_cp = cp;
                continue;
            }
            if (ri_count == 0) {
                ri_count = 1;
                i += d.len;
                last_cp = cp;
                continue;
            }
        }
        break;
    }
    return i;
}

pub fn prevGrapheme(text: []const u8, at: usize) usize {
    if (at == 0) return 0;
    var i: usize = 0;
    var prev: usize = 0;
    while (i < at) {
        prev = i;
        i = nextGrapheme(text, i);
    }
    return prev;
}

pub fn byteToUtf16(text: []const u8, byte_offset: usize) usize {
    const limit = @min(byte_offset, text.len);
    var i: usize = 0;
    var units: usize = 0;
    while (i < limit) {
        const d = decodeAt(text, i);
        if (d.len == 0 or i + d.len > limit) break;
        units += if (d.codepoint > 0xFFFF) 2 else 1;
        i += d.len;
    }
    return units;
}

pub fn utf16ToByte(text: []const u8, unit_offset: usize) usize {
    var i: usize = 0;
    var units: usize = 0;
    while (i < text.len and units < unit_offset) {
        const d = decodeAt(text, i);
        if (d.len == 0) break;
        const add: usize = if (d.codepoint > 0xFFFF) 2 else 1;
        if (units + add > unit_offset) break;
        units += add;
        i += d.len;
    }
    return i;
}

pub fn isWordCodepoint(cp: u21) bool {
    if (cp < 128) return std.ascii.isAlphanumeric(@intCast(cp)) or cp == '_';
    return !(cp == 0x3000 or (cp >= 0x3001 and cp <= 0x303F) or
        (cp >= 0xFF01 and cp <= 0xFF0F) or (cp >= 0xFF1A and cp <= 0xFF20));
}

pub fn wordStart(text: []const u8, at: usize) usize {
    var i = @min(at, text.len);
    while (i > 0) {
        const p = prevCodepoint(text, i);
        const d = decodeAt(text, p);
        if (!isWordCodepoint(d.codepoint)) break;
        i = p;
    }
    return i;
}

pub fn wordEnd(text: []const u8, at: usize) usize {
    var i = @min(at, text.len);
    while (i < text.len) {
        const d = decodeAt(text, i);
        if (!isWordCodepoint(d.codepoint)) break;
        i += d.len;
    }
    return i;
}

test "grapheme movement never splits UTF-8" {
    const s = "a中e\u{0301}👨‍👩‍👧‍👦";
    var i: usize = 0;
    i = nextGrapheme(s, i);
    try std.testing.expectEqual(@as(usize, 1), i);
    i = nextGrapheme(s, i);
    try std.testing.expectEqual(@as(usize, 4), i);
    i = nextGrapheme(s, i);
    try std.testing.expectEqual(@as(usize, 7), i);
    i = nextGrapheme(s, i);
    try std.testing.expectEqual(s.len, i);
    try std.testing.expectEqual(@as(usize, 7), prevGrapheme(s, s.len));
}

test "utf16 conversion" {
    const s = "A中😀";
    try std.testing.expectEqual(@as(usize, 1), byteToUtf16(s, 1));
    try std.testing.expectEqual(@as(usize, 2), byteToUtf16(s, 4));
    try std.testing.expectEqual(@as(usize, 4), byteToUtf16(s, s.len));
    try std.testing.expectEqual(@as(usize, 4), utf16ToByte(s, 2));
    try std.testing.expectEqual(s.len, utf16ToByte(s, 4));
}
