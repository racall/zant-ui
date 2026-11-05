//! Port of Streamdown `lib/table/utils.ts`: table → CSV / TSV / Markdown.
const std = @import("std");

pub const TableData = struct {
    headers: []const []const u8,
    rows: []const []const []const u8,
};

pub const Separator = enum {
    comma,
    semicolon,
    tab,

    fn char(self: Separator) u8 {
        return switch (self) {
            .comma => ',',
            .semicolon => ';',
            .tab => '\t',
        };
    }
};

fn csvCell(out: *std.ArrayListUnmanaged(u8), a: std.mem.Allocator, value: []const u8, sep: u8) !void {
    var needs = false;
    for (value) |c| if (c == sep or c == '"' or c == '\n' or c == '\r') {
        needs = true;
        break;
    };
    if (!needs) return out.appendSlice(a, value);
    try out.append(a, '"');
    for (value) |c| {
        if (c == '"') try out.append(a, '"');
        try out.append(a, c);
    }
    try out.append(a, '"');
}

pub fn toCsv(a: std.mem.Allocator, data: TableData, separator: Separator) ![]u8 {
    const sep = separator.char();
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var first_row = true;
    if (data.headers.len > 0) {
        for (data.headers, 0..) |h, i| {
            if (i > 0) try out.append(a, sep);
            try csvCell(&out, a, h, sep);
        }
        first_row = false;
    }
    for (data.rows) |row| {
        if (!first_row) try out.append(a, '\n');
        first_row = false;
        for (row, 0..) |cell, i| {
            if (i > 0) try out.append(a, sep);
            try csvCell(&out, a, cell, sep);
        }
    }
    return out.toOwnedSlice(a);
}

fn tsvCell(out: *std.ArrayListUnmanaged(u8), a: std.mem.Allocator, value: []const u8) !void {
    for (value) |c| switch (c) {
        '\t' => try out.appendSlice(a, "\\t"),
        '\n' => try out.appendSlice(a, "\\n"),
        '\r' => try out.appendSlice(a, "\\r"),
        else => try out.append(a, c),
    };
}

pub fn toTsv(a: std.mem.Allocator, data: TableData) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var first_row = true;
    if (data.headers.len > 0) {
        for (data.headers, 0..) |h, i| {
            if (i > 0) try out.append(a, '\t');
            try tsvCell(&out, a, h);
        }
        first_row = false;
    }
    for (data.rows) |row| {
        if (!first_row) try out.append(a, '\n');
        first_row = false;
        for (row, 0..) |cell, i| {
            if (i > 0) try out.append(a, '\t');
            try tsvCell(&out, a, cell);
        }
    }
    return out.toOwnedSlice(a);
}

pub fn escapeMarkdownCell(out: *std.ArrayListUnmanaged(u8), a: std.mem.Allocator, cell: []const u8) !void {
    for (cell) |c| switch (c) {
        '\\' => try out.appendSlice(a, "\\\\"),
        '|' => try out.appendSlice(a, "\\|"),
        '\n' => try out.appendSlice(a, "<br>"),
        '&' => try out.appendSlice(a, "&amp;"),
        '<' => try out.appendSlice(a, "&lt;"),
        '>' => try out.appendSlice(a, "&gt;"),
        else => try out.append(a, c),
    };
}

pub fn toMarkdown(a: std.mem.Allocator, data: TableData) ![]u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    if (data.headers.len == 0) return out.toOwnedSlice(a);
    try out.appendSlice(a, "| ");
    for (data.headers, 0..) |h, i| {
        if (i > 0) try out.appendSlice(a, " | ");
        try escapeMarkdownCell(&out, a, h);
    }
    try out.appendSlice(a, " |\n| ");
    for (data.headers, 0..) |_, i| {
        if (i > 0) try out.appendSlice(a, " | ");
        try out.appendSlice(a, "---");
    }
    try out.appendSlice(a, " |");
    for (data.rows) |row| {
        try out.appendSlice(a, "\n| ");
        const n = @max(row.len, data.headers.len);
        for (0..n) |i| {
            if (i > 0) try out.appendSlice(a, " | ");
            if (i < row.len) try escapeMarkdownCell(&out, a, row[i]);
        }
        try out.appendSlice(a, " |");
    }
    return out.toOwnedSlice(a);
}

test "csv / tsv / markdown match streamdown" {
    const a = std.testing.allocator;
    const data: TableData = .{
        .headers = &.{ "Name", "Note" },
        .rows = &.{&.{ "a,b", "say \"hi\"" }},
    };
    const csv = try toCsv(a, data, .comma);
    defer a.free(csv);
    try std.testing.expectEqualStrings("Name,Note\n\"a,b\",\"say \"\"hi\"\"\"", csv);
    const md = try toMarkdown(a, .{ .headers = &.{"A|B"}, .rows = &.{&.{"x<y"}} });
    defer a.free(md);
    try std.testing.expectEqualStrings("| A\\|B |\n| --- |\n| x&lt;y |", md);
}
