//! HTML character reference decoding (`&amp;`, `&#169;`, `&#x1F600;`).
const std = @import("std");

const Named = struct { name: []const u8, value: []const u8 };

const named = [_]Named{
    .{ .name = "amp", .value = "&" },
    .{ .name = "lt", .value = "<" },
    .{ .name = "gt", .value = ">" },
    .{ .name = "quot", .value = "\"" },
    .{ .name = "apos", .value = "'" },
    .{ .name = "nbsp", .value = "\u{00A0}" },
    .{ .name = "copy", .value = "©" },
    .{ .name = "reg", .value = "®" },
    .{ .name = "trade", .value = "™" },
    .{ .name = "hellip", .value = "…" },
    .{ .name = "mdash", .value = "—" },
    .{ .name = "ndash", .value = "–" },
    .{ .name = "lsquo", .value = "‘" },
    .{ .name = "rsquo", .value = "’" },
    .{ .name = "ldquo", .value = "“" },
    .{ .name = "rdquo", .value = "”" },
    .{ .name = "laquo", .value = "«" },
    .{ .name = "raquo", .value = "»" },
    .{ .name = "middot", .value = "·" },
    .{ .name = "bull", .value = "•" },
    .{ .name = "deg", .value = "°" },
    .{ .name = "plusmn", .value = "±" },
    .{ .name = "times", .value = "×" },
    .{ .name = "divide", .value = "÷" },
    .{ .name = "ne", .value = "≠" },
    .{ .name = "le", .value = "≤" },
    .{ .name = "ge", .value = "≥" },
    .{ .name = "larr", .value = "←" },
    .{ .name = "rarr", .value = "→" },
    .{ .name = "uarr", .value = "↑" },
    .{ .name = "darr", .value = "↓" },
    .{ .name = "harr", .value = "↔" },
    .{ .name = "euro", .value = "€" },
    .{ .name = "pound", .value = "£" },
    .{ .name = "yen", .value = "¥" },
    .{ .name = "cent", .value = "¢" },
    .{ .name = "sect", .value = "§" },
    .{ .name = "para", .value = "¶" },
    .{ .name = "frac12", .value = "½" },
    .{ .name = "frac14", .value = "¼" },
    .{ .name = "frac34", .value = "¾" },
    .{ .name = "alpha", .value = "α" },
    .{ .name = "beta", .value = "β" },
    .{ .name = "gamma", .value = "γ" },
    .{ .name = "delta", .value = "δ" },
    .{ .name = "pi", .value = "π" },
    .{ .name = "sigma", .value = "σ" },
    .{ .name = "mu", .value = "μ" },
    .{ .name = "infin", .value = "∞" },
    .{ .name = "check", .value = "✓" },
    .{ .name = "zwj", .value = "\u{200D}" },
    .{ .name = "zwnj", .value = "\u{200C}" },
    .{ .name = "emsp", .value = "\u{2003}" },
    .{ .name = "ensp", .value = "\u{2002}" },
    .{ .name = "thinsp", .value = "\u{2009}" },
};

pub fn lookup(name: []const u8) ?[]const u8 {
    for (named) |entry| {
        if (std.mem.eql(u8, entry.name, name)) return entry.value;
    }
    return null;
}

/// Decodes one reference starting at `text[0] == '&'`. Returns the decoded
/// UTF-8 bytes (written to `buf`) and the consumed length.
pub fn decodeAt(text: []const u8, buf: *[4]u8) ?struct { value: []const u8, len: usize } {
    if (text.len < 3 or text[0] != '&') return null;
    const limit = @min(text.len, 34);
    const semi = std.mem.indexOfScalar(u8, text[0..limit], ';') orelse return null;
    const body = text[1..semi];
    if (body.len == 0) return null;
    if (body[0] == '#') {
        const hex = body.len > 1 and (body[1] == 'x' or body[1] == 'X');
        const digits = if (hex) body[2..] else body[1..];
        if (digits.len == 0 or digits.len > 7) return null;
        const code = std.fmt.parseInt(u21, digits, if (hex) 16 else 10) catch return null;
        const cp: u21 = if (code == 0 or code > 0x10FFFF or (code >= 0xD800 and code <= 0xDFFF)) 0xFFFD else code;
        const n = std.unicode.utf8Encode(cp, buf) catch return null;
        return .{ .value = buf[0..n], .len = semi + 1 };
    }
    for (body) |c| if (!std.ascii.isAlphanumeric(c)) return null;
    const value = lookup(body) orelse return null;
    return .{ .value = value, .len = semi + 1 };
}

/// Returns `text` with all character references decoded. Returns `text`
/// itself when nothing needs decoding.
pub fn decode(allocator: std.mem.Allocator, text: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, text, '&') == null) return text;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var buf: [4]u8 = undefined;
    var i: usize = 0;
    while (i < text.len) {
        if (text[i] == '&') {
            if (decodeAt(text[i..], &buf)) |hit| {
                try out.appendSlice(allocator, hit.value);
                i += hit.len;
                continue;
            }
        }
        try out.append(allocator, text[i]);
        i += 1;
    }
    return out.toOwnedSlice(allocator);
}

test "named and numeric references" {
    const a = std.testing.allocator;
    const out = try decode(a, "a &amp; b &#169; &#x4E2D; &bogus;");
    defer a.free(out);
    try std.testing.expectEqualStrings("a & b © 中 &bogus;", out);
}
