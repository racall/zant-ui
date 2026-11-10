//! JSX text: React's whitespace rule and HTML entities.
const std = @import("std");

/// React's JSX text rule (Babel `cleanJSXElementLiteralChild`): lines are
/// trimmed except at the outer edges, whitespace-only lines vanish and the
/// remaining lines join with one space. Null when nothing is left.
pub fn cleanJsxText(arena: std.mem.Allocator, value: []const u8) !?[]const u8 {
    var lines: std.ArrayList([]const u8) = .empty;
    var it = std.mem.splitScalar(u8, value, '\n');
    while (it.next()) |line| try lines.append(arena, std.mem.trimEnd(u8, line, "\r"));
    var last_non_empty: usize = 0; // Babel's default
    for (lines.items, 0..) |line, i| {
        if (std.mem.indexOfNone(u8, line, " \t") != null) last_non_empty = i;
    }
    var out: std.ArrayList(u8) = .empty;
    for (lines.items, 0..) |line, i| {
        var trimmed = line;
        if (i != 0) trimmed = std.mem.trimStart(u8, trimmed, " \t");
        if (i != lines.items.len - 1) trimmed = std.mem.trimEnd(u8, trimmed, " \t");
        if (trimmed.len == 0) continue;
        for (trimmed) |c| try out.append(arena, if (c == '\t') ' ' else c);
        if (i != last_non_empty) try out.append(arena, ' ');
    }
    if (out.items.len == 0) return null;
    return out.items;
}

const entities = @import("jsx_entities.zig").table;
const Entity = struct { cp: u21, len: usize };

/// Decode one reference at the start, following SWC's `decode_jsx_entity`.
fn entity(raw: []const u8) ?Entity {
    if (raw.len < 3 or raw[0] != '&') return null;
    var i: usize = 1;
    if (raw[i] == '#') {
        i += 1;
        const radix: u32 = if (i < raw.len and raw[i] == 'x') blk: {
            i += 1;
            break :blk 16;
        } else 10;
        const start = i;
        var value: u32 = 0;
        while (i < raw.len) : (i += 1) {
            const ch = raw[i];
            if (ch == ';' and i > start) return .{ .cp = @intCast(value), .len = i + 1 };
            const digit: u32 = switch (ch) {
                '0'...'9' => ch - '0',
                'a'...'f' => if (radix == 16) ch - 'a' + 10 else return null,
                'A'...'F' => if (radix == 16) ch - 'A' + 10 else return null,
                else => return null,
            };
            // Values cannot shrink: this also prevents integer overflow on arbitrarily long inputs.
            if (value > (0x10ffff - digit) / radix) return null;
            value = value * radix + digit;
        }
        return null;
    }
    while (i < raw.len and std.ascii.isAlphanumeric(raw[i])) : (i += 1) {}
    if (i >= raw.len or raw[i] != ';') return null;
    const cp = entities.get(raw[1..i]) orelse return null;
    return .{ .cp = cp, .len = i + 1 };
}

/// XHTML entities in JSX text and attributes. Invalid references stay literal.
/// Like SWC's JS strings, lone surrogates use WTF-8; adjacent pairs combine.
pub fn decodeEntities(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, raw, '&') == null) return raw;
    var out: std.ArrayList(u8) = .empty;
    var i: usize = 0;
    while (i < raw.len) {
        if (entity(raw[i..])) |decoded| {
            i += decoded.len;
            var cp = decoded.cp;
            if (cp >= 0xd800 and cp <= 0xdbff) {
                if (entity(raw[i..])) |next| {
                    if (next.cp >= 0xdc00 and next.cp <= 0xdfff) {
                        cp = 0x10000 + (@as(u21, cp - 0xd800) << 10) + (next.cp - 0xdc00);
                        i += next.len;
                    }
                }
            }
            var buf: [4]u8 = undefined;
            const n = std.unicode.wtf8Encode(cp, &buf) catch unreachable;
            try out.appendSlice(arena, buf[0..n]);
        } else {
            try out.append(arena, raw[i]);
            i += 1;
        }
    }
    return out.items;
}
