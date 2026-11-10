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

const entities = .{
    .{ "amp", "&" },  .{ "lbrace", "{" },    .{ "rbrace", "}" },    .{ "lt", "<" },       .{ "gt", ">" }, .{ "quot", "\"" },
    .{ "apos", "'" }, .{ "nbsp", "\u{a0}" }, .{ "copy", "\u{a9}" }, .{ "reg", "\u{ae}" },
    .{ "mdash", "—" },
    .{ "ndash", "–" },
    .{ "hellip", "…" },
    .{ "middot", "·" },
    .{ "times", "×" },
    .{ "laquo", "«" },
    .{ "raquo", "»" },
    .{ "bull", "•" },
    .{ "lsquo", "‘" },
    .{ "rsquo", "’" },
    .{ "ldquo", "“" },
    .{ "rdquo", "”" },
};

/// HTML entities in JSX text and attribute strings (`&amp;`, `&#169;`).
pub fn decodeEntities(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, raw, '&') == null) return raw;
    var out: std.ArrayList(u8) = .empty;
    var i: usize = 0;
    outer: while (i < raw.len) {
        if (raw[i] == '&') {
            if (std.mem.indexOfScalarPos(u8, raw, i + 1, ';')) |semi| {
                const name = raw[i + 1 .. semi];
                if (name.len > 1 and name[0] == '#') {
                    const cp = if (name[1] == 'x' or name[1] == 'X')
                        std.fmt.parseInt(u21, name[2..], 16) catch null
                    else
                        std.fmt.parseInt(u21, name[1..], 10) catch null;
                    if (cp) |c| {
                        var buf: [4]u8 = undefined;
                        if (std.unicode.utf8Encode(c, &buf)) |n| {
                            try out.appendSlice(arena, buf[0..n]);
                            i = semi + 1;
                            continue :outer;
                        } else |_| {}
                    }
                }
                inline for (entities) |e| {
                    if (std.mem.eql(u8, name, e[0])) {
                        try out.appendSlice(arena, e[1]);
                        i = semi + 1;
                        continue :outer;
                    }
                }
            }
        }
        try out.append(arena, raw[i]);
        i += 1;
    }
    return out.items;
}
