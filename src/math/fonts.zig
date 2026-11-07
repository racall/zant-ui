//! Math alphabets (`\mathbb`, `\mathcal`, ...) mapped to Unicode
//! Mathematical Alphanumeric Symbols, which STIX Two Math covers. This is
//! how KaTeX's font commands look without loading extra font files.
const std = @import("std");

pub const Variant = enum {
    /// Default math: Latin letters and lowercase Greek italic, the rest upright.
    normal,
    italic,
    bold,
    bold_italic,
    roman,
    double_struck,
    script,
    fraktur,
    sans_serif,
    sans_serif_bold,
    monospace,
};

pub fn fromCommand(name: []const u8) ?Variant {
    const map = .{
        .{ "mathbb", Variant.double_struck },   .{ "Bbb", Variant.double_struck },
        .{ "mathcal", Variant.script },         .{ "mathscr", Variant.script },
        .{ "mathfrak", Variant.fraktur },       .{ "frak", Variant.fraktur },
        .{ "mathbf", Variant.bold },            .{ "bf", Variant.bold },
        .{ "boldsymbol", Variant.bold_italic }, .{ "bm", Variant.bold_italic },
        .{ "pmb", Variant.bold_italic },        .{ "mathit", Variant.italic },
        .{ "it", Variant.italic },              .{ "mathrm", Variant.roman },
        .{ "rm", Variant.roman },               .{ "mathup", Variant.roman },
        .{ "mathnormal", Variant.normal },      .{ "mathsf", Variant.sans_serif },
        .{ "sf", Variant.sans_serif },          .{ "mathtt", Variant.monospace },
        .{ "tt", Variant.monospace },           .{ "bold", Variant.bold },
        .{ "Bbb", Variant.double_struck },      .{ "cal", Variant.script },
    };
    inline for (map) |entry| if (std.mem.eql(u8, name, entry[0])) return entry[1];
    return null;
}

fn exception(variant: Variant, c: u8) ?u21 {
    return switch (variant) {
        .italic, .normal => if (c == 'h') 0x210E else null,
        .double_struck => switch (c) {
            'C' => 0x2102,
            'H' => 0x210D,
            'N' => 0x2115,
            'P' => 0x2119,
            'Q' => 0x211A,
            'R' => 0x211D,
            'Z' => 0x2124,
            else => null,
        },
        .script => switch (c) {
            'B' => 0x212C,
            'E' => 0x2130,
            'F' => 0x2131,
            'H' => 0x210B,
            'I' => 0x2110,
            'L' => 0x2112,
            'M' => 0x2133,
            'R' => 0x211B,
            'e' => 0x212F,
            'g' => 0x210A,
            'o' => 0x2134,
            else => null,
        },
        .fraktur => switch (c) {
            'C' => 0x212D,
            'H' => 0x210C,
            'I' => 0x2111,
            'R' => 0x211C,
            'Z' => 0x2128,
            else => null,
        },
        else => null,
    };
}

fn letterBase(variant: Variant) ?u21 {
    return switch (variant) {
        .bold => 0x1D400,
        .italic, .normal => 0x1D434,
        .bold_italic => 0x1D468,
        .script => 0x1D49C,
        .fraktur => 0x1D504,
        .double_struck => 0x1D538,
        .sans_serif => 0x1D5A0,
        .sans_serif_bold => 0x1D5D4,
        .monospace => 0x1D670,
        .roman => null,
    };
}

fn digitBase(variant: Variant) ?u21 {
    return switch (variant) {
        .bold, .bold_italic => 0x1D7CE,
        .double_struck => 0x1D7D8,
        .sans_serif => 0x1D7E2,
        .sans_serif_bold => 0x1D7EC,
        .monospace => 0x1D7F6,
        else => null,
    };
}

/// Lowercase Greek α..ω (U+03B1..U+03C9) in math italic.
fn greekItalic(cp: u21, bold: bool) ?u21 {
    const base: u21 = if (bold) 0x1D736 else 0x1D6FC;
    if (cp >= 0x03B1 and cp <= 0x03C9) return base + (cp - 0x03B1);
    return switch (cp) {
        0x03F5 => base + 0x1A - 0x00 + 0, // ϵ lunate epsilon → 𝜖 (base+26)
        0x03D1 => base + 27, // ϑ
        0x03F0 => base + 28, // ϰ
        0x03D5 => base + 29, // ϕ
        0x03F1 => base + 30, // ϱ
        0x03D6 => base + 31, // ϖ
        else => null,
    };
}

/// Maps one code point; returns it unchanged when the alphabet has no form.
pub fn mapCodepoint(variant: Variant, cp: u21) u21 {
    if (cp < 0x80) {
        const c: u8 = @intCast(cp);
        if (std.ascii.isAlphabetic(c)) {
            if (exception(variant, c)) |e| return e;
            const base = letterBase(variant) orelse return cp;
            const offset: u21 = if (std.ascii.isUpper(c)) c - 'A' else 26 + (c - 'a');
            return base + offset;
        }
        if (std.ascii.isDigit(c)) {
            const base = digitBase(variant) orelse return cp;
            return base + (c - '0');
        }
        return cp;
    }
    switch (variant) {
        .normal, .italic => if (greekItalic(cp, false)) |g| return g,
        .bold_italic => if (greekItalic(cp, true)) |g| return g,
        .bold => if (cp >= 0x0391 and cp <= 0x03A9) return 0x1D6A8 + (cp - 0x0391) else if (cp >= 0x03B1 and cp <= 0x03C9) return 0x1D6C2 + (cp - 0x03B1),
        else => {},
    }
    return cp;
}

/// Maps a UTF-8 string into `allocator`.
pub fn mapText(allocator: std.mem.Allocator, variant: Variant, text: []const u8) ![]const u8 {
    if (variant == .roman) return text;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var it = (std.unicode.Utf8View.init(text) catch return text).iterator();
    var buf: [4]u8 = undefined;
    while (it.nextCodepoint()) |cp| {
        const mapped = mapCodepoint(variant, cp);
        const n = std.unicode.utf8Encode(mapped, &buf) catch 0;
        try out.appendSlice(allocator, buf[0..n]);
    }
    return out.toOwnedSlice(allocator);
}

test "alphabets" {
    const a = std.testing.allocator;
    const r = try mapText(a, .double_struck, "RZx1");
    defer a.free(r);
    try std.testing.expectEqualStrings("ℝℤ𝕩𝟙", r);
    const h = try mapText(a, .italic, "hx");
    defer a.free(h);
    try std.testing.expectEqualStrings("ℎ𝑥", h);
}
