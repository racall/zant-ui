//! Port of remend `scan.ts`: single-pass classification of a text into code
//! and prose regions, plus lazy masks for math, link URLs and HTML tags.
//!
//! Positions are UTF-8 byte offsets. Every delimiter remend looks at is ASCII,
//! so byte positions behave exactly like the UTF-16 positions of the original.
const std = @import("std");

pub const Region = enum(u8) {
    prose = 0,
    /// The ``` or ~~~ run that opens or closes a fence
    fence_marker = 1,
    /// The info string on a fence opener line
    fence_info = 2,
    fence_body = 3,
    /// A complete inline code span, including its backtick markers
    code_span = 4,
    /// An inline code span whose closing run has not arrived yet
    code_span_open = 5,
};

pub const OpenFence = struct {
    char: u8,
    length: usize,
    list_indent: usize,
    quote_depth: usize,
};

pub const OpenSpan = struct {
    run_length: usize,
    start: usize,
};

pub const Scan = struct {
    allocator: std.mem.Allocator,
    text: []const u8,
    regions: []Region,
    open_fence: ?OpenFence,
    open_span: ?OpenSpan,
    math_mask: ?[]const bool = null,
    link_url_mask: ?[]const bool = null,
    html_tag_mask: ?[]const bool = null,

    pub fn init(allocator: std.mem.Allocator, text: []const u8) !Scan {
        const regions = try allocator.alloc(Region, text.len);
        @memset(regions, .prose);
        const open_fence = paintFences(text, regions);
        const open_span = paintSpans(text, regions);
        return .{ .allocator = allocator, .text = text, .regions = regions, .open_fence = open_fence, .open_span = open_span };
    }

    /// A code construct here means a fence or inline span.
    pub fn isCodeAt(self: *const Scan, position: isize) bool {
        if (position >= @as(isize, @intCast(self.regions.len))) return self.open_fence != null or self.open_span != null;
        if (position < 0) return false;
        return self.regions[@intCast(position)] != .prose;
    }

    /// Whether the position is inside a fenced code block (marker, info, or body)
    pub fn isFenceAt(self: *const Scan, position: isize) bool {
        if (position >= @as(isize, @intCast(self.regions.len))) return self.open_fence != null;
        if (position < 0) return false;
        return switch (self.regions[@intCast(position)]) {
            .fence_marker, .fence_info, .fence_body => true,
            else => false,
        };
    }

    pub fn isCompleteSpanAt(self: *const Scan, position: isize) bool {
        if (position < 0 or position >= @as(isize, @intCast(self.regions.len))) return false;
        return self.regions[@intCast(position)] == .code_span;
    }

    pub fn isProse(self: *const Scan, position: usize) bool {
        return position < self.regions.len and self.regions[position] == .prose;
    }

    pub fn inMathAt(self: *Scan, position: usize) bool {
        if (position >= self.text.len) return false;
        if (self.math_mask == null) {
            self.math_mask = if (hasMathDelimiters(self.text)) buildMathMask(self) catch &.{} else &.{};
        }
        const mask = self.math_mask.?;
        return position < mask.len and mask[position];
    }

    pub fn inLinkUrlAt(self: *Scan, position: usize) bool {
        if (position >= self.text.len) return false;
        if (self.link_url_mask == null) {
            self.link_url_mask = if (std.mem.indexOf(u8, self.text, "](") != null) buildLinkUrlMask(self) catch &.{} else &.{};
        }
        const mask = self.link_url_mask.?;
        return position < mask.len and mask[position];
    }

    pub fn inHtmlTagAt(self: *Scan, position: usize) bool {
        if (position >= self.text.len) return false;
        if (self.html_tag_mask == null) {
            self.html_tag_mask = if (std.mem.indexOfScalar(u8, self.text, '<') != null) buildHtmlTagMask(self) catch &.{} else &.{};
        }
        const mask = self.html_tag_mask.?;
        return position < mask.len and mask[position];
    }

    /// Counts non-overlapping double-character pairs (**, ~~, $$) in prose.
    pub fn countDoublePairs(self: *const Scan, char: u8) usize {
        const text = self.text;
        var count: usize = 0;
        var i: usize = 0;
        while (i + 1 < text.len) {
            if (text[i] == char and text[i + 1] == char) {
                if (self.regions[i] == .prose) {
                    count += 1;
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        return count;
    }
};

fn isDigit(text: []const u8, i: usize) bool {
    return i < text.len and text[i] >= '0' and text[i] <= '9';
}

fn at(text: []const u8, i: usize) u8 {
    return if (i < text.len) text[i] else 0;
}

/// Index just past a list marker and its space at `start`, or null.
fn skipListMarker(text: []const u8, start: usize) ?usize {
    var i = start;
    const c = at(text, i);
    if (c == '-' or c == '*' or c == '+') {
        i += 1;
    } else {
        while (isDigit(text, i) and i - start < 9) i += 1;
        if (i == start or (at(text, i) != '.' and at(text, i) != ')')) return null;
        i += 1;
    }
    return if (at(text, i) == ' ') i + 1 else null;
}

const ContainerPrefix = struct { content_start: usize, list_indent: usize, quote_depth: usize };

fn skipContainerPrefix(text: []const u8, line_start: usize, line_end: usize) ContainerPrefix {
    var quote_depth: usize = 0;
    var after_quotes = line_start;
    var starts_item = false;
    var i = line_start;
    while (true) {
        while (i < line_end and text[i] == ' ') i += 1;
        if (i < line_end and text[i] == '>') {
            quote_depth += 1;
            i += if (at(text, i + 1) == ' ') 2 else 1;
            after_quotes = i;
            starts_item = false;
            continue;
        }
        const after_marker = if (i < line_end) skipListMarker(text, i) else null;
        if (after_marker == null) {
            return .{
                .content_start = i,
                .list_indent = if (starts_item) i - after_quotes else 0,
                .quote_depth = quote_depth,
            };
        }
        i = after_marker.?;
        starts_item = true;
    }
}

fn skipQuoteMarkers(text: []const u8, line_start: usize, line_end: usize, depth: usize) ?usize {
    var i = line_start;
    var found: usize = 0;
    while (found < depth) : (found += 1) {
        while (i < line_end and text[i] == ' ') i += 1;
        if (at(text, i) != '>') return null;
        i += 1;
        if (at(text, i) == ' ') i += 1;
    }
    return i;
}

fn fill(regions: []Region, value: Region, start: usize, end: usize) void {
    const s = @min(start, regions.len);
    const e = @min(end, regions.len);
    if (s < e) @memset(regions[s..e], value);
}

fn isFenceCloser(text: []const u8, content_start: usize, line_end: usize, fence: OpenFence) bool {
    var i = content_start;
    while (i < line_end and text[i] == ' ') i += 1;
    var run: usize = 0;
    while (i < line_end and text[i] == fence.char) {
        i += 1;
        run += 1;
    }
    if (run < fence.length) return false;
    while (i < line_end) : (i += 1) {
        if (text[i] != ' ' and text[i] != '\t' and text[i] != '\r') return false;
    }
    return true;
}

fn openFenceAt(text: []const u8, regions: []Region, line_start: usize, line_end: usize) ?OpenFence {
    const content_end = if (line_end > line_start and text[line_end - 1] == '\r') line_end - 1 else line_end;
    const prefix = skipContainerPrefix(text, line_start, content_end);
    const line = text[@min(prefix.content_start, content_end)..content_end];
    // /^(`{3,}|~{3,})(.*)$/
    if (line.len < 3 or (line[0] != '`' and line[0] != '~')) return null;
    const char = line[0];
    var run: usize = 0;
    while (run < line.len and line[run] == char) run += 1;
    if (run < 3) return null;
    const info = line[run..];
    if (char == '`' and std.mem.indexOfScalar(u8, info, '`') != null) return null;
    const marker_start = prefix.content_start;
    fill(regions, .fence_marker, marker_start, marker_start + run);
    fill(regions, .fence_info, marker_start + run, line_end + 1);
    return .{ .char = char, .length = run, .list_indent = prefix.list_indent, .quote_depth = prefix.quote_depth };
}

fn fenceContentStart(text: []const u8, line_start: usize, line_end: usize, fence: OpenFence) ?usize {
    const content_start = skipQuoteMarkers(text, line_start, line_end, fence.quote_depth) orelse return null;
    if (fence.list_indent == 0) return content_start;
    var i = content_start;
    while (i < line_end and text[i] == ' ') i += 1;
    const blank = i == line_end or (text[i] == '\r' and i + 1 == line_end);
    return if (blank or i - content_start >= fence.list_indent) content_start else null;
}

fn paintFences(text: []const u8, regions: []Region) ?OpenFence {
    const n = text.len;
    var open_fence: ?OpenFence = null;
    var line_start: usize = 0;
    while (line_start < n) {
        const line_end = std.mem.indexOfScalarPos(u8, text, line_start, '\n') orelse n;
        var content_start: ?usize = null;
        if (open_fence) |fence| {
            content_start = fenceContentStart(text, line_start, line_end, fence);
            if (content_start == null) open_fence = null;
        }
        if (open_fence == null) {
            open_fence = openFenceAt(text, regions, line_start, line_end);
        } else if (isFenceCloser(text, content_start.?, line_end, open_fence.?)) {
            fill(regions, .fence_marker, line_start, line_end);
            open_fence = null;
        } else {
            fill(regions, .fence_body, line_start, @min(line_end + 1, n));
        }
        line_start = line_end + 1;
    }
    return open_fence;
}

fn measureBacktickRun(text: []const u8, start: usize) usize {
    var end = start + 1;
    while (end < text.len and text[end] == '`') end += 1;
    return end;
}

fn isParagraphBreakAt(text: []const u8, newline_index: usize) bool {
    var j = newline_index + 1;
    while (j < text.len and (text[j] == ' ' or text[j] == '\t' or text[j] == '\r')) j += 1;
    return j < text.len and text[j] == '\n';
}

fn nextSpanOpener(text: []const u8, regions: []const Region, from: usize) ?usize {
    var i = from;
    while (true) {
        const next = std.mem.indexOfScalarPos(u8, text, i, '`') orelse return null;
        const escaped = next > i and text[next - 1] == '\\' and regions[next - 1] == .prose;
        if (regions[next] == .prose and !escaped) return next;
        i = next + 1;
    }
}

fn paintSpans(text: []const u8, regions: []Region) ?OpenSpan {
    const n = text.len;
    var span_start: ?usize = null;
    var span_run: usize = 0;
    var i: usize = 0;
    while (i < n) {
        if (span_start == null) {
            i = nextSpanOpener(text, regions, i) orelse break;
            const opener_end = measureBacktickRun(text, i);
            span_start = i;
            span_run = opener_end - i;
            i = opener_end;
            continue;
        }
        if (regions[i] != .prose) {
            fill(regions, .code_span_open, span_start.?, i);
            span_start = null;
            i += 1;
            continue;
        }
        if (text[i] == '\n' and isParagraphBreakAt(text, i)) {
            span_start = null;
            i += 1;
            continue;
        }
        if (text[i] != '`') {
            i += 1;
            continue;
        }
        const run_end = measureBacktickRun(text, i);
        if (run_end - i == span_run) {
            fill(regions, .code_span, span_start.?, run_end);
            span_start = null;
        }
        i = run_end;
    }
    if (span_start) |start| {
        fill(regions, .code_span_open, start, n);
        return .{ .start = start, .run_length = span_run };
    }
    return null;
}

const MathContext = enum { none, inline_dollar, block_dollar, inline_latex, block_latex };

fn hasMathDelimiters(text: []const u8) bool {
    return std.mem.indexOfScalar(u8, text, '$') != null or
        std.mem.indexOf(u8, text, "\\(") != null or std.mem.indexOf(u8, text, "\\[") != null;
}

fn buildMathMask(scan: *const Scan) ![]const bool {
    const text = scan.text;
    const mask = try scan.allocator.alloc(bool, text.len);
    @memset(mask, false);
    var context: MathContext = .none;
    var i: usize = 0;
    while (i < text.len) {
        mask[i] = context != .none;
        if (scan.regions[i] != .prose) {
            i += 1;
            continue;
        }
        const next = at(text, i + 1);
        var new_context: ?MathContext = null;
        var length: usize = 1;
        if (text[i] == '\\') {
            if (next == '$') {
                new_context = context;
                length = 2;
            } else {
                const latex: ?MathContext = if (next == '[' and context == .none) .block_latex else if (next == ']' and context == .block_latex) .none else if (next == '(' and context == .none) .inline_latex else if (next == ')' and context == .inline_latex) .none else null;
                if (latex) |value| {
                    new_context = value;
                    length = 2;
                }
            }
        } else if (text[i] == '$' and context != .inline_latex and context != .block_latex) {
            const is_block = next == '$';
            new_context = if (is_block)
                (if (context == .block_dollar) .none else .block_dollar)
            else if (context == .block_dollar)
                context
            else if (context == .inline_dollar) .none else .inline_dollar;
            length = if (is_block) 2 else 1;
        }
        const value = new_context orelse {
            i += 1;
            continue;
        };
        context = value;
        if (length == 2 and i + 1 < text.len) mask[i + 1] = context != .none;
        i += length;
    }
    return mask;
}

fn buildLinkUrlMask(scan: *const Scan) ![]const bool {
    const text = scan.text;
    const mask = try scan.allocator.alloc(bool, text.len);
    @memset(mask, false);
    var line_start: usize = 0;
    while (line_start < text.len) {
        const line_end = std.mem.indexOfScalarPos(u8, text, line_start, '\n') orelse text.len;
        // closerFollows: a ")" exists at or after i on this line
        var closer_after: ?usize = null; // smallest index of a prose ')' >= i, walking backwards
        var follows = try scan.allocator.alloc(bool, line_end - line_start);
        defer scan.allocator.free(follows);
        var i = line_end;
        while (i > line_start) {
            i -= 1;
            if (text[i] == ')' and scan.regions[i] == .prose) closer_after = i;
            follows[i - line_start] = closer_after != null;
        }
        var in_url = false;
        i = line_start;
        while (i < line_end) : (i += 1) {
            if (in_url and follows[i - line_start]) mask[i] = true;
            if (scan.regions[i] != .prose) continue;
            if (text[i] == ')') {
                in_url = false;
            } else if (text[i] == '(') {
                in_url = i > 0 and text[i - 1] == ']';
            }
        }
        line_start = line_end + 1;
    }
    return mask;
}

fn buildHtmlTagMask(scan: *const Scan) ![]const bool {
    const text = scan.text;
    const mask = try scan.allocator.alloc(bool, text.len);
    @memset(mask, false);
    var in_tag = false;
    for (text, 0..) |c, i| {
        if (c == '\n') {
            in_tag = false;
            continue;
        }
        mask[i] = in_tag;
        if (scan.regions[i] != .prose) continue;
        if (c == '>') {
            in_tag = false;
        } else if (c == '<') {
            const next = at(text, i + 1);
            in_tag = std.ascii.isAlphabetic(next) or next == '/';
        }
    }
    return mask;
}

test "fences and spans are painted" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const text = "a `b` c\n```js\nx\n```\n``open";
    const scan = try Scan.init(arena.allocator(), text);
    try std.testing.expect(scan.regions[0] == .prose);
    try std.testing.expect(scan.regions[2] == .code_span);
    try std.testing.expect(scan.isFenceAt(12));
    try std.testing.expect(scan.open_span != null);
    try std.testing.expect(scan.open_fence == null);
}
