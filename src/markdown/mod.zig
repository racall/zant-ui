//! Markdown block/inline parser and native Zant UI renderer.
//! The parser is intentionally self-contained: no HTML, JS or DOM.
const std = @import("std");
const ui = @import("../ui.zig");
const zant_node_mod = @import("../core/node.zig");
const math = @import("../math/mod.zig");
const flex = @import("../layout/flex.zig");

const RefMap = std.StringHashMapUnmanaged([]const u8);
var active_refs: ?*RefMap = null;

pub const Style = struct {
    bold: bool = false,
    italic: bool = false,
    code: bool = false,
    strike: bool = false,
    math: bool = false,
    link: bool = false,
};

pub const Span = struct { text: []const u8, style: Style = .{} };

pub const List = struct {
    ordered: bool,
    start: usize,
    items: []const []const Span,
    checked: []const ?bool,
    indents: []const u2 = &.{},
};

pub const Table = struct {
    headers: []const []const Span,
    rows: []const [][]const Span,
    aligns: []const flex.Align = &.{},
};

pub const Block = union(enum) {
    heading: struct { level: u8, spans: []const Span },
    paragraph: []const Span,
    code: struct { lang: []const u8, text: []const u8 },
    quote: []const Block,
    list: List,
    table: Table,
    math: []const u8,
    rule,
};

pub const Document = struct {
    arena: std.heap.ArenaAllocator,
    blocks: []const Block,

    pub fn deinit(self: *Document) void {
        self.arena.deinit();
    }

    pub fn render(self: *const Document) ui.Node {
        var nodes: [256]ui.Node = undefined;
        var count: usize = 0;
        for (self.blocks) |block| {
            if (count >= nodes.len) break;
            nodes[count] = renderBlock(block);
            count += 1;
        }
        return ui.column(.{ .gap = 14, .children = nodes[0..count] });
    }
};

fn appendText(arena: std.mem.Allocator, out: *std.ArrayListUnmanaged(Span), text: []const u8, style: Style) !void {
    if (text.len == 0) return;
    const copy = try arena.dupe(u8, text);
    try out.append(arena, .{ .text = copy, .style = style });
}

fn findClose(text: []const u8, start: usize, needle: []const u8) ?usize {
    return std.mem.indexOfPos(u8, text, start, needle);
}

fn parseInlineStyle(arena: std.mem.Allocator, text: []const u8, base: Style, out: *std.ArrayListUnmanaged(Span)) !void {
    var i: usize = 0;
    var plain_start: usize = 0;
    while (i < text.len) {
        const ch = text[i];
        if (ch == '\\' and i + 1 < text.len) {
            if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
            try appendText(arena, out, text[i + 1 .. i + 2], base);
            i += 2;
            plain_start = i;
            continue;
        }
        if (ch == '`') {
            const end = findClose(text, i + 1, "`") orelse {
                i += 1;
                continue;
            };
            if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
            var style = base;
            style.code = true;
            try appendText(arena, out, text[i + 1 .. end], style);
            i = end + 1;
            plain_start = i;
            continue;
        }
        if (ch == '$' and i + 1 < text.len) {
            const double = text[i + 1] == '$';
            const offset: usize = if (double) 2 else 1;
            const start: usize = i + offset;
            const close = findClose(text, start, if (double) "$$" else "$") orelse {
                i += 1;
                continue;
            };
            if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
            var style = base;
            style.math = true;
            try appendText(arena, out, text[start..close], style);
            i = close + offset;
            plain_start = i;
            continue;
        }
        if ((ch == '*' or ch == '_') and i + 1 < text.len and text[i + 1] == ch) {
            const marker = text[i .. i + 2];
            if (findClose(text, i + 2, marker)) |end| {
                if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                var style = base;
                style.bold = true;
                try parseInlineStyle(arena, text[i + 2 .. end], style, out);
                i = end + 2;
                plain_start = i;
                continue;
            }
        }
        if (ch == '*' or ch == '_') {
            if (findClose(text, i + 1, text[i .. i + 1])) |end| {
                if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                var style = base;
                style.italic = true;
                try parseInlineStyle(arena, text[i + 1 .. end], style, out);
                i = end + 1;
                plain_start = i;
                continue;
            }
        }
        if (ch == '~' and i + 1 < text.len and text[i + 1] == '~') {
            if (findClose(text, i + 2, "~~")) |end| {
                if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                var style = base;
                style.strike = true;
                try parseInlineStyle(arena, text[i + 2 .. end], style, out);
                i = end + 2;
                plain_start = i;
                continue;
            }
        }
        if (ch == '[') {
            if (findClose(text, i + 1, "]")) |label_end| {
                var id = text[i + 1 .. label_end];
                var after = label_end + 1;
                if (after < text.len and text[after] == '[') {
                    if (findClose(text, after + 1, "]")) |id_end| {
                        const explicit = text[after + 1 .. id_end];
                        if (explicit.len > 0) id = explicit;
                        after = id_end + 1;
                    }
                }
                if (active_refs) |refs| {
                    if (refs.get(id)) |_| {
                        if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                        var style = base;
                        style.link = true;
                        try parseInlineStyle(arena, text[i + 1 .. label_end], style, out);
                        i = after;
                        plain_start = i;
                        continue;
                    }
                }
            }
        }
        if (ch == '<') {
            if (findClose(text, i + 1, ">")) |end| {
                const target = text[i + 1 .. end];
                if (std.mem.startsWith(u8, target, "http://") or std.mem.startsWith(u8, target, "https://") or std.mem.startsWith(u8, target, "mailto:")) {
                    if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                    var style = base;
                    style.link = true;
                    try appendText(arena, out, target, style);
                    i = end + 1;
                    plain_start = i;
                    continue;
                }
            }
        }
        if (ch == '!' and i + 1 < text.len and text[i + 1] == '[') {
            if (findClose(text, i + 2, "](")) |label_end| {
                if (findClose(text, label_end + 2, ")")) |url_end| {
                    if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                    var style = base;
                    style.link = true;
                    try parseInlineStyle(arena, text[i + 2 .. label_end], style, out);
                    i = url_end + 1;
                    plain_start = i;
                    continue;
                }
            }
        }
        if (ch == '[') {
            if (findClose(text, i + 1, "](")) |label_end| {
                if (findClose(text, label_end + 2, ")")) |url_end| {
                    if (i > plain_start) try appendText(arena, out, text[plain_start..i], base);
                    var style = base;
                    style.link = true;
                    try parseInlineStyle(arena, text[i + 1 .. label_end], style, out);
                    i = url_end + 1;
                    plain_start = i;
                    continue;
                }
            }
        }
        i += 1;
    }
    if (plain_start < text.len) try appendText(arena, out, text[plain_start..], base);
}

pub fn parseInline(arena: std.mem.Allocator, text: []const u8) ![]const Span {
    var out: std.ArrayListUnmanaged(Span) = .empty;
    try parseInlineStyle(arena, text, .{}, &out);
    return out.toOwnedSlice(arena);
}

fn isBlockStart(line: []const u8) bool {
    const t = std.mem.trimStart(u8, line, " \t");
    return std.mem.startsWith(u8, t, "#") or std.mem.startsWith(u8, t, "```") or
        std.mem.startsWith(u8, t, ">") or std.mem.startsWith(u8, t, "- ") or
        std.mem.startsWith(u8, t, "* ") or std.mem.startsWith(u8, t, "+ ") or
        std.mem.eql(u8, t, "---") or std.mem.eql(u8, t, "***") or
        (t.len > 2 and std.ascii.isDigit(t[0]) and std.mem.indexOfScalar(u8, t, '.') != null);
}

const Definition = struct { id: []const u8, url: []const u8 };

fn parseDefinition(line: []const u8) ?Definition {
    const trimmed = std.mem.trim(u8, line, " \t");
    if (trimmed.len < 4 or trimmed[0] != '[') return null;
    const close = std.mem.indexOf(u8, trimmed, "]:") orelse return null;
    if (close <= 1) return null;
    const id = std.mem.trim(u8, trimmed[1..close], " \t");
    if (id.len == 0) return null;
    var url = std.mem.trim(u8, trimmed[close + 2 ..], " \t");
    if (url.len >= 2 and url[0] == '<' and url[url.len - 1] == '>') url = url[1 .. url.len - 1];
    if (url.len == 0) return null;
    return .{ .id = id, .url = url };
}

fn parseBlocks(arena: std.mem.Allocator, text: []const u8) ![]const Block {
    var lines: std.ArrayListUnmanaged([]const u8) = .empty;
    var it = std.mem.splitScalar(u8, text, '\n');
    while (it.next()) |line| try lines.append(arena, std.mem.trimEnd(u8, line, "\r"));
    if (active_refs) |refs| {
        for (lines.items) |line| {
            if (parseDefinition(line)) |def| {
                refs.put(arena, try arena.dupe(u8, def.id), try arena.dupe(u8, def.url)) catch {};
            }
        }
    }
    var blocks: std.ArrayListUnmanaged(Block) = .empty;
    var i: usize = 0;
    while (i < lines.items.len) {
        const line = lines.items[i];
        if (parseDefinition(line) != null) {
            i += 1;
            continue;
        }
        const trimmed = std.mem.trim(u8, line, " \t");
        if (trimmed.len == 0) {
            i += 1;
            continue;
        }
        if ((line.len >= 4 and std.mem.startsWith(u8, line, "    ")) or (line.len > 0 and line[0] == '\t')) {
            var body: std.ArrayListUnmanaged(u8) = .empty;
            var started = false;
            while (i < lines.items.len) : (i += 1) {
                const raw = lines.items[i];
                const is_indented = (raw.len >= 4 and std.mem.startsWith(u8, raw, "    ")) or (raw.len > 0 and raw[0] == '\t');
                if (std.mem.trim(u8, raw, " \t").len == 0) {
                    if (body.items.len > 0) try body.append(arena, '\n');
                    continue;
                }
                if (!is_indented) break;
                if (started) try body.append(arena, '\n');
                const stripped = if (raw[0] == '\t') raw[1..] else raw[4..];
                try body.appendSlice(arena, stripped);
                started = true;
            }
            if (started) {
                const trimmed_code = std.mem.trimEnd(u8, body.items, "\n");
                try blocks.append(arena, .{ .code = .{ .lang = "", .text = try arena.dupe(u8, trimmed_code) } });
                continue;
            }
        }
        if (std.mem.startsWith(u8, trimmed, "```")) {
            const lang = trimmed[3..];
            var body: std.ArrayListUnmanaged(u8) = .empty;
            i += 1;
            while (i < lines.items.len and !std.mem.startsWith(u8, std.mem.trim(u8, lines.items[i], " \t"), "```")) : (i += 1) {
                if (body.items.len > 0) try body.append(arena, '\n');
                try body.appendSlice(arena, lines.items[i]);
            }
            if (i < lines.items.len) i += 1;
            try blocks.append(arena, .{ .code = .{ .lang = try arena.dupe(u8, lang), .text = try body.toOwnedSlice(arena) } });
            continue;
        }
        if (std.mem.startsWith(u8, trimmed, "$$")) {
            var source: std.ArrayListUnmanaged(u8) = .empty;
            if (trimmed.len > 4 and std.mem.endsWith(u8, trimmed, "$$")) {
                try source.appendSlice(arena, trimmed[2 .. trimmed.len - 2]);
                i += 1;
            } else {
                if (trimmed.len > 2) try source.appendSlice(arena, trimmed[2..]);
                i += 1;
                while (i < lines.items.len) : (i += 1) {
                    const t = std.mem.trim(u8, lines.items[i], " \t");
                    if (std.mem.endsWith(u8, t, "$$")) {
                        if (source.items.len > 0) try source.append(arena, '\n');
                        try source.appendSlice(arena, t[0 .. t.len - 2]);
                        i += 1;
                        break;
                    }
                    if (source.items.len > 0) try source.append(arena, '\n');
                    try source.appendSlice(arena, lines.items[i]);
                }
            }
            try blocks.append(arena, .{ .math = try source.toOwnedSlice(arena) });
            continue;
        }
        if (trimmed.len >= 2 and trimmed[0] == '#') {
            var level: u8 = 0;
            while (level < trimmed.len and trimmed[level] == '#') level += 1;
            if (level <= 6 and level < trimmed.len and trimmed[level] == ' ') {
                var heading_text = std.mem.trim(u8, trimmed[level + 1 ..], " \t");
                while (heading_text.len > 0 and heading_text[heading_text.len - 1] == '#') heading_text = std.mem.trimEnd(u8, heading_text[0 .. heading_text.len - 1], " \t");
                try blocks.append(arena, .{ .heading = .{ .level = level, .spans = try parseInline(arena, heading_text) } });
                i += 1;
                continue;
            }
        }
        if (std.mem.eql(u8, trimmed, "---") or std.mem.eql(u8, trimmed, "***") or std.mem.eql(u8, trimmed, "___")) {
            try blocks.append(arena, .rule);
            i += 1;
            continue;
        }
        if (std.mem.startsWith(u8, trimmed, "> ")) {
            var body: std.ArrayListUnmanaged(u8) = .empty;
            while (i < lines.items.len and std.mem.startsWith(u8, std.mem.trim(u8, lines.items[i], " \t"), ">")) : (i += 1) {
                const t = std.mem.trim(u8, lines.items[i], " \t");
                const rest = if (t.len > 1 and t[1] == ' ') t[2..] else t[1..];
                if (body.items.len > 0) try body.append(arena, '\n');
                try body.appendSlice(arena, rest);
            }
            try blocks.append(arena, .{ .quote = try parseBlocks(arena, body.items) });
            continue;
        }
        if (std.mem.startsWith(u8, trimmed, "- ") or std.mem.startsWith(u8, trimmed, "* ") or std.mem.startsWith(u8, trimmed, "+ ")) {
            var items: std.ArrayListUnmanaged([]const Span) = .empty;
            var checked: std.ArrayListUnmanaged(?bool) = .empty;
            var indents: std.ArrayListUnmanaged(u2) = .empty;
            while (i < lines.items.len) : (i += 1) {
                const raw = lines.items[i];
                var indent: usize = 0;
                while (indent < raw.len and (raw[indent] == ' ' or raw[indent] == '\t')) : (indent += 1) {}
                const t = std.mem.trim(u8, raw, " \t");
                var content: []const u8 = undefined;
                if (std.mem.startsWith(u8, t, "- ") or std.mem.startsWith(u8, t, "* ") or std.mem.startsWith(u8, t, "+ ")) content = t[2..] else break;
                var task: ?bool = null;
                if (std.mem.startsWith(u8, content, "[ ] ")) { task = false; content = content[4..]; }
                if (std.mem.startsWith(u8, content, "[x] ") or std.mem.startsWith(u8, content, "[X] ")) { task = true; content = content[4..]; }
                try items.append(arena, try parseInline(arena, content));
                try checked.append(arena, task);
                const depth: u2 = @intCast(@min(@as(usize, 3), indent / 2));
                try indents.append(arena, depth);
            }
            try blocks.append(arena, .{ .list = .{ .ordered = false, .start = 1, .items = try items.toOwnedSlice(arena), .checked = try checked.toOwnedSlice(arena), .indents = try indents.toOwnedSlice(arena) } });
            continue;
        }
        if (trimmed.len > 2 and std.ascii.isDigit(trimmed[0])) {
            var dot: usize = 0;
            while (dot < trimmed.len and std.ascii.isDigit(trimmed[dot])) dot += 1;
            if (dot < trimmed.len and trimmed[dot] == '.' and dot + 1 < trimmed.len and trimmed[dot + 1] == ' ') {
                const start = std.fmt.parseInt(usize, trimmed[0..dot], 10) catch 1;
                var items: std.ArrayListUnmanaged([]const Span) = .empty;
                var checked: std.ArrayListUnmanaged(?bool) = .empty;
                var indents: std.ArrayListUnmanaged(u2) = .empty;
                while (i < lines.items.len) : (i += 1) {
                    const raw = lines.items[i];
                    var indent: usize = 0;
                    while (indent < raw.len and (raw[indent] == ' ' or raw[indent] == '\t')) : (indent += 1) {}
                    const t = std.mem.trim(u8, raw, " \t");
                    const d = std.mem.indexOfScalar(u8, t, '.') orelse break;
                    if (d == 0 or d + 2 > t.len) break;
                    var ok = true;
                    for (t[0..d]) |c| if (!std.ascii.isDigit(c)) { ok = false; break; };
                    if (!ok) break;
                    try items.append(arena, try parseInline(arena, t[d + 2 ..]));
                    try checked.append(arena, null);
                    const depth: u2 = @intCast(@min(@as(usize, 3), indent / 2));
                    try indents.append(arena, depth);
                }
                try blocks.append(arena, .{ .list = .{ .ordered = true, .start = start, .items = try items.toOwnedSlice(arena), .checked = try checked.toOwnedSlice(arena), .indents = try indents.toOwnedSlice(arena) } });
                continue;
            }
        }
        if (std.mem.indexOfScalar(u8, trimmed, '|') != null and i + 1 < lines.items.len and std.mem.indexOfScalar(u8, lines.items[i + 1], '-') != null) {
            var headers: std.ArrayListUnmanaged([]const Span) = .empty;
            var rows: std.ArrayListUnmanaged([][]const Span) = .empty;
            var header_it = std.mem.splitScalar(u8, std.mem.trim(u8, trimmed, "|"), '|');
            while (header_it.next()) |cell| try headers.append(arena, try parseInline(arena, std.mem.trim(u8, cell, " \t")));
            var aligns: std.ArrayListUnmanaged(flex.Align) = .empty;
            var sep_it = std.mem.splitScalar(u8, std.mem.trim(u8, lines.items[i + 1], "|"), '|');
            while (sep_it.next()) |cell| {
                const spec = std.mem.trim(u8, cell, " \t");
                const left = spec.len > 0 and spec[0] == ':';
                const right = spec.len > 1 and spec[spec.len - 1] == ':';
                try aligns.append(arena, if (left and right) .center else if (right) .end else .start);
            }
            i += 2;
            while (i < lines.items.len and std.mem.indexOfScalar(u8, lines.items[i], '|') != null) : (i += 1) {
                var cells: std.ArrayListUnmanaged([]const Span) = .empty;
                var row_it = std.mem.splitScalar(u8, std.mem.trim(u8, lines.items[i], "|"), '|');
                while (row_it.next()) |cell| try cells.append(arena, try parseInline(arena, std.mem.trim(u8, cell, " \t")));
                try rows.append(arena, try cells.toOwnedSlice(arena));
            }
            try blocks.append(arena, .{ .table = .{
                .headers = try headers.toOwnedSlice(arena),
                .rows = try rows.toOwnedSlice(arena),
                .aligns = try aligns.toOwnedSlice(arena),
            } });
            continue;
        }
        if (i + 1 < lines.items.len and trimmed.len > 0) {
            const underline = std.mem.trim(u8, lines.items[i + 1], " \t");
            if (underline.len > 0) {
                var all_eq = true;
                var all_dash = true;
                for (underline) |c| {
                    if (c != '=') all_eq = false;
                    if (c != '-') all_dash = false;
                }
                if (all_eq or all_dash) {
                    var heading_text = trimmed;
                    while (heading_text.len > 0 and heading_text[heading_text.len - 1] == '#') heading_text = std.mem.trimEnd(u8, heading_text[0 .. heading_text.len - 1], " \t");
                    try blocks.append(arena, .{ .heading = .{ .level = if (all_eq) 1 else 2, .spans = try parseInline(arena, heading_text) } });
                    i += 2;
                    continue;
                }
            }
        }
        var body: std.ArrayListUnmanaged(u8) = .empty;
        while (i < lines.items.len) {
            const t = std.mem.trim(u8, lines.items[i], " \t");
            if (t.len == 0) break;
            if (body.items.len > 0 and isBlockStart(lines.items[i])) break;
            if (body.items.len > 0) try body.append(arena, '\n');
            try body.appendSlice(arena, lines.items[i]);
            i += 1;
        }
        try blocks.append(arena, .{ .paragraph = try parseInline(arena, body.items) });
    }
    return blocks.toOwnedSlice(arena);
}

pub fn parse(allocator: std.mem.Allocator, text: []const u8) !Document {
    var arena = std.heap.ArenaAllocator.init(allocator);
    errdefer arena.deinit();
    var refs: RefMap = .empty;
    active_refs = &refs;
    defer active_refs = null;
    const blocks = try parseBlocks(arena.allocator(), text);
    return .{ .arena = arena, .blocks = blocks };
}

fn spanColor(span: Span) u32 {
    if (span.style.link) return 0xff607d6b;
    if (span.style.code or span.style.math) return 0xff3c4a3e;
    return 0xff292d2a;
}

fn makeRich(spans: []const Span, size: f32, out: *[96]zant_node_mod.RichSpan) []const zant_node_mod.RichSpan {
    var count: usize = 0;
    for (spans) |span| {
        if (count >= out.len) break;
        out[count] = .{
            .text = span.text,
            .font_size = if (span.style.code or span.style.math) size - 1 else size,
            .weight = if (span.style.bold) .semibold else .regular,
            .color = spanColor(span),
            .italic = span.style.italic,
        };
        count += 1;
    }
    return out[0..count];
}

fn spansNode(spans: []const Span, size: f32) ui.Node {
    if (spans.len == 0) return ui.space(.{ .height = 1 });
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    var rendered: [96]Span = undefined;
    var rendered_len: usize = 0;
    for (spans) |span| {
        if (rendered_len >= rendered.len) break;
        var copy = span;
        if (span.style.math) {
            copy.text = math.inlineText(arena.allocator(), span.text) catch span.text;
        }
        rendered[rendered_len] = copy;
        rendered_len += 1;
    }
    var rich: [96]zant_node_mod.RichSpan = undefined;
    return ui.richText(.{ .spans = makeRich(rendered[0..rendered_len], size, &rich) });
}

fn spansRow(spans: []const Span, size: f32) ui.Node {
    if (spans.len == 0) return ui.space(.{ .height = 1 });
    var children: [64]ui.Node = undefined;
    var count: usize = 0;
    for (spans) |span| {
        if (count >= children.len) break;
        children[count] = ui.text(span.text, .{
            .size = size,
            .weight = if (span.style.bold) .semibold else .regular,
            .color = spanColor(span),
        });
        count += 1;
    }
    return ui.row(.{ .gap = 0, .align_items = .start, .children = children[0..count] });
}

fn renderBlock(block: Block) ui.Node {
    return switch (block) {
        .heading => |h| ui.column(.{ .gap = 4, .children = &.{
            spansNode(h.spans, switch (h.level) { 1 => 30, 2 => 23, 3 => 18, else => 16 }),
        } }),
        .paragraph => |spans| spansNode(spans, 15),
        .code => |code| ui.styled(ui.paragraph(code.text, .{ .size = 13, .color = 0xff3c4a3e }), .{
            .background = 0xfff0f2ed,
            .radius = 7,
            .padding = .{ .left = 14, .right = 14, .top = 10, .bottom = 10 },
        }),
        .quote => |quote_blocks| blk: {
            var children: [32]ui.Node = undefined;
            var count: usize = 0;
            for (quote_blocks) |item| {
                if (count >= children.len) break;
                children[count] = renderBlock(item);
                count += 1;
            }
            break :blk ui.styled(ui.column(.{ .gap = 8, .children = children[0..count] }), .{
                .background = 0xfff4f6f1,
                .border_color = 0xff607d6b,
                .border_width = 0,
                .padding = .{ .left = 16, .right = 12, .top = 8, .bottom = 8 },
                .radius = 2,
            });
        },
        .list => |list| blk: {
            var rows: [64]ui.Node = undefined;
            var count: usize = 0;
            for (list.items, 0..) |item, index| {
                if (count >= rows.len) break;
                var combined: [97]Span = undefined;
                var combined_len: usize = 0;
                const marker = if (list.ordered) std.fmt.allocPrint(std.heap.c_allocator, "{d}. ", .{list.start + index}) catch "• " else if (list.checked[index]) |checked| (if (checked) "☑ " else "☐ ") else "• ";
                defer if (list.ordered) std.heap.c_allocator.free(marker);
                combined[0] = .{ .text = marker, .style = .{ .link = false } };
                combined_len = 1;
                for (item) |span| {
                    if (combined_len >= combined.len) break;
                    combined[combined_len] = span;
                    combined_len += 1;
                }
                var row_node = spansNode(combined[0..combined_len], 15);
                const depth = if (index < list.indents.len) list.indents[index] else 0;
                if (depth > 0) {
                    row_node = ui.styled(row_node, .{ .padding = .{ .left = @as(f32, @floatFromInt(depth)) * 16 } });
                }
                rows[count] = row_node;
                count += 1;
            }
            break :blk ui.column(.{ .gap = 6, .children = rows[0..count] });
        },
        .table => |table| blk: {
            var rows: [64]ui.Node = undefined;
            var count: usize = 0;
            if (count < rows.len) {
                var header_cells: [32]ui.Node = undefined;
                var header_count: usize = 0;
                for (table.headers, 0..) |header, index| {
                    if (header_count >= header_cells.len) break;
                    const alignment = if (index < table.aligns.len) table.aligns[index] else .start;
                    header_cells[header_count] = ui.column(.{
                        .grow = 1,
                        .align_items = alignment,
                        .children = &.{spansRow(header, 13)},
                    });
                    header_count += 1;
                }
                rows[count] = ui.styled(ui.row(.{
                    .width_percent = 100,
                    .gap = 18,
                    .align_items = .start,
                    .children = header_cells[0..header_count],
                }), .{
                    .background = 0xfff4f6f1,
                    .padding = .{ .top = 6, .bottom = 6, .left = 8, .right = 8 },
                    .radius = 5,
                });
                count += 1;
            }
            for (table.rows) |row| {
                if (count >= rows.len) break;
                var cells: [32]ui.Node = undefined;
                var cell_count: usize = 0;
                for (row, 0..) |cell, index| {
                    if (cell_count >= cells.len) break;
                    const alignment = if (index < table.aligns.len) table.aligns[index] else .start;
                    cells[cell_count] = ui.column(.{
                        .grow = 1,
                        .align_items = alignment,
                        .children = &.{spansRow(cell, 13)},
                    });
                    cell_count += 1;
                }
                rows[count] = ui.row(.{
                    .width_percent = 100,
                    .gap = 18,
                    .padding = .{ .left = 8, .right = 8 },
                    .children = cells[0..cell_count],
                });
                count += 1;
            }
            break :blk ui.column(.{ .width_percent = 100, .gap = 4, .children = rows[0..count] });
        },
        .math => |source| math.render(std.heap.c_allocator, source),
        .rule => ui.divider(.{ .margin = .{ .top = 8, .bottom = 8 } }),
    };
}

pub fn render(allocator: std.mem.Allocator, text: []const u8) !ui.Node {
    var doc = try parse(allocator, text);
    defer doc.deinit();
    return doc.render();
}
