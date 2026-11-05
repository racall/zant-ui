//! Line- and character-level recognizers shared by the block parser, the
//! streaming healer and the editor commands.
const std = @import("std");

// ── characters ──────────────────────────────────────────────────────────

pub fn isSpace(c: u8) bool {
    return c == ' ' or c == '\t';
}

pub fn isAsciiPunct(c: u8) bool {
    return switch (c) {
        '!'...'/', ':'...'@', '['...'`', '{'...'~' => true,
        else => false,
    };
}

/// Decodes the code point that ends right before `index` (0 when none).
pub fn cpBefore(text: []const u8, index: usize) u21 {
    if (index == 0) return 0;
    var start = index - 1;
    while (start > 0 and (text[start] & 0xC0) == 0x80) start -= 1;
    return std.unicode.utf8Decode(text[start..index]) catch 0xFFFD;
}

/// Decodes the code point starting at `index` (0 when none).
pub fn cpAt(text: []const u8, index: usize) u21 {
    if (index >= text.len) return 0;
    const len = std.unicode.utf8ByteSequenceLength(text[index]) catch return 0xFFFD;
    if (index + len > text.len) return 0xFFFD;
    return std.unicode.utf8Decode(text[index .. index + len]) catch 0xFFFD;
}

pub fn isCjk(cp: u21) bool {
    return (cp >= 0x2E80 and cp <= 0x9FFF) or // radicals, kana, CJK unified
        (cp >= 0xAC00 and cp <= 0xD7AF) or // hangul
        (cp >= 0xF900 and cp <= 0xFAFF) or
        (cp >= 0xFF00 and cp <= 0xFFEF) or // full-width forms
        (cp >= 0x20000 and cp <= 0x2FA1F);
}

/// CJK and general punctuation (。，、！？「」…).
pub fn isUnicodePunct(cp: u21) bool {
    if (cp < 0x80) return cp != 0 and isAsciiPunct(@intCast(cp));
    return (cp >= 0x2000 and cp <= 0x206F) or (cp >= 0x3000 and cp <= 0x303F) or
        (cp >= 0xFF01 and cp <= 0xFF0F) or (cp >= 0xFF1A and cp <= 0xFF20) or
        (cp >= 0xFF3B and cp <= 0xFF40) or (cp >= 0xFF5B and cp <= 0xFF65);
}

pub fn isWhitespaceCp(cp: u21) bool {
    return cp == 0 or cp == ' ' or cp == '\t' or cp == '\n' or cp == '\r' or cp == 0xA0 or cp == 0x3000;
}

pub fn isWordCp(cp: u21) bool {
    if (cp < 0x80) return std.ascii.isAlphanumeric(@intCast(cp));
    return !isWhitespaceCp(cp) and !isUnicodePunct(cp) and !isCjk(cp);
}

// ── lines ───────────────────────────────────────────────────────────────

pub fn isBlank(line: []const u8) bool {
    for (line) |c| if (!isSpace(c) and c != '\r') return false;
    return true;
}

/// Number of leading spaces (tabs are expanded before parsing).
pub fn indentOf(line: []const u8) usize {
    var n: usize = 0;
    while (n < line.len and line[n] == ' ') n += 1;
    return n;
}

pub fn stripIndent(line: []const u8, max: usize) []const u8 {
    var n: usize = 0;
    while (n < line.len and n < max and line[n] == ' ') n += 1;
    return line[n..];
}

pub fn isThematicBreak(line: []const u8) bool {
    if (indentOf(line) > 3) return false;
    var count: usize = 0;
    var marker: u8 = 0;
    for (line) |c| {
        if (isSpace(c)) continue;
        if (c != '-' and c != '*' and c != '_') return false;
        if (marker == 0) marker = c else if (c != marker) return false;
        count += 1;
    }
    return count >= 3;
}

pub const AtxHeading = struct { level: u8, text: []const u8 };

pub fn atxHeading(line: []const u8) ?AtxHeading {
    if (indentOf(line) > 3) return null;
    const t = std.mem.trimStart(u8, line, " ");
    var level: u8 = 0;
    while (level < t.len and t[level] == '#') level += 1;
    if (level == 0 or level > 6) return null;
    if (level < t.len and !isSpace(t[level])) return null;
    var body = std.mem.trim(u8, t[level..], " \t");
    // optional closing sequence: "## Title ##"
    var end = body.len;
    while (end > 0 and body[end - 1] == '#') end -= 1;
    if (end == 0) {
        body = "";
    } else if (end < body.len and isSpace(body[end - 1])) {
        body = std.mem.trimEnd(u8, body[0..end], " \t");
    }
    return .{ .level = level, .text = body };
}

/// `=` → 1, `-` → 2 for a setext underline, else null.
pub fn setextLevel(line: []const u8) ?u8 {
    if (indentOf(line) > 3) return null;
    const t = std.mem.trim(u8, line, " \t");
    if (t.len == 0) return null;
    const c = t[0];
    if (c != '=' and c != '-') return null;
    for (t) |x| if (x != c) return null;
    return if (c == '=') 1 else 2;
}

pub const Fence = struct {
    char: u8,
    len: usize,
    indent: usize,
    info: []const u8,
};

pub fn fenceOpen(line: []const u8) ?Fence {
    const indent = indentOf(line);
    if (indent > 3) return null;
    const t = line[indent..];
    if (t.len < 3 or (t[0] != '`' and t[0] != '~')) return null;
    const c = t[0];
    var n: usize = 0;
    while (n < t.len and t[n] == c) n += 1;
    if (n < 3) return null;
    const info = std.mem.trim(u8, t[n..], " \t");
    if (c == '`' and std.mem.indexOfScalar(u8, info, '`') != null) return null;
    return .{ .char = c, .len = n, .indent = indent, .info = info };
}

pub fn isFenceClose(line: []const u8, open: Fence) bool {
    const indent = indentOf(line);
    if (indent > 3) return false;
    const t = std.mem.trimEnd(u8, line[indent..], " \t");
    if (t.len < open.len) return false;
    for (t) |c| if (c != open.char) return false;
    return true;
}

pub const ListMarker = struct {
    ordered: bool,
    bullet: u8 = 0,
    delimiter: u8 = 0,
    number: usize = 1,
    /// Columns from line start to the item content.
    content_indent: usize,
    /// Item text after the marker (may be empty).
    rest: []const u8,
};

pub fn listMarker(line: []const u8) ?ListMarker {
    const indent = indentOf(line);
    if (indent > 3) return null;
    const t = line[indent..];
    if (t.len == 0) return null;
    var marker_len: usize = 0;
    var result: ListMarker = .{ .ordered = false, .content_indent = 0, .rest = "" };
    if (t[0] == '-' or t[0] == '*' or t[0] == '+') {
        marker_len = 1;
        result.bullet = t[0];
    } else if (std.ascii.isDigit(t[0])) {
        var d: usize = 0;
        while (d < t.len and d < 9 and std.ascii.isDigit(t[d])) d += 1;
        if (d >= t.len or (t[d] != '.' and t[d] != ')')) return null;
        result.ordered = true;
        result.delimiter = t[d];
        result.number = std.fmt.parseInt(usize, t[0..d], 10) catch 1;
        marker_len = d + 1;
    } else return null;
    const after = t[marker_len..];
    if (after.len > 0 and !isSpace(after[0])) return null;
    var spaces: usize = 0;
    while (spaces < after.len and after[spaces] == ' ') spaces += 1;
    // 5+ spaces: content is an indented code block; content starts after 1.
    if (spaces > 4 or spaces == after.len) spaces = @min(spaces, 1);
    result.content_indent = indent + marker_len + @max(spaces, 1);
    result.rest = if (spaces < after.len) after[spaces..] else "";
    return result;
}

pub fn isHtmlBlockStart(line: []const u8) bool {
    if (indentOf(line) > 3) return false;
    const t = std.mem.trimStart(u8, line, " ");
    if (t.len < 2 or t[0] != '<') return false;
    if (std.mem.startsWith(u8, t, "<!--")) return true;
    if (std.mem.startsWith(u8, t, "<http") or std.mem.startsWith(u8, t, "<mailto:")) return false;
    var i: usize = 1;
    if (t[i] == '/') i += 1;
    if (i >= t.len or !std.ascii.isAlphabetic(t[i])) return false;
    var j = i;
    while (j < t.len and (std.ascii.isAlphanumeric(t[j]) or t[j] == '-')) j += 1;
    // must look like a tag: <div>, <div ...>, <br/>, </div>
    if (j < t.len and t[j] != '>' and t[j] != ' ' and t[j] != '/' and t[j] != '\t') return false;
    // autolink-ish "<foo@bar>" is not a block
    if (std.mem.indexOfScalar(u8, t[0..@min(t.len, j + 1)], '@') != null) return false;
    return true;
}

pub fn isMathFence(line: []const u8) bool {
    const t = std.mem.trim(u8, line, " \t");
    return std.mem.startsWith(u8, t, "$$") or std.mem.eql(u8, t, "\\[");
}

/// Splits a GFM table row into trimmed cells, honouring `\|` escapes and
/// pipes inside code spans.
pub fn splitTableRow(allocator: std.mem.Allocator, line: []const u8) ![]const []const u8 {
    var t = std.mem.trim(u8, line, " \t");
    if (t.len > 0 and t[0] == '|') t = t[1..];
    if (t.len > 0 and t[t.len - 1] == '|' and !(t.len > 1 and t[t.len - 2] == '\\')) t = t[0 .. t.len - 1];
    var cells: std.ArrayListUnmanaged([]const u8) = .empty;
    var cell: std.ArrayListUnmanaged(u8) = .empty;
    var in_code = false;
    var i: usize = 0;
    while (i < t.len) : (i += 1) {
        const c = t[i];
        if (c == '\\' and i + 1 < t.len and t[i + 1] == '|') {
            try cell.append(allocator, '|');
            i += 1;
            continue;
        }
        if (c == '`') in_code = !in_code;
        if (c == '|' and !in_code) {
            try cells.append(allocator, std.mem.trim(u8, try cell.toOwnedSlice(allocator), " \t"));
            continue;
        }
        try cell.append(allocator, c);
    }
    try cells.append(allocator, std.mem.trim(u8, try cell.toOwnedSlice(allocator), " \t"));
    return cells.toOwnedSlice(allocator);
}

/// Parses a delimiter row (`| :--- | ---: |`). Returns null if invalid.
pub fn tableDelimiter(allocator: std.mem.Allocator, line: []const u8) !?[]const @import("ast.zig").Align {
    if (std.mem.indexOfScalar(u8, line, '-') == null) return null;
    const t = std.mem.trim(u8, line, " \t");
    if (t.len == 0) return null;
    const cells = try splitTableRow(allocator, t);
    if (cells.len == 0) return null;
    // a single column needs an explicit pipe to avoid matching setext/rules
    if (cells.len == 1 and std.mem.indexOfScalar(u8, t, '|') == null) return null;
    const out = try allocator.alloc(@import("ast.zig").Align, cells.len);
    for (cells, 0..) |cell, i| {
        if (cell.len == 0) return null;
        const left = cell[0] == ':';
        const right = cell[cell.len - 1] == ':';
        const body = cell[@intFromBool(left) .. cell.len - @intFromBool(right and cell.len > 1)];
        if (body.len == 0) return null;
        for (body) |c| if (c != '-') return null;
        out[i] = if (left and right) .center else if (right) .right else if (left) .left else .none;
    }
    return out;
}

/// Expands tabs in the leading whitespace to 4-column stops.
pub fn expandLeadingTabs(allocator: std.mem.Allocator, line: []const u8) ![]const u8 {
    var i: usize = 0;
    var has_tab = false;
    while (i < line.len and isSpace(line[i])) : (i += 1) {
        if (line[i] == '\t') has_tab = true;
    }
    if (!has_tab) return line;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var col: usize = 0;
    for (line[0..i]) |c| {
        if (c == '\t') {
            const n = 4 - (col % 4);
            try out.appendNTimes(allocator, ' ', n);
            col += n;
        } else {
            try out.append(allocator, ' ');
            col += 1;
        }
    }
    try out.appendSlice(allocator, line[i..]);
    return out.toOwnedSlice(allocator);
}

test "list markers" {
    const m = listMarker("  12) item").?;
    try std.testing.expect(m.ordered);
    try std.testing.expectEqual(@as(usize, 12), m.number);
    try std.testing.expectEqual(@as(usize, 6), m.content_indent);
    try std.testing.expectEqualStrings("item", m.rest);
    try std.testing.expect(listMarker("-not") == null);
    try std.testing.expect(listMarker("* ").?.rest.len == 0);
}

test "fences and headings" {
    const f = fenceOpen("~~~~ zig title=a").?;
    try std.testing.expectEqual(@as(usize, 4), f.len);
    try std.testing.expect(isFenceClose("~~~~~", f));
    try std.testing.expect(!isFenceClose("~~~", f));
    try std.testing.expectEqualStrings("Title", atxHeading("## Title ##").?.text);
    try std.testing.expect(atxHeading("#Title") == null);
    try std.testing.expectEqualStrings("C#", atxHeading("# C#").?.text);
}
