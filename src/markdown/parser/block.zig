//! Block-level Markdown parser (CommonMark + GFM): headings, paragraphs,
//! fenced/indented code, block quotes and GFM alerts, nested lists with task
//! items, tables, math, HTML blocks, footnote and link definitions, front
//! matter.
//!
//! Container blocks are parsed recursively on their stripped lines. The
//! top-level pass can also report the source line range of every block,
//! which `blocks.zig` (Streamdown's parse-blocks) uses to split documents.
const std = @import("std");
const ast = @import("../ast.zig");
const scan = @import("../scan.zig");
const inline_parser = @import("inline.zig");

const Block = ast.Block;

pub const Options = struct {
    /// `$x$` inline math with pandoc's rules (see inline.zig), on by default.
    single_dollar_math: bool = true,
    /// Recognise YAML front matter at the very start of the document.
    front_matter: bool = true,
};

pub const LineRange = struct { start: usize, end: usize };

const FootnoteDef = struct { label: []const u8, blocks: []const Block };

pub const Parser = struct {
    arena: std.mem.Allocator,
    options: Options,
    refs: inline_parser.RefMap = .empty,
    footnote_order: std.ArrayListUnmanaged([]const u8) = .empty,
    footnote_defs: std.ArrayListUnmanaged(FootnoteDef) = .empty,
    slugs: std.StringHashMapUnmanaged(usize) = .empty,
    inline_ctx: inline_parser.Context = undefined,
    /// When set, inline content is not parsed (block splitting only).
    structure_only: bool = false,

    pub fn init(arena: std.mem.Allocator, options: Options) Parser {
        return .{ .arena = arena, .options = options };
    }

    fn ctx(self: *Parser) *inline_parser.Context {
        self.inline_ctx = .{
            .arena = self.arena,
            .refs = &self.refs,
            .footnote_order = &self.footnote_order,
            .single_dollar_math = self.options.single_dollar_math,
        };
        return &self.inline_ctx;
    }

    fn inlines(self: *Parser, text: []const u8) ![]const ast.Span {
        if (self.structure_only) return &.{};
        return inline_parser.parse(self.ctx(), text);
    }

    // ── link reference definitions ──────────────────────────────────────

    pub const Definition = struct { label: []const u8, url: []const u8, title: []const u8 };

    pub fn parseDefinition(line: []const u8) ?Definition {
        if (scan.indentOf(line) > 3) return null;
        const t = std.mem.trim(u8, line, " \t");
        if (t.len < 4 or t[0] != '[' or (t.len > 1 and t[1] == '^')) return null;
        const close = std.mem.indexOf(u8, t, "]:") orelse return null;
        if (close <= 1) return null;
        const label = t[1..close];
        if (std.mem.indexOfScalar(u8, label, ']') != null) return null;
        var rest = std.mem.trim(u8, t[close + 2 ..], " \t");
        if (rest.len == 0) return null;
        var url: []const u8 = undefined;
        if (rest[0] == '<') {
            const end = std.mem.indexOfScalar(u8, rest, '>') orelse return null;
            url = rest[1..end];
            rest = std.mem.trim(u8, rest[end + 1 ..], " \t");
        } else {
            const end = std.mem.indexOfAny(u8, rest, " \t") orelse rest.len;
            url = rest[0..end];
            rest = std.mem.trim(u8, rest[end..], " \t");
        }
        var title: []const u8 = "";
        if (rest.len >= 2 and (rest[0] == '"' or rest[0] == '\'' or rest[0] == '(')) {
            const closing: u8 = if (rest[0] == '(') ')' else rest[0];
            if (rest[rest.len - 1] != closing) return null;
            title = rest[1 .. rest.len - 1];
        } else if (rest.len > 0) return null;
        return .{ .label = label, .url = url, .title = title };
    }

    fn collectDefinitions(self: *Parser, lines: []const []const u8) !void {
        var fence: ?scan.Fence = null;
        var prev_blank = true;
        for (lines) |raw| {
            const line = stripQuotePrefix(raw);
            if (fence) |f| {
                if (scan.isFenceClose(line, f)) fence = null;
                continue;
            }
            if (scan.fenceOpen(line)) |f| {
                fence = f;
                continue;
            }
            if (prev_blank or parseDefinition(line) != null) {
                if (parseDefinition(line)) |def| {
                    const key = try inline_parser.normalizeRefLabel(self.arena, def.label);
                    const gop = try self.refs.getOrPut(self.arena, key);
                    if (!gop.found_existing) gop.value_ptr.* = .{ .url = def.url, .title = def.title };
                    continue;
                }
            }
            prev_blank = scan.isBlank(line);
        }
    }

    fn stripQuotePrefix(line: []const u8) []const u8 {
        var t = line;
        while (true) {
            const s = std.mem.trimStart(u8, t, " ");
            if (s.len > 0 and s[0] == '>') {
                t = if (s.len > 1 and s[1] == ' ') s[2..] else s[1..];
            } else return t;
        }
    }

    // ── document ────────────────────────────────────────────────────────

    pub fn parseDocument(self: *Parser, source: []const u8, ranges: ?*std.ArrayListUnmanaged(LineRange)) !ast.Document.Parts {
        var lines = try splitLines(self.arena, source);
        try self.collectDefinitions(lines);
        var front: []const u8 = "";
        var first: usize = 0;
        if (self.options.front_matter and lines.len > 1 and std.mem.eql(u8, std.mem.trimEnd(u8, lines[0], " \t"), "---")) {
            var j: usize = 1;
            while (j < lines.len) : (j += 1) {
                const t = std.mem.trimEnd(u8, lines[j], " \t");
                if (std.mem.eql(u8, t, "---") or std.mem.eql(u8, t, "...")) break;
            }
            if (j < lines.len and j > 1) {
                const start = lineOffset(source, lines, 1);
                const end = lineOffset(source, lines, j);
                front = std.mem.trimEnd(u8, source[start..end], "\n");
                first = j + 1;
                if (ranges) |r| try r.append(self.arena, .{ .start = 0, .end = first });
            }
        }
        lines = lines[first..];
        const blocks = try self.parseLines(lines, ranges, first);
        return .{ .blocks = blocks, .footnotes = try self.collectFootnotes(), .front_matter = front };
    }

    fn collectFootnotes(self: *Parser) ![]const ast.Footnote {
        if (self.footnote_defs.items.len == 0) return &.{};
        var out: std.ArrayListUnmanaged(ast.Footnote) = .empty;
        // referenced notes in reference order, then the unreferenced ones
        for (self.footnote_order.items, 0..) |label, idx| {
            for (self.footnote_defs.items) |def| {
                if (std.mem.eql(u8, def.label, label)) {
                    try out.append(self.arena, .{ .label = label, .number = idx + 1, .blocks = def.blocks });
                    break;
                }
            }
        }
        return out.toOwnedSlice(self.arena);
    }

    fn slugFor(self: *Parser, spans: []const ast.Span) ![]const u8 {
        if (self.structure_only) return "";
        var out: std.ArrayListUnmanaged(u8) = .empty;
        for (spans) |span| {
            for (span.text) |c| {
                if (std.ascii.isAlphanumeric(c)) {
                    try out.append(self.arena, std.ascii.toLower(c));
                } else if (c == ' ' or c == '-') {
                    try out.append(self.arena, '-');
                } else if (c >= 0x80) {
                    try out.append(self.arena, c);
                }
            }
        }
        const base = try out.toOwnedSlice(self.arena);
        const gop = try self.slugs.getOrPut(self.arena, base);
        if (!gop.found_existing) {
            gop.value_ptr.* = 0;
            return base;
        }
        gop.value_ptr.* += 1;
        return std.fmt.allocPrint(self.arena, "{s}-{d}", .{ base, gop.value_ptr.* });
    }

    // ── blocks ──────────────────────────────────────────────────────────

    /// Parses a sequence of (container-stripped) lines. When `ranges` is set,
    /// records the line range of each emitted block (offset by `base`).
    pub fn parseLines(self: *Parser, lines: []const []const u8, ranges: ?*std.ArrayListUnmanaged(LineRange), base: usize) anyerror![]const Block {
        var blocks: std.ArrayListUnmanaged(Block) = .empty;
        var i: usize = 0;
        while (i < lines.len) {
            const line = lines[i];
            if (scan.isBlank(line)) {
                i += 1;
                continue;
            }
            const start = i;
            const before = blocks.items.len;
            i = try self.parseBlock(lines, i, &blocks);
            if (ranges) |r| {
                if (blocks.items.len > before or i > start) try r.append(self.arena, .{ .start = base + start, .end = base + i });
            }
        }
        return blocks.toOwnedSlice(self.arena);
    }

    fn parseBlock(self: *Parser, lines: []const []const u8, i: usize, out: *std.ArrayListUnmanaged(Block)) !usize {
        const line = lines[i];
        const indent = scan.indentOf(line);

        if (indent >= 4) return self.indentedCode(lines, i, out);
        if (scan.fenceOpen(line)) |fence| return self.fencedCode(lines, i, fence, out);
        if (mathStart(line)) |fence| return self.mathBlock(lines, i, fence, out);
        if (scan.atxHeading(line)) |h| {
            const spans = try self.inlines(h.text);
            try out.append(self.arena, .{ .heading = .{ .level = h.level, .spans = spans, .id = try self.slugFor(spans) } });
            return i + 1;
        }
        if (scan.isThematicBreak(line)) {
            try out.append(self.arena, .rule);
            return i + 1;
        }
        const t = line[indent..];
        if (t.len > 0 and t[0] == '>') return self.blockquote(lines, i, out);
        if (scan.listMarker(line) != null) return self.list(lines, i, out);
        if (footnoteDefStart(line)) |fd| return self.footnoteDef(lines, i, fd, out);
        if (parseDefinition(line) != null) return i + 1;
        if (scan.isHtmlBlockStart(line)) return self.htmlBlock(lines, i, out);
        if (i + 1 < lines.len and std.mem.indexOfScalar(u8, line, '|') != null) {
            if (try self.table(lines, i, out)) |next| return next;
        }
        return self.paragraph(lines, i, out);
    }

    fn indentedCode(self: *Parser, lines: []const []const u8, start: usize, out: *std.ArrayListUnmanaged(Block)) !usize {
        var body: std.ArrayListUnmanaged(u8) = .empty;
        var i = start;
        var last_content = start;
        while (i < lines.len) : (i += 1) {
            const line = lines[i];
            if (scan.isBlank(line)) continue;
            if (scan.indentOf(line) < 4) break;
            last_content = i;
        }
        for (lines[start .. last_content + 1], 0..) |line, k| {
            if (k > 0) try body.append(self.arena, '\n');
            try body.appendSlice(self.arena, scan.stripIndent(line, 4));
        }
        try out.append(self.arena, .{ .code = .{ .text = try body.toOwnedSlice(self.arena), .fenced = false } });
        return last_content + 1;
    }

    fn fencedCode(self: *Parser, lines: []const []const u8, start: usize, fence: scan.Fence, out: *std.ArrayListUnmanaged(Block)) !usize {
        var body: std.ArrayListUnmanaged(u8) = .empty;
        var i = start + 1;
        var closed = false;
        while (i < lines.len) : (i += 1) {
            if (scan.isFenceClose(lines[i], fence)) {
                closed = true;
                i += 1;
                break;
            }
            if (i > start + 1) try body.append(self.arena, '\n');
            try body.appendSlice(self.arena, scan.stripIndent(lines[i], fence.indent));
        }
        const info = fence.info;
        const lang_end = std.mem.indexOfAny(u8, info, " \t{") orelse info.len;
        try out.append(self.arena, .{ .code = .{
            .lang = info[0..lang_end],
            .meta = std.mem.trim(u8, info[lang_end..], " \t"),
            .text = try body.toOwnedSlice(self.arena),
            .closed = closed,
        } });
        return i;
    }

    const MathFence = struct { len: usize, indent: usize };

    /// micromark-extension-math (mathFlow) opening fence: up to 3 spaces,
    /// two or more `$`, then meta that must not contain `$`.
    fn mathStart(line: []const u8) ?MathFence {
        const indent = scan.indentOf(line);
        if (indent > 3) return null;
        const t = line[indent..];
        var n: usize = 0;
        while (n < t.len and t[n] == '$') n += 1;
        if (n < 2) return null;
        if (std.mem.indexOfScalar(u8, t[n..], '$') != null) return null;
        return .{ .len = n, .indent = indent };
    }

    fn isMathClose(line: []const u8, fence: MathFence) bool {
        const indent = scan.indentOf(line);
        if (indent > 3) return false;
        const t = std.mem.trimEnd(u8, line[indent..], " \t");
        if (t.len < fence.len) return false;
        for (t) |c| if (c != '$') return false;
        return true;
    }

    fn mathBlock(self: *Parser, lines: []const []const u8, start: usize, fence: MathFence, out: *std.ArrayListUnmanaged(Block)) !usize {
        var source: std.ArrayListUnmanaged(u8) = .empty;
        var i = start + 1;
        var closed = false;
        while (i < lines.len) : (i += 1) {
            if (isMathClose(lines[i], fence)) {
                closed = true;
                i += 1;
                break;
            }
            if (i > start + 1) try source.append(self.arena, '\n');
            try source.appendSlice(self.arena, scan.stripIndent(lines[i], fence.indent));
        }
        try out.append(self.arena, .{ .math = .{ .source = try source.toOwnedSlice(self.arena), .closed = closed } });
        return i;
    }

    fn isQuoteLine(line: []const u8) bool {
        if (scan.indentOf(line) > 3) return false;
        const t = std.mem.trimStart(u8, line, " ");
        return t.len > 0 and t[0] == '>';
    }

    fn stripQuote(line: []const u8) []const u8 {
        const t = std.mem.trimStart(u8, line, " ");
        if (t.len > 1 and t[1] == ' ') return t[2..];
        return t[1..];
    }

    fn blockquote(self: *Parser, lines: []const []const u8, start: usize, out: *std.ArrayListUnmanaged(Block)) !usize {
        var inner: std.ArrayListUnmanaged([]const u8) = .empty;
        var i = start;
        var lazy_ok = false;
        while (i < lines.len) : (i += 1) {
            const line = lines[i];
            if (isQuoteLine(line)) {
                const stripped = stripQuote(line);
                try inner.append(self.arena, stripped);
                lazy_ok = !scan.isBlank(stripped) and scan.fenceOpen(stripped) == null;
                continue;
            }
            // lazy continuation of a paragraph inside the quote
            if (lazy_ok and !scan.isBlank(line) and !interruptsParagraph(line)) {
                try inner.append(self.arena, line);
                continue;
            }
            break;
        }
        var body = inner.items;
        // GFM alerts: > [!NOTE]
        if (body.len > 0) {
            const first = std.mem.trim(u8, body[0], " \t");
            if (calloutKind(first)) |kind| {
                const blocks = try self.parseLines(body[1..], null, 0);
                try out.append(self.arena, .{ .callout = .{ .kind = kind, .blocks = blocks } });
                return i;
            }
        }
        body = inner.items;
        try out.append(self.arena, .{ .quote = try self.parseLines(body, null, 0) });
        return i;
    }

    fn calloutKind(line: []const u8) ?ast.CalloutKind {
        if (line.len < 5 or !std.mem.startsWith(u8, line, "[!") or line[line.len - 1] != ']') return null;
        const name = line[2 .. line.len - 1];
        inline for (.{ .{ "NOTE", ast.CalloutKind.note }, .{ "TIP", ast.CalloutKind.tip }, .{ "IMPORTANT", ast.CalloutKind.important }, .{ "WARNING", ast.CalloutKind.warning }, .{ "CAUTION", ast.CalloutKind.caution } }) |entry| {
            if (std.ascii.eqlIgnoreCase(name, entry[0])) return entry[1];
        }
        return null;
    }

    fn sameListType(a: scan.ListMarker, b: scan.ListMarker) bool {
        if (a.ordered != b.ordered) return false;
        return if (a.ordered) a.delimiter == b.delimiter else a.bullet == b.bullet;
    }

    fn list(self: *Parser, lines: []const []const u8, start: usize, out: *std.ArrayListUnmanaged(Block)) !usize {
        const first = scan.listMarker(lines[start]).?;
        var items: std.ArrayListUnmanaged(ast.ListItem) = .empty;
        var tight = true;
        var i = start;
        while (i < lines.len) {
            const marker = scan.listMarker(lines[i]) orelse break;
            if (!sameListType(first, marker)) break;
            if (scan.isThematicBreak(lines[i])) break;
            const marker_indent = scan.indentOf(lines[i]);
            const content_indent = marker.content_indent;
            var item_lines: std.ArrayListUnmanaged([]const u8) = .empty;
            var rest = marker.rest;
            var checked: ?bool = null;
            if (rest.len >= 3 and rest[0] == '[' and rest[2] == ']' and (rest.len == 3 or rest[3] == ' ')) {
                switch (rest[1]) {
                    ' ' => checked = false,
                    'x', 'X' => checked = true,
                    else => {},
                }
                if (checked != null) rest = if (rest.len > 4) rest[4..] else "";
            }
            try item_lines.append(self.arena, rest);
            var j = i + 1;
            var saw_blank = false;
            var in_fence: ?scan.Fence = null;
            if (scan.fenceOpen(rest)) |f| in_fence = f;
            while (j < lines.len) : (j += 1) {
                const line = lines[j];
                if (scan.isBlank(line)) {
                    try item_lines.append(self.arena, "");
                    saw_blank = true;
                    continue;
                }
                const ind = scan.indentOf(line);
                const nested_marker = ind >= marker_indent + 2 and scan.listMarker(scan.stripIndent(line, ind)) != null;
                if (ind >= content_indent or nested_marker or (in_fence != null and ind > marker_indent)) {
                    const stripped = scan.stripIndent(line, @min(ind, content_indent));
                    if (in_fence) |f| {
                        if (scan.isFenceClose(stripped, f)) in_fence = null;
                    } else if (scan.fenceOpen(stripped)) |f| in_fence = f;
                    try item_lines.append(self.arena, stripped);
                    continue;
                }
                // lazy paragraph continuation
                if (!saw_blank and in_fence == null and !interruptsParagraph(line) and scan.listMarker(line) == null) {
                    const prev = item_lines.items[item_lines.items.len - 1];
                    if (!scan.isBlank(prev)) {
                        try item_lines.append(self.arena, std.mem.trimStart(u8, line, " "));
                        continue;
                    }
                }
                break;
            }
            // trailing blank lines belong between items, not to this item
            var content_end = item_lines.items.len;
            while (content_end > 1 and scan.isBlank(item_lines.items[content_end - 1])) content_end -= 1;
            const trailing_blank = content_end < item_lines.items.len;
            for (item_lines.items[0..content_end]) |l| {
                if (scan.isBlank(l)) {
                    tight = false;
                    break;
                }
            }
            const blocks = try self.parseLines(item_lines.items[0..content_end], null, 0);
            try items.append(self.arena, .{ .checked = checked, .blocks = blocks });
            i = j;
            if (i < lines.len and trailing_blank) {
                if (scan.listMarker(lines[i])) |next| {
                    if (sameListType(first, next)) tight = false;
                }
            }
            // return the trailing blank lines to the caller when the list ends
            if (trailing_blank and (i >= lines.len or scan.listMarker(lines[i]) == null or !sameListType(first, scan.listMarker(lines[i]).?))) {
                i -= item_lines.items.len - content_end;
                break;
            }
        }
        try out.append(self.arena, .{ .list = .{
            .ordered = first.ordered,
            .start = first.number,
            .tight = tight,
            .items = try items.toOwnedSlice(self.arena),
        } });
        return i;
    }

    const FootnoteStart = struct { label: []const u8, rest: []const u8 };

    fn footnoteDefStart(line: []const u8) ?FootnoteStart {
        if (scan.indentOf(line) > 3) return null;
        const t = std.mem.trimStart(u8, line, " ");
        if (!std.mem.startsWith(u8, t, "[^")) return null;
        const close = std.mem.indexOf(u8, t, "]:") orelse return null;
        const label = t[2..close];
        if (label.len == 0 or std.mem.indexOfAny(u8, label, " \t]") != null) return null;
        return .{ .label = label, .rest = std.mem.trimStart(u8, t[close + 2 ..], " ") };
    }

    fn footnoteDef(self: *Parser, lines: []const []const u8, start: usize, fd: FootnoteStart, out: *std.ArrayListUnmanaged(Block)) !usize {
        _ = out;
        var body: std.ArrayListUnmanaged([]const u8) = .empty;
        try body.append(self.arena, fd.rest);
        var i = start + 1;
        var saw_blank = false;
        while (i < lines.len) : (i += 1) {
            const line = lines[i];
            if (scan.isBlank(line)) {
                try body.append(self.arena, "");
                saw_blank = true;
                continue;
            }
            if (scan.indentOf(line) >= 2) {
                try body.append(self.arena, scan.stripIndent(line, 4));
                saw_blank = false;
                continue;
            }
            if (!saw_blank and !interruptsParagraph(line) and footnoteDefStart(line) == null) {
                try body.append(self.arena, line);
                continue;
            }
            break;
        }
        var end = body.items.len;
        while (end > 1 and scan.isBlank(body.items[end - 1])) end -= 1;
        const consumed_blank = body.items.len - end;
        const blocks = try self.parseLines(body.items[0..end], null, 0);
        try self.footnote_defs.append(self.arena, .{ .label = fd.label, .blocks = blocks });
        return i - consumed_blank;
    }

    fn htmlBlock(self: *Parser, lines: []const []const u8, start: usize, out: *std.ArrayListUnmanaged(Block)) !usize {
        var raw: std.ArrayListUnmanaged(u8) = .empty;
        const first = std.mem.trimStart(u8, lines[start], " ");
        const is_comment = std.mem.startsWith(u8, first, "<!--");
        var i = start;
        while (i < lines.len) : (i += 1) {
            const line = lines[i];
            if (!is_comment and i > start and scan.isBlank(line)) break;
            if (raw.items.len > 0) try raw.append(self.arena, '\n');
            try raw.appendSlice(self.arena, line);
            if (is_comment and std.mem.indexOf(u8, line, "-->") != null) {
                i += 1;
                break;
            }
        }
        try out.append(self.arena, .{ .html = try raw.toOwnedSlice(self.arena) });
        return i;
    }

    fn table(self: *Parser, lines: []const []const u8, start: usize, out: *std.ArrayListUnmanaged(Block)) !?usize {
        const aligns = (try scan.tableDelimiter(self.arena, lines[start + 1])) orelse return null;
        const header_cells = try scan.splitTableRow(self.arena, lines[start]);
        if (header_cells.len != aligns.len) return null;
        const cols = aligns.len;
        var headers = try self.arena.alloc([]const ast.Span, cols);
        var raw_headers = try self.arena.alloc([]const u8, cols);
        for (header_cells, 0..) |cell, c| {
            headers[c] = try self.inlines(cell);
            raw_headers[c] = if (self.structure_only) cell else try ast.plainText(self.arena, headers[c]);
        }
        var rows: std.ArrayListUnmanaged([]const []const ast.Span) = .empty;
        var raw_rows: std.ArrayListUnmanaged([]const []const u8) = .empty;
        var i = start + 2;
        while (i < lines.len) : (i += 1) {
            const line = lines[i];
            if (scan.isBlank(line) or interruptsParagraph(line)) break;
            const cells = try scan.splitTableRow(self.arena, line);
            var row = try self.arena.alloc([]const ast.Span, cols);
            var raw_row = try self.arena.alloc([]const u8, cols);
            for (0..cols) |c| {
                const cell = if (c < cells.len) cells[c] else "";
                row[c] = try self.inlines(cell);
                raw_row[c] = if (self.structure_only) cell else try ast.plainText(self.arena, row[c]);
            }
            try rows.append(self.arena, row);
            try raw_rows.append(self.arena, raw_row);
        }
        try out.append(self.arena, .{ .table = .{
            .headers = headers,
            .rows = try rows.toOwnedSlice(self.arena),
            .aligns = aligns,
            .raw_headers = raw_headers,
            .raw_rows = try raw_rows.toOwnedSlice(self.arena),
        } });
        return i;
    }

    fn interruptsParagraph(line: []const u8) bool {
        if (scan.indentOf(line) > 3) return false;
        if (scan.atxHeading(line) != null or scan.fenceOpen(line) != null or scan.isThematicBreak(line)) return true;
        const t = std.mem.trimStart(u8, line, " ");
        if (t.len > 0 and t[0] == '>') return true;
        if (mathStart(line) != null) return true;
        if (scan.listMarker(line)) |m| {
            // empty items and ordered lists not starting at 1 cannot interrupt
            if (m.rest.len == 0) return false;
            if (m.ordered and m.number != 1) return false;
            return true;
        }
        if (scan.isHtmlBlockStart(line)) {
            // only block-level tags interrupt a paragraph
            return isBlockTag(t);
        }
        return false;
    }

    fn isBlockTag(t: []const u8) bool {
        const block_tags = [_][]const u8{ "div", "p", "table", "pre", "details", "summary", "section", "article", "aside", "blockquote", "figure", "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "center", "nav", "header", "footer", "!--" };
        var name_start: usize = 1;
        if (t.len > 1 and t[1] == '/') name_start = 2;
        var end = name_start;
        while (end < t.len and (std.ascii.isAlphanumeric(t[end]) or t[end] == '!' or t[end] == '-')) end += 1;
        const name = t[name_start..end];
        for (block_tags) |tag| if (std.ascii.eqlIgnoreCase(tag, name)) return true;
        return false;
    }

    fn paragraph(self: *Parser, lines: []const []const u8, start: usize, out: *std.ArrayListUnmanaged(Block)) !usize {
        var body: std.ArrayListUnmanaged(u8) = .empty;
        var i = start;
        while (i < lines.len) : (i += 1) {
            const line = lines[i];
            if (scan.isBlank(line)) break;
            if (i > start) {
                if (scan.setextLevel(line)) |level| {
                    const spans = try self.inlines(body.items);
                    try out.append(self.arena, .{ .heading = .{ .level = level, .spans = spans, .id = try self.slugFor(spans) } });
                    return i + 1;
                }
                if (interruptsParagraph(line)) break;
                // a table can start right after paragraph text
                if (std.mem.indexOfScalar(u8, line, '|') != null and i + 1 < lines.len) {
                    if (try scan.tableDelimiter(self.arena, lines[i + 1])) |aligns| {
                        if ((try scan.splitTableRow(self.arena, line)).len == aligns.len) break;
                    }
                }
                try body.append(self.arena, '\n');
            }
            try body.appendSlice(self.arena, std.mem.trimStart(u8, line, " "));
        }
        const text = std.mem.trimEnd(u8, body.items, " \t");
        const spans = try self.inlines(text);
        // a paragraph that is a single image becomes an image block
        if (spans.len == 1 and spans[0].style.image) {
            try out.append(self.arena, .{ .image = .{ .alt = spans[0].text, .url = spans[0].url } });
        } else {
            try out.append(self.arena, .{ .paragraph = spans });
        }
        return i;
    }
};

pub fn splitLines(arena: std.mem.Allocator, source: []const u8) ![]const []const u8 {
    var lines: std.ArrayListUnmanaged([]const u8) = .empty;
    var it = std.mem.splitScalar(u8, source, '\n');
    while (it.next()) |raw| {
        const line = std.mem.trimEnd(u8, raw, "\r");
        try lines.append(arena, try scan.expandLeadingTabs(arena, line));
    }
    // a trailing newline does not start another line
    if (lines.items.len > 0 and source.len > 0 and source[source.len - 1] == '\n') _ = lines.pop();
    return lines.toOwnedSlice(arena);
}

/// Byte offset of line `index` in `source` (lines come from `splitLines`).
fn lineOffset(source: []const u8, lines: []const []const u8, index: usize) usize {
    _ = lines;
    var line: usize = 0;
    var i: usize = 0;
    while (line < index and i < source.len) : (i += 1) {
        if (source[i] == '\n') line += 1;
    }
    return i;
}

/// Byte offsets of the start of every line, plus `source.len` at the end.
pub fn lineStarts(arena: std.mem.Allocator, source: []const u8) ![]const usize {
    var starts: std.ArrayListUnmanaged(usize) = .empty;
    try starts.append(arena, 0);
    for (source, 0..) |c, i| {
        if (c == '\n' and i + 1 < source.len) try starts.append(arena, i + 1);
    }
    try starts.append(arena, source.len);
    return starts.toOwnedSlice(arena);
}
