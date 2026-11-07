//! Port of Streamdown `lib/parse-blocks.tsx`: splits a document into
//! top-level block strings so each block can be parsed, cached and rendered
//! independently.
//!
//! Merge rules follow Streamdown: an unclosed HTML block swallows the
//! following blocks until its tag closes, a block with an odd number of `$$`
//! swallows the next one, and documents with footnotes or link reference
//! definitions stay one block (references must see their definitions).
//!
//! `Splitter` is the incremental cache (`lastParse`): while a document only
//! grows (streaming), blocks before the last stable boundary are reused and
//! only the tail is split again. It is caller-owned — no global state.
const std = @import("std");
const scan = @import("scan.zig");
const parser = @import("parser/mod.zig");
const block_parser = @import("parser/block.zig");

fn hasFootnotes(markdown: []const u8) bool {
    // /\[\^[\w-]{1,200}\](?!:)/ or /\[\^[\w-]{1,200}\]:/ — either way "[^label]"
    var i: usize = 0;
    while (std.mem.indexOfPos(u8, markdown, i, "[^")) |start| : (i = start + 2) {
        var j = start + 2;
        while (j < markdown.len and j - start - 2 < 200 and (std.ascii.isAlphanumeric(markdown[j]) or markdown[j] == '_' or markdown[j] == '-')) j += 1;
        if (j > start + 2 and j < markdown.len and markdown[j] == ']') return true;
    }
    return false;
}

fn hasLinkDefinitions(markdown: []const u8) bool {
    var it = std.mem.splitScalar(u8, markdown, '\n');
    var fence: ?scan.Fence = null;
    while (it.next()) |raw| {
        var line = std.mem.trimEnd(u8, raw, "\r");
        // definitions may sit inside block quotes / list items
        while (true) {
            const t = std.mem.trimStart(u8, line, " ");
            if (t.len > 0 and t[0] == '>') {
                line = t[1..];
            } else break;
        }
        if (fence) |f| {
            if (scan.isFenceClose(line, f)) fence = null;
            continue;
        }
        if (scan.fenceOpen(line)) |f| {
            fence = f;
            continue;
        }
        if (block_parser.Parser.parseDefinition(std.mem.trimStart(u8, line, " "))) |_| return true;
    }
    return false;
}

const voidElements = [_][]const u8{ "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr" };

fn isVoid(tag: []const u8) bool {
    for (voidElements) |v| if (std.ascii.eqlIgnoreCase(v, tag)) return true;
    return false;
}

/// `<tag(?=[\s>/])[^>]*>` occurrences that do not end in `/>`.
fn countOpenTags(block: []const u8, tag: []const u8) usize {
    if (isVoid(tag)) return 0;
    var count: usize = 0;
    var i: usize = 0;
    while (std.mem.indexOfScalarPos(u8, block, i, '<')) |lt| : (i = lt + 1) {
        const name_end = lt + 1 + tag.len;
        if (name_end >= block.len) break;
        if (!std.ascii.eqlIgnoreCase(block[lt + 1 .. name_end], tag)) continue;
        const after = block[name_end];
        if (!(std.ascii.isWhitespace(after) or after == '>' or after == '/')) continue;
        const close = std.mem.indexOfScalarPos(u8, block, name_end, '>') orelse continue;
        if (!std.mem.endsWith(u8, std.mem.trimEnd(u8, block[lt .. close + 1], " \t"), "/>")) count += 1;
    }
    return count;
}

/// `</tag(?=[\s>])[^>]*>` occurrences.
fn countCloseTags(block: []const u8, tag: []const u8) usize {
    var count: usize = 0;
    var i: usize = 0;
    while (std.mem.indexOfPos(u8, block, i, "</")) |lt| : (i = lt + 2) {
        const name_end = lt + 2 + tag.len;
        if (name_end >= block.len) break;
        if (!std.ascii.eqlIgnoreCase(block[lt + 2 .. name_end], tag)) continue;
        const after = block[name_end];
        if (!(std.ascii.isWhitespace(after) or after == '>')) continue;
        if (std.mem.indexOfScalarPos(u8, block, name_end, '>') != null) count += 1;
    }
    return count;
}

/// `<([A-Za-z][\w:-]*)[\s>/]` — first opening tag name.
fn openingTagName(block: []const u8) ?[]const u8 {
    var i: usize = 0;
    while (std.mem.indexOfScalarPos(u8, block, i, '<')) |lt| : (i = lt + 1) {
        var j = lt + 1;
        if (j >= block.len or !std.ascii.isAlphabetic(block[j])) continue;
        while (j < block.len and (std.ascii.isAlphanumeric(block[j]) or block[j] == '_' or block[j] == ':' or block[j] == '-')) j += 1;
        if (j < block.len and (std.ascii.isWhitespace(block[j]) or block[j] == '>' or block[j] == '/')) return block[lt + 1 .. j];
    }
    return null;
}

fn countDoubleDollars(text: []const u8) usize {
    var count: usize = 0;
    var i: usize = 0;
    while (i + 1 < text.len) : (i += 1) {
        if (text[i] == '$' and text[i + 1] == '$') {
            count += 1;
            i += 1;
        }
    }
    return count;
}

const Kind = enum { html, code, other };

fn classify(raw: []const u8) Kind {
    const first = raw[0 .. std.mem.indexOfScalar(u8, raw, '\n') orelse raw.len];
    if (scan.fenceOpen(first) != null or scan.indentOf(first) >= 4) return .code;
    if (scan.isHtmlBlockStart(first)) return .html;
    return .other;
}

/// Splits `input` into raw block strings (slices of `input`), applying
/// Streamdown's merge rules. Each block keeps its trailing blank lines, so
/// the blocks concatenate back to `input`.
pub fn splitRaw(arena: std.mem.Allocator, input: []const u8) ![]const []const u8 {
    return splitRawWith(arena, input, .{});
}

/// `splitRaw` with the document's parse options: block boundaries depend on
/// them (with `latex_delimiters`, a multi-line `\[ … \]` formula is one
/// block even when a line inside it looks like a heading underline `=` or a
/// list item `+`).
pub fn splitRawWith(arena: std.mem.Allocator, input: []const u8, options: parser.Options) ![]const []const u8 {
    if (input.len == 0) return &.{};
    const ranges = try parser.topLevelRanges(arena, input, options);
    const starts = try block_parser.lineStarts(arena, input);
    var tokens: std.ArrayListUnmanaged([]const u8) = .empty;
    for (ranges, 0..) |range, k| {
        const begin = if (k == 0) 0 else starts[@min(range.start, starts.len - 1)];
        const end = if (k + 1 < ranges.len) starts[@min(ranges[k + 1].start, starts.len - 1)] else input.len;
        if (end > begin) try tokens.append(arena, input[begin..end]);
    }
    if (tokens.items.len == 0) try tokens.append(arena, input);
    return mergeTokensIntoBlocks(arena, input, tokens.items);
}

fn mergeTokensIntoBlocks(arena: std.mem.Allocator, input: []const u8, tokens: []const []const u8) ![]const []const u8 {
    var merged: std.ArrayListUnmanaged([]const u8) = .empty;
    var html_stack: std.ArrayListUnmanaged([]const u8) = .empty;
    var previous_was_code = false;
    for (tokens) |current| {
        if (html_stack.items.len > 0) {
            extend(input, &merged, current);
            const tracked = html_stack.items[html_stack.items.len - 1];
            const opens = countOpenTags(current, tracked);
            const closes = countCloseTags(current, tracked);
            for (0..opens) |_| try html_stack.append(arena, tracked);
            for (0..closes) |_| {
                if (html_stack.items.len > 0 and std.mem.eql(u8, html_stack.items[html_stack.items.len - 1], tracked)) _ = html_stack.pop();
            }
            continue;
        }
        const kind = classify(current);
        if (kind == .html) {
            if (openingTagName(current)) |tag| {
                if (countOpenTags(current, tag) > countCloseTags(current, tag)) try html_stack.append(arena, tag);
            }
        }
        if (merged.items.len > 0 and !previous_was_code) {
            if (countDoubleDollars(merged.items[merged.items.len - 1]) % 2 == 1) {
                extend(input, &merged, current);
                continue;
            }
        }
        try merged.append(arena, current);
        previous_was_code = kind == .code;
    }
    return merged.toOwnedSlice(arena);
}

/// Grows the last merged block to also cover `next` (they are adjacent
/// slices of `input`).
fn extend(input: []const u8, merged: *std.ArrayListUnmanaged([]const u8), next: []const u8) void {
    const last = &merged.items[merged.items.len - 1];
    const begin = @intFromPtr(last.ptr) - @intFromPtr(input.ptr);
    const end = @intFromPtr(next.ptr) - @intFromPtr(input.ptr) + next.len;
    last.* = input[begin..end];
}

/// Number of leading blocks that cannot change when text is appended.
fn countStableBlocks(blocks: []const []const u8) usize {
    if (blocks.len < 3) return 0;
    var i = blocks.len - 3;
    while (true) {
        if (std.mem.endsWith(u8, blocks[i], "\n\n")) return i + 1;
        if (i == 0) return 0;
        i -= 1;
    }
}

/// Incremental splitter (Streamdown's single-entry `ParseCache`).
pub const Splitter = struct {
    allocator: std.mem.Allocator,
    input: std.ArrayListUnmanaged(u8) = .empty,
    /// Owned copies of the block strings of the last split.
    blocks: std.ArrayListUnmanaged([]u8) = .empty,
    /// Statistics for tests/profiling: blocks reused by the last split.
    reused: usize = 0,
    /// Parse options the blocks were split with (`setOptions`).
    options: parser.Options = .{},

    pub fn init(allocator: std.mem.Allocator) Splitter {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Splitter) void {
        self.clearBlocks(0);
        self.blocks.deinit(self.allocator);
        self.input.deinit(self.allocator);
    }

    /// Parse options for the following splits; a change drops the cache.
    pub fn setOptions(self: *Splitter, options: parser.Options) void {
        if (std.meta.eql(self.options, options)) return;
        self.options = options;
        self.clearBlocks(0);
        self.input.clearRetainingCapacity();
    }

    fn clearBlocks(self: *Splitter, keep: usize) void {
        for (self.blocks.items[keep..]) |b| self.allocator.free(b);
        self.blocks.shrinkRetainingCapacity(keep);
    }

    /// Returns the blocks of `markdown`; valid until the next call.
    pub fn split(self: *Splitter, markdown: []const u8) ![]const []const u8 {
        var arena_state = std.heap.ArenaAllocator.init(self.allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();

        const input = if (std.mem.indexOfScalar(u8, markdown, '\r') != null) try normalizeNewlines(arena, markdown) else markdown;
        if (std.mem.eql(u8, input, self.input.items) and (self.blocks.items.len > 0 or input.len == 0)) {
            self.reused = self.blocks.items.len;
            return self.view();
        }

        // footnotes and link definitions: the whole document is one block
        if (hasFootnotes(input) or hasLinkDefinitions(input)) {
            try self.replaceAll(&.{input}, input);
            self.reused = 0;
            return self.view();
        }

        var stable: usize = 0;
        var stable_len: usize = 0;
        if (input.len > self.input.items.len and std.mem.startsWith(u8, input, self.input.items)) {
            stable = countStableBlocks(@ptrCast(self.blocks.items));
            for (self.blocks.items[0..stable]) |b| stable_len += b.len;
        }
        const tail = try splitRawWith(arena, input[stable_len..], self.options);
        self.clearBlocks(stable);
        for (tail) |b| try self.blocks.append(self.allocator, try self.allocator.dupe(u8, b));
        self.input.clearRetainingCapacity();
        try self.input.appendSlice(self.allocator, input);
        self.reused = stable;
        return self.view();
    }

    fn replaceAll(self: *Splitter, blocks: []const []const u8, input: []const u8) !void {
        self.clearBlocks(0);
        for (blocks) |b| try self.blocks.append(self.allocator, try self.allocator.dupe(u8, b));
        self.input.clearRetainingCapacity();
        try self.input.appendSlice(self.allocator, input);
    }

    fn view(self: *Splitter) []const []const u8 {
        return @ptrCast(self.blocks.items);
    }
};

fn normalizeNewlines(arena: std.mem.Allocator, text: []const u8) ![]const u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var i: usize = 0;
    while (i < text.len) : (i += 1) {
        if (text[i] == '\r') {
            try out.append(arena, '\n');
            if (i + 1 < text.len and text[i + 1] == '\n') i += 1;
        } else try out.append(arena, text[i]);
    }
    return out.toOwnedSlice(arena);
}

test "blocks concatenate back to the input" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const src = "# T\n\npara\n\n```js\nx\n\ny\n```\n\n$$\na\n\n= b\n$$\n\n<div>\n\ninner\n\n</div>\n\nend";
    const blocks = try splitRaw(arena.allocator(), src);
    var joined: std.ArrayListUnmanaged(u8) = .empty;
    for (blocks) |b| try joined.appendSlice(arena.allocator(), b);
    try std.testing.expectEqualStrings(src, joined.items);
    // heading, para, code, math (merged), html (merged), end
    try std.testing.expectEqual(@as(usize, 6), blocks.len);
}

test "splitter reuses stable blocks while streaming" {
    var s = Splitter.init(std.testing.allocator);
    defer s.deinit();
    _ = try s.split("# A\n\none\n\ntwo\n\nthr");
    const blocks = try s.split("# A\n\none\n\ntwo\n\nthree and more\n\nfour");
    try std.testing.expect(s.reused >= 2);
    try std.testing.expectEqualStrings("# A\n\n", blocks[0]);
    try std.testing.expectEqualStrings("four", blocks[blocks.len - 1]);
}
