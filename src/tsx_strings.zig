//! UTF-8 storage with JS UTF-16 positions. Results belong to the current UI frame.
const std = @import("std");
const tsx = @import("tsx.zig");
const unicode = @import("core/unicode.zig");

fn copy(s: []const u8) []const u8 {
    const out = tsx.alloc(u8, s.len);
    @memcpy(out, s);
    return out;
}

/// `for (const ch of s)`: the code points of `s` as strings (like JS, which
/// iterates a string by code point); an invalid byte is its own element.
pub fn chars(s: []const u8) []const []const u8 {
    var n: usize = 0;
    var i: usize = 0;
    while (i < s.len) : (n += 1) i += charLen(s, i);
    const out = tsx.alloc([]const u8, n);
    i = 0;
    for (out) |*ch| {
        const len = charLen(s, i);
        ch.* = s[i .. i + len];
        i += len;
    }
    return out;
}

fn charLen(s: []const u8, i: usize) usize {
    const len = std.unicode.utf8ByteSequenceLength(s[i]) catch return 1;
    if (i + len > s.len) return 1;
    _ = std.unicode.utf8Decode(s[i .. i + len]) catch return 1;
    return len;
}

test chars {
    @import("ui.zig").beginFrame();
    defer @import("ui.zig").endFrame();
    const got = chars("a剪\xffb😀");
    try std.testing.expectEqual(@as(usize, 5), got.len);
    try std.testing.expectEqualStrings("剪", got[1]);
    try std.testing.expectEqualStrings("\xff", got[2]);
    try std.testing.expectEqualStrings("😀", got[4]);
}

/// Allocation-free substring search.
pub fn includes(s: []const u8, needle: []const u8) bool {
    return std.mem.indexOf(u8, s, needle) != null;
}

/// Allocation-free prefix check.
pub fn startsWith(s: []const u8, needle: []const u8) bool {
    return std.mem.startsWith(u8, s, needle);
}

/// Allocation-free suffix check.
pub fn endsWith(s: []const u8, needle: []const u8) bool {
    return std.mem.endsWith(u8, s, needle);
}

/// First match in UTF-16 units, or -1 (the same signed type as array indexOf).
pub fn indexOf(s: []const u8, needle: []const u8) isize {
    const byte = std.mem.indexOf(u8, s, needle) orelse return -1;
    return @intCast(unicode.byteToUtf16(s, byte));
}

/// Last match in UTF-16 units, or -1; an empty pattern matches the end.
pub fn lastIndexOf(s: []const u8, needle: []const u8) isize {
    const byte = std.mem.lastIndexOf(u8, s, needle) orelse return -1;
    return @intCast(unicode.byteToUtf16(s, byte));
}

fn whitespace(cp: u21) bool {
    return switch (cp) {
        0x09...0x0d,
        0x20,
        0xa0,
        0x1680,
        0x2000...0x200a,
        0x2028,
        0x2029,
        0x202f,
        0x205f,
        0x3000,
        0xfeff,
        => true,
        else => false,
    };
}

fn trimmed(s: []const u8, start: bool, end: bool) []const u8 {
    var lo: usize = 0;
    var hi = s.len;
    if (start) while (lo < hi) {
        const d = unicode.decodeAt(s, lo);
        if (!whitespace(d.codepoint)) break;
        lo += d.len;
    };
    if (end) while (hi > lo) {
        const prev = unicode.prevCodepoint(s, hi);
        if (!whitespace(unicode.decodeAt(s, prev).codepoint)) break;
        hi = prev;
    };
    return copy(s[lo..hi]);
}

/// Trim ECMAScript WhiteSpace and LineTerminator code points from both ends.
pub fn trim(s: []const u8) []const u8 {
    return trimmed(s, true, true);
}

/// Trim JS whitespace at the start only.
pub fn trimStart(s: []const u8) []const u8 {
    return trimmed(s, true, false);
}

/// Trim JS whitespace at the end only.
pub fn trimEnd(s: []const u8) []const u8 {
    return trimmed(s, false, true);
}

fn asciiCase(s: []const u8, upper: bool) []const u8 {
    const out = tsx.alloc(u8, s.len);
    for (s, out) |b, *dest| dest.* = if (upper) std.ascii.toUpper(b) else std.ascii.toLower(b);
    return out;
}

/// ASCII lowercase only; non-ASCII UTF-8 bytes are unchanged.
pub fn toLowerCase(s: []const u8) []const u8 {
    return asciiCase(s, false);
}

/// ASCII uppercase only; non-ASCII UTF-8 bytes are unchanged.
pub fn toUpperCase(s: []const u8) []const u8 {
    return asciiCase(s, true);
}

fn clamp(value: f64, length: usize) usize {
    if (std.math.isNan(value) or value <= 0) return 0;
    return @intFromFloat(@min(@as(f64, @floatFromInt(length)), @trunc(value)));
}

fn range(s: []const u8, lo: usize, hi: usize) []const u8 {
    return copy(s[unicode.utf16ToByte(s, lo)..unicode.utf16ToByte(s, hi)]);
}

/// Negative offsets count from the end. Both endpoints round down inside surrogate pairs.
pub fn slice(s: []const u8, start: f64, end: ?f64) []const u8 {
    const len = tsx.utf16Length(s);
    const lo = tsx.sliceIndex(@trunc(start), len);
    const hi = if (end) |n| tsx.sliceIndex(@trunc(n), len) else len;
    return range(s, lo, @max(lo, hi));
}

/// Clamp negatives to zero, swap reversed endpoints, and round surrogate boundaries down.
pub fn substring(s: []const u8, start: f64, end: ?f64) []const u8 {
    const len = tsx.utf16Length(s);
    const a = clamp(start, len);
    const b = if (end) |n| clamp(n, len) else len;
    return range(s, @min(a, b), @max(a, b));
}

fn checkedLength(n: f64) usize {
    if (!std.math.isFinite(n) or n >= @as(f64, @floatFromInt(std.math.maxInt(usize)))) {
        @panic("TSX string length out of range");
    }
    return @intFromFloat(@max(0, @trunc(n)));
}

fn add(a: usize, b: usize) usize {
    return std.math.add(usize, a, b) catch @panic("TSX string length out of range");
}

fn multiply(a: usize, b: usize) usize {
    return std.math.mul(usize, a, b) catch @panic("TSX string length out of range");
}

fn pad(s: []const u8, target: f64, fill: []const u8, start: bool) []const u8 {
    const len = tsx.utf16Length(s);
    if (std.math.isNan(target) or target <= @as(f64, @floatFromInt(len)) or fill.len == 0) return copy(s);
    const needed = checkedLength(target) - len;
    const units = tsx.utf16Length(fill);
    const full = needed / units;
    const tail = unicode.utf16ToByte(fill, needed % units);
    const bytes = add(multiply(full, fill.len), tail);
    const out = tsx.alloc(u8, add(s.len, bytes));
    const padding = if (start) out[0..bytes] else out[s.len..];
    for (0..full) |i| @memcpy(padding[i * fill.len ..][0..fill.len], fill);
    @memcpy(padding[full * fill.len ..], fill[0..tail]);
    @memcpy(if (start) out[bytes..] else out[0..s.len], s);
    return out;
}

/// Pad to a UTF-16 length; a truncated fill surrogate pair rounds down.
pub fn padStart(s: []const u8, target: f64, fill: []const u8) []const u8 {
    return pad(s, target, fill, true);
}

/// Pad on the right using the same boundary policy as slice.
pub fn padEnd(s: []const u8, target: f64, fill: []const u8) []const u8 {
    return pad(s, target, fill, false);
}

/// Repeat a truncated count; invalid JS RangeError counts panic in the native runtime.
pub fn repeat(s: []const u8, count: f64) []const u8 {
    const n = if (std.math.isNan(count)) 0 else @trunc(count);
    if (n < 0 or !std.math.isFinite(n)) @panic("TSX repeat count out of range");
    if (s.len == 0 or n == 0) return copy("");
    const times = checkedLength(n);
    const out = tsx.alloc(u8, multiply(s.len, times));
    for (0..times) |i| @memcpy(out[i * s.len ..][0..s.len], s);
    return out;
}

const Buffer = std.ArrayList(u8);
fn append(out: *Buffer, s: []const u8) void {
    out.appendSlice(@import("ui.zig").frameAllocator(), s) catch @panic("TSX string allocation failed");
}

// JS GetSubstitution for string patterns: $$, $&, $`, $'; capture tokens stay literal.
fn substitution(out: *Buffer, s: []const u8, lo: usize, hi: usize, replacement: []const u8) void {
    var i: usize = 0;
    while (i < replacement.len) : (i += 1) {
        if (replacement[i] == '$' and i + 1 < replacement.len) {
            const part: ?[]const u8 = switch (replacement[i + 1]) {
                '$' => "$",
                '&' => s[lo..hi],
                '`' => s[0..lo],
                '\'' => s[hi..],
                else => null,
            };
            if (part) |value| {
                append(out, value);
                i += 1;
                continue;
            }
        }
        append(out, replacement[i .. i + 1]);
    }
}

fn replaced(s: []const u8, pattern: []const u8, replacement: []const u8, all: bool) []const u8 {
    var out: Buffer = .empty;
    var offset: usize = 0;
    if (pattern.len == 0) {
        // Enumerate UTF-16 boundaries, rounding each boundary down like slice.
        substitution(&out, s, 0, 0, replacement);
        if (all) while (offset < s.len) {
            const d = unicode.decodeAt(s, offset);
            if (d.codepoint > 0xffff) substitution(&out, s, offset, offset, replacement);
            append(&out, s[offset .. offset + d.len]);
            offset += d.len;
            substitution(&out, s, offset, offset, replacement);
        };
    } else while (std.mem.indexOfPos(u8, s, offset, pattern)) |at_| {
        append(&out, s[offset..at_]);
        substitution(&out, s, at_, at_ + pattern.len, replacement);
        offset = at_ + pattern.len;
        if (!all) break;
    }
    append(&out, s[offset..]);
    return out.items;
}

/// Replace the first string match, with JS replacement dollar tokens.
pub fn replace(s: []const u8, pattern: []const u8, replacement: []const u8) []const u8 {
    return replaced(s, pattern, replacement, false);
}

/// Replace non-overlapping string matches, including empty patterns.
pub fn replaceAll(s: []const u8, pattern: []const u8, replacement: []const u8) []const u8 {
    return replaced(s, pattern, replacement, true);
}

/// Split on a literal separator; an empty separator splits by Unicode code point.
/// Every returned string, as well as the slice, belongs to the current frame.
pub fn split(s: []const u8, separator: []const u8) []const []const u8 {
    var parts: std.ArrayList([]const u8) = .empty;
    const a = @import("ui.zig").frameAllocator();
    var offset: usize = 0;
    if (separator.len == 0) {
        while (offset < s.len) {
            const end = unicode.nextCodepoint(s, offset);
            parts.append(a, copy(s[offset..end])) catch @panic("TSX split allocation failed");
            offset = end;
        }
    } else {
        while (std.mem.indexOfPos(u8, s, offset, separator)) |at_| {
            parts.append(a, copy(s[offset..at_])) catch @panic("TSX split allocation failed");
            offset = at_ + separator.len;
        }
        parts.append(a, copy(s[offset..])) catch @panic("TSX split allocation failed");
    }
    return parts.items;
}

fn character(s: []const u8, position: f64, negative: bool) ?[]const u8 {
    const n = if (std.math.isNan(position)) 0 else @trunc(position);
    const len: f64 = @floatFromInt(tsx.utf16Length(s));
    const i = if (negative and n < 0) len + n else n;
    if (i < 0 or i >= len) return null;
    const unit: usize = @intFromFloat(i);
    return range(s, unit, unit + 1);
}

/// One UTF-16 unit using rounded UTF-8 boundaries; absent positions return an empty string.
pub fn charAt(s: []const u8, position: f64) []const u8 {
    return character(s, position, false) orelse copy("");
}

/// Negative indices count from the end; absent positions use the compiler's optional model.
pub fn at(s: []const u8, position: f64) ?[]const u8 {
    return character(s, position, true);
}

/// The UTF-16 code unit at `position` (NaN outside the string, like JS).
pub fn charCodeAt(s: []const u8, position: f64) f64 {
    const unit = utf16Unit(s, position) orelse return std.math.nan(f64);
    return @floatFromInt(unit);
}

/// The code point starting at UTF-16 `position` (a trailing surrogate's own
/// value in the middle of a pair, like JS); null outside the string.
pub fn codePointAt(s: []const u8, position: f64) ?f64 {
    const unit = utf16Unit(s, position) orelse return null;
    if (unit < 0xd800 or unit > 0xdbff) return @floatFromInt(unit);
    const low = utf16Unit(s, @trunc(position) + 1) orelse return @floatFromInt(unit);
    if (low < 0xdc00 or low > 0xdfff) return @floatFromInt(unit);
    return @floatFromInt(0x10000 + ((@as(u32, unit) - 0xd800) << 10) + (low - 0xdc00));
}

fn utf16Unit(s: []const u8, position: f64) ?u16 {
    if (std.math.isNan(position)) return utf16Unit(s, 0);
    const p = @trunc(position);
    if (p < 0) return null;
    var index: f64 = 0;
    var view = std.unicode.Utf8View.init(s) catch return null;
    var it = view.iterator();
    while (it.nextCodepoint()) |cp| {
        if (cp >= 0x10000) {
            const v = cp - 0x10000;
            if (index == p) return @intCast(0xd800 + (v >> 10));
            if (index + 1 == p) return @intCast(0xdc00 + (v & 0x3ff));
            index += 2;
        } else {
            if (index == p) return @intCast(cp);
            index += 1;
        }
    }
    return null;
}

test charCodeAt {
    try std.testing.expectEqual(@as(f64, 97), charCodeAt("a|", 0));
    try std.testing.expectEqual(@as(f64, 124), charCodeAt("a|", 1));
    try std.testing.expect(std.math.isNan(charCodeAt("a", 3)));
    try std.testing.expectEqual(@as(f64, 0x526a), charCodeAt("剪", 0));
    try std.testing.expectEqual(@as(f64, 0xd83d), charCodeAt("😀", 0));
    try std.testing.expectEqual(@as(f64, 0xde00), charCodeAt("😀", 1));
    try std.testing.expectEqual(@as(?f64, 0x1f600), codePointAt("😀", 0));
    try std.testing.expectEqual(@as(?f64, 0xde00), codePointAt("😀", 1));
    try std.testing.expectEqual(@as(?f64, null), codePointAt("a", 1));
}

test {
    _ = @import("tsx_strings_test.zig");
}
