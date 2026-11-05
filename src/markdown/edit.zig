//! Pure Markdown editing commands used by the editor component: formatting
//! toggles, line prefixes, list continuation on Enter and list indentation.
//! Every command returns the new text plus the selection to apply.
const std = @import("std");
const scan = @import("scan.zig");

pub const Selection = struct {
    anchor: usize,
    cursor: usize,

    pub fn start(self: Selection) usize {
        return @min(self.anchor, self.cursor);
    }
    pub fn end(self: Selection) usize {
        return @max(self.anchor, self.cursor);
    }
    pub fn collapsed(at: usize) Selection {
        return .{ .anchor = at, .cursor = at };
    }
};

pub const Result = struct {
    text: []u8,
    selection: Selection,
};

pub const Command = enum {
    bold,
    italic,
    strikethrough,
    inline_code,
    link,
    heading,
    quote,
    bullet_list,
    ordered_list,
    task_list,
    code_block,
    inline_math,
    math_block,
    rule,
    table,

    pub fn label(self: Command) []const u8 {
        return switch (self) {
            .bold => "B",
            .italic => "I",
            .strikethrough => "S",
            .inline_code => "`",
            .link => "🔗",
            .heading => "H",
            .quote => "❝",
            .bullet_list => "•",
            .ordered_list => "1.",
            .task_list => "☑",
            .code_block => "{}",
            .inline_math => "∑",
            .math_block => "$$",
            .rule => "—",
            .table => "▦",
        };
    }
};

fn clampSel(text: []const u8, sel: Selection) Selection {
    return .{ .anchor = @min(sel.anchor, text.len), .cursor = @min(sel.cursor, text.len) };
}

fn splice(a: std.mem.Allocator, text: []const u8, start: usize, end: usize, insert: []const u8) ![]u8 {
    return std.mem.concat(a, u8, &.{ text[0..start], insert, text[end..] });
}

/// Toggles `open`…`close` around the selection (or inserts an empty pair).
pub fn wrap(a: std.mem.Allocator, text: []const u8, sel_in: Selection, open: []const u8, close: []const u8) !Result {
    const sel = clampSel(text, sel_in);
    const s = sel.start();
    const e = sel.end();
    // already wrapped outside the selection → unwrap
    if (s >= open.len and e + close.len <= text.len and
        std.mem.eql(u8, text[s - open.len .. s], open) and std.mem.eql(u8, text[e .. e + close.len], close))
    {
        const out = try std.mem.concat(a, u8, &.{ text[0 .. s - open.len], text[s..e], text[e + close.len ..] });
        return .{ .text = out, .selection = .{ .anchor = s - open.len, .cursor = e - open.len } };
    }
    // selection includes the markers → unwrap
    const inner = text[s..e];
    if (inner.len >= open.len + close.len and std.mem.startsWith(u8, inner, open) and std.mem.endsWith(u8, inner, close)) {
        const body = inner[open.len .. inner.len - close.len];
        const out = try splice(a, text, s, e, body);
        return .{ .text = out, .selection = .{ .anchor = s, .cursor = s + body.len } };
    }
    const out = try std.mem.concat(a, u8, &.{ text[0..s], open, inner, close, text[e..] });
    return .{ .text = out, .selection = .{ .anchor = s + open.len, .cursor = e + open.len } };
}

fn lineStart(text: []const u8, at: usize) usize {
    var i = @min(at, text.len);
    while (i > 0 and text[i - 1] != '\n') i -= 1;
    return i;
}

fn lineEnd(text: []const u8, at: usize) usize {
    return std.mem.indexOfScalarPos(u8, text, @min(at, text.len), '\n') orelse text.len;
}

const Prefix = enum { quote, bullet, ordered, task };

fn existingPrefix(line: []const u8, kind: Prefix) ?usize {
    const indent = scan.indentOf(line);
    const t = line[indent..];
    switch (kind) {
        .quote => return if (std.mem.startsWith(u8, t, "> ")) indent + 2 else if (std.mem.startsWith(u8, t, ">")) indent + 1 else null,
        .task => {
            const m = scan.listMarker(line) orelse return null;
            if (m.ordered) return null;
            if (std.mem.startsWith(u8, m.rest, "[ ] ") or std.mem.startsWith(u8, m.rest, "[x] ") or std.mem.startsWith(u8, m.rest, "[X] ")) return m.content_indent + 4;
            return null;
        },
        .bullet, .ordered => {
            const m = scan.listMarker(line) orelse return null;
            if (m.ordered != (kind == .ordered)) return null;
            if (kind == .bullet and (std.mem.startsWith(u8, m.rest, "[ ] ") or std.mem.startsWith(u8, m.rest, "[x] "))) return null;
            return m.content_indent;
        },
    }
}

/// Toggles a line prefix on every line touched by the selection.
fn togglePrefix(a: std.mem.Allocator, text: []const u8, sel_in: Selection, kind: Prefix) !Result {
    const sel = clampSel(text, sel_in);
    const first = lineStart(text, sel.start());
    const last_end = lineEnd(text, sel.end());
    // all lines already prefixed → remove
    var all = true;
    var it = std.mem.splitScalar(u8, text[first..last_end], '\n');
    while (it.next()) |line| {
        if (existingPrefix(line, kind) == null and line.len > 0) all = false;
    }
    var out: std.ArrayListUnmanaged(u8) = .empty;
    try out.appendSlice(a, text[0..first]);
    var number: usize = 1;
    var delta_first: isize = 0;
    var delta_total: isize = 0;
    var index: usize = 0;
    it = std.mem.splitScalar(u8, text[first..last_end], '\n');
    while (it.next()) |line| : (index += 1) {
        if (index > 0) try out.append(a, '\n');
        var d: isize = 0;
        if (all) {
            if (existingPrefix(line, kind)) |p| {
                try out.appendSlice(a, line[p..]);
                d = -@as(isize, @intCast(p));
            } else try out.appendSlice(a, line);
        } else {
            // replace a different list marker instead of stacking
            var body = line;
            if (kind != .quote) {
                if (scan.listMarker(line)) |m| {
                    body = m.rest;
                    if (std.mem.startsWith(u8, body, "[ ] ") or std.mem.startsWith(u8, body, "[x] ")) body = body[4..];
                    d -= @intCast(line.len - body.len);
                }
            }
            const prefix = switch (kind) {
                .quote => "> ",
                .bullet => "- ",
                .task => "- [ ] ",
                .ordered => try std.fmt.allocPrint(a, "{d}. ", .{number}),
            };
            number += 1;
            try out.appendSlice(a, prefix);
            try out.appendSlice(a, body);
            d += @intCast(prefix.len);
        }
        if (index == 0) delta_first = d;
        delta_total += d;
    }
    try out.appendSlice(a, text[last_end..]);
    const s: isize = @as(isize, @intCast(sel.start())) + delta_first;
    const e: isize = @as(isize, @intCast(sel.end())) + delta_total;
    return .{ .text = try out.toOwnedSlice(a), .selection = .{ .anchor = @intCast(@max(@as(isize, @intCast(first)), s)), .cursor = @intCast(@max(@as(isize, @intCast(first)), e)) } };
}

/// Cycles the heading level of the current line: none → # → ## → ### → none.
fn cycleHeading(a: std.mem.Allocator, text: []const u8, sel_in: Selection) !Result {
    const sel = clampSel(text, sel_in);
    const ls = lineStart(text, sel.cursor);
    const le = lineEnd(text, sel.cursor);
    const line = text[ls..le];
    var level: usize = 0;
    while (level < line.len and line[level] == '#') level += 1;
    const has_space = level < line.len and line[level] == ' ';
    const body = if (level > 0 and has_space) line[level + 1 ..] else if (level > 0 and level == line.len) "" else line;
    const current: usize = if (level > 0 and (has_space or level == line.len)) level else 0;
    const next: usize = if (current >= 3) 0 else current + 1;
    var prefix_buf: [8]u8 = undefined;
    const prefix: []const u8 = if (next == 0) "" else blk: {
        @memset(prefix_buf[0..next], '#');
        prefix_buf[next] = ' ';
        break :blk prefix_buf[0 .. next + 1];
    };
    const out = try std.mem.concat(a, u8, &.{ text[0..ls], prefix, body, text[le..] });
    const old_prefix = line.len - body.len;
    const shift = @as(isize, @intCast(prefix.len)) - @as(isize, @intCast(old_prefix));
    const cur: usize = @intCast(@max(@as(isize, @intCast(ls + prefix.len)), @as(isize, @intCast(sel.cursor)) + shift));
    return .{ .text = out, .selection = Selection.collapsed(cur) };
}

fn insertBlock(a: std.mem.Allocator, text: []const u8, sel_in: Selection, before: []const u8, after: []const u8, placeholder: []const u8) !Result {
    const sel = clampSel(text, sel_in);
    const s = sel.start();
    const e = sel.end();
    const inner = if (e > s) text[s..e] else placeholder;
    // blocks start on their own line
    const lead: []const u8 = if (s > 0 and text[s - 1] != '\n') "\n\n" else if (s > 1 and text[s - 2] != '\n') "\n" else "";
    const trail: []const u8 = if (e < text.len and text[e] != '\n') "\n\n" else "";
    const out = try std.mem.concat(a, u8, &.{ text[0..s], lead, before, inner, after, trail, text[e..] });
    const body_start = s + lead.len + before.len;
    return .{ .text = out, .selection = .{ .anchor = body_start, .cursor = body_start + inner.len } };
}

pub fn apply(a: std.mem.Allocator, text: []const u8, sel: Selection, cmd: Command) !Result {
    return switch (cmd) {
        .bold => wrap(a, text, sel, "**", "**"),
        .italic => wrap(a, text, sel, "*", "*"),
        .strikethrough => wrap(a, text, sel, "~~", "~~"),
        .inline_code => wrap(a, text, sel, "`", "`"),
        .inline_math => wrap(a, text, sel, "$$", "$$"),
        .link => blk: {
            const c = clampSel(text, sel);
            const label = if (c.end() > c.start()) text[c.start()..c.end()] else "链接";
            const out = try std.mem.concat(a, u8, &.{ text[0..c.start()], "[", label, "](https://)", text[c.end()..] });
            const url_start = c.start() + 1 + label.len + 2;
            break :blk .{ .text = out, .selection = .{ .anchor = url_start, .cursor = url_start + "https://".len } };
        },
        .heading => cycleHeading(a, text, sel),
        .quote => togglePrefix(a, text, sel, .quote),
        .bullet_list => togglePrefix(a, text, sel, .bullet),
        .ordered_list => togglePrefix(a, text, sel, .ordered),
        .task_list => togglePrefix(a, text, sel, .task),
        .code_block => insertBlock(a, text, sel, "```\n", "\n```", "code"),
        .math_block => insertBlock(a, text, sel, "$$\n", "\n$$", "E = mc^2"),
        .rule => insertBlock(a, text, sel, "", "", "---"),
        .table => insertBlock(a, text, sel, "| 列 1 | 列 2 |\n| --- | --- |\n| ", " |  |", "内容"),
    };
}

/// Enter inside a list item: continue the list, or end it on an empty item.
/// Returns null when the caret is not in a list item (normal newline).
pub fn continueList(a: std.mem.Allocator, text: []const u8, sel_in: Selection) !?Result {
    const sel = clampSel(text, sel_in);
    if (sel.anchor != sel.cursor) return null;
    const ls = lineStart(text, sel.cursor);
    const le = lineEnd(text, sel.cursor);
    const line = text[ls..le];
    var quote_prefix: []const u8 = "";
    var body = line;
    if (std.mem.startsWith(u8, std.mem.trimStart(u8, line, " "), ">")) {
        const qi = std.mem.indexOfScalar(u8, line, '>').?;
        const end = if (qi + 1 < line.len and line[qi + 1] == ' ') qi + 2 else qi + 1;
        quote_prefix = line[0..end];
        body = line[end..];
    }
    const marker = scan.listMarker(body) orelse {
        if (quote_prefix.len == 0) return null;
        // continue a block quote
        if (std.mem.trim(u8, body, " ").len == 0) {
            const out = try splice(a, text, ls, le, "");
            return .{ .text = out, .selection = Selection.collapsed(ls) };
        }
        const out = try std.mem.concat(a, u8, &.{ text[0..sel.cursor], "\n", quote_prefix, text[sel.cursor..] });
        return .{ .text = out, .selection = Selection.collapsed(sel.cursor + 1 + quote_prefix.len) };
    };
    const content_start = ls + quote_prefix.len + marker.content_indent;
    if (sel.cursor < @min(content_start, le)) return null;
    var rest = marker.rest;
    const is_task = std.mem.startsWith(u8, rest, "[ ] ") or std.mem.startsWith(u8, rest, "[x] ") or std.mem.startsWith(u8, rest, "[X] ") or std.mem.eql(u8, rest, "[ ]");
    if (is_task) rest = if (rest.len >= 4) rest[4..] else "";
    if (std.mem.trim(u8, rest, " ").len == 0) {
        // empty item: remove the marker and stop the list
        const out = try splice(a, text, ls, le, quote_prefix);
        return .{ .text = out, .selection = Selection.collapsed(ls + quote_prefix.len) };
    }
    const indent = body[0..scan.indentOf(body)];
    const next_marker = if (marker.ordered)
        try std.fmt.allocPrint(a, "{s}{d}{c} ", .{ indent, marker.number + 1, marker.delimiter })
    else
        try std.fmt.allocPrint(a, "{s}{c} {s}", .{ indent, marker.bullet, if (is_task) "[ ] " else "" });
    const out = try std.mem.concat(a, u8, &.{ text[0..sel.cursor], "\n", quote_prefix, next_marker, text[sel.cursor..] });
    return .{ .text = out, .selection = Selection.collapsed(sel.cursor + 1 + quote_prefix.len + next_marker.len) };
}

/// Tab / Shift-Tab on list lines: indent or outdent by two spaces.
/// Returns null when the selection is not on list items.
pub fn indentList(a: std.mem.Allocator, text: []const u8, sel_in: Selection, outdent: bool) !?Result {
    const sel = clampSel(text, sel_in);
    const first = lineStart(text, sel.start());
    const last_end = lineEnd(text, sel.end());
    var it = std.mem.splitScalar(u8, text[first..last_end], '\n');
    var any = false;
    while (it.next()) |line| {
        if (scan.listMarker(scan.stripIndent(line, 64)) != null) any = true;
    }
    if (!any) return null;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    try out.appendSlice(a, text[0..first]);
    var delta_first: isize = 0;
    var delta_total: isize = 0;
    var index: usize = 0;
    it = std.mem.splitScalar(u8, text[first..last_end], '\n');
    while (it.next()) |line| : (index += 1) {
        if (index > 0) try out.append(a, '\n');
        var d: isize = 0;
        if (outdent) {
            const n = @min(scan.indentOf(line), 2);
            try out.appendSlice(a, line[n..]);
            d = -@as(isize, @intCast(n));
        } else {
            try out.appendSlice(a, "  ");
            try out.appendSlice(a, line);
            d = 2;
        }
        if (index == 0) delta_first = d;
        delta_total += d;
    }
    try out.appendSlice(a, text[last_end..]);
    const s: usize = @intCast(@max(@as(isize, @intCast(first)), @as(isize, @intCast(sel.start())) + delta_first));
    const e: usize = @intCast(@max(@as(isize, @intCast(first)), @as(isize, @intCast(sel.end())) + delta_total));
    return .{ .text = try out.toOwnedSlice(a), .selection = .{ .anchor = s, .cursor = e } };
}

test "wrap toggles" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const r1 = try apply(a, "hello world", .{ .anchor = 6, .cursor = 11 }, .bold);
    try std.testing.expectEqualStrings("hello **world**", r1.text);
    try std.testing.expectEqual(@as(usize, 8), r1.selection.start());
    const r2 = try apply(a, r1.text, r1.selection, .bold);
    try std.testing.expectEqualStrings("hello world", r2.text);
}

test "list continuation" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const r = (try continueList(a, "1. one", Selection.collapsed(6))).?;
    try std.testing.expectEqualStrings("1. one\n2. ", r.text);
    try std.testing.expectEqual(@as(usize, 10), r.selection.cursor);
    const t = (try continueList(a, "- [x] done", Selection.collapsed(10))).?;
    try std.testing.expectEqualStrings("- [x] done\n- [ ] ", t.text);
    const end = (try continueList(a, "- a\n- ", Selection.collapsed(6))).?;
    try std.testing.expectEqualStrings("- a\n", end.text);
    try std.testing.expect((try continueList(a, "plain", Selection.collapsed(5))) == null);
}

test "line prefixes and headings" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const q = try apply(a, "a\nb", .{ .anchor = 0, .cursor = 3 }, .quote);
    try std.testing.expectEqualStrings("> a\n> b", q.text);
    const o = try apply(a, "- a\n- b", .{ .anchor = 0, .cursor = 7 }, .ordered_list);
    try std.testing.expectEqualStrings("1. a\n2. b", o.text);
    const h1 = try apply(a, "Title", Selection.collapsed(2), .heading);
    try std.testing.expectEqualStrings("# Title", h1.text);
    const h2 = try apply(a, h1.text, h1.selection, .heading);
    try std.testing.expectEqualStrings("## Title", h2.text);
    const ind = (try indentList(a, "- a", Selection.collapsed(3), false)).?;
    try std.testing.expectEqualStrings("  - a", ind.text);
}
