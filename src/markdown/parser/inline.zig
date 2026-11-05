//! Inline Markdown parser: emphasis, code spans, math, links, images,
//! autolinks, footnote references, inline HTML and character references.
//!
//! Emphasis follows CommonMark's flanking rules, relaxed the way Streamdown's
//! CJK plugin relaxes them: a closing run only needs a non-space character
//! before it, so `**中文。**后文` stays bold.
const std = @import("std");
const ast = @import("../ast.zig");
const entities = @import("../entities.zig");
const scan = @import("../scan.zig");
const remend = @import("../remend/mod.zig");

const Span = ast.Span;
const Style = ast.Style;

pub const LinkDef = struct { url: []const u8, title: []const u8 = "" };
pub const RefMap = std.StringHashMapUnmanaged(LinkDef);

pub const Context = struct {
    arena: std.mem.Allocator,
    refs: *const RefMap,
    /// Footnote labels in order of first reference.
    footnote_order: *std.ArrayListUnmanaged([]const u8),
    /// Recognise `$x$` inline math. On by default with pandoc's rules so
    /// prices stay text: the opener must be followed by a non-space, the
    /// closer preceded by a non-space and not followed by a digit
    /// (`$20 and $10` is text, `$x$` is math).
    single_dollar_math: bool = true,
};

const Builder = struct {
    ctx: *Context,
    out: std.ArrayListUnmanaged(Span) = .empty,

    fn push(self: *Builder, text: []const u8, style: Style, url: []const u8) !void {
        if (text.len == 0) return;
        if (self.out.items.len > 0) {
            const last = &self.out.items[self.out.items.len - 1];
            // merge adjacent runs with identical styling
            if (last.style.eql(style) and std.mem.eql(u8, last.url, url) and !style.footnote_ref and
                last.text.ptr + last.text.len == text.ptr)
            {
                last.text = last.text.ptr[0 .. last.text.len + text.len];
                return;
            }
        }
        try self.out.append(self.ctx.arena, .{ .text = text, .style = style, .url = url });
    }

    fn pushText(self: *Builder, text: []const u8, style: Style, url: []const u8) !void {
        if (text.len == 0) return;
        if (style.code or style.math) return self.push(text, style, url);
        const decoded = try entities.decode(self.ctx.arena, text);
        if (decoded.ptr == text.ptr) return self.push(text, style, url);
        try self.out.append(self.ctx.arena, .{ .text = decoded, .style = style, .url = url });
    }
};

pub fn parse(ctx: *Context, text: []const u8) ![]const Span {
    var b: Builder = .{ .ctx = ctx };
    try parseInto(&b, text, .{}, "");
    return b.out.toOwnedSlice(ctx.arena);
}

/// Parses with a base style (used for table cells / headings with extra style).
pub fn parseStyled(ctx: *Context, text: []const u8, style: Style) ![]const Span {
    var b: Builder = .{ .ctx = ctx };
    try parseInto(&b, text, style, "");
    return b.out.toOwnedSlice(ctx.arena);
}

fn at(text: []const u8, i: usize) u8 {
    return if (i < text.len) text[i] else 0;
}

fn runLength(text: []const u8, i: usize, c: u8) usize {
    var n: usize = 0;
    while (i + n < text.len and text[i + n] == c) n += 1;
    return n;
}

/// End of a code span opened by a run of `n` backticks at `start`, or null.
fn codeSpanEnd(text: []const u8, start: usize, n: usize) ?usize {
    var i = start + n;
    while (std.mem.indexOfScalarPos(u8, text, i, '`')) |j| {
        const len = runLength(text, j, '`');
        if (len == n) return j;
        i = j + len;
    }
    return null;
}

/// End of a math text span opened by `n` dollars at `start` (closing run
/// of exactly `n`), or null. Paragraph-scoped like code spans.
fn mathTextEnd(text: []const u8, start: usize, n: usize) ?usize {
    var i = start + n;
    while (std.mem.indexOfScalarPos(u8, text, i, '$')) |j| {
        const len = runLength(text, j, '$');
        if (len == n) return j;
        i = j + len;
    }
    return null;
}

/// End of a single-dollar math span opened at `start` (pandoc rules), or
/// null when the `$` is literal. Only the first candidate closer is tried.
fn singleDollarEnd(text: []const u8, start: usize) ?usize {
    if (start + 1 >= text.len) return null;
    const first = text[start + 1];
    if (first == ' ' or first == '\t' or first == '\n') return null;
    var from = start + 1;
    while (std.mem.indexOfScalarPos(u8, text, from, '$')) |k| {
        if (text[k - 1] == '\\') {
            from = k + 1;
            continue;
        }
        if (runLength(text, k, '$') != 1) return null;
        const prev = text[k - 1];
        if (prev == ' ' or prev == '\t' or prev == '\n') return null;
        if (k + 1 < text.len and std.ascii.isDigit(text[k + 1])) return null;
        if (k == start + 1) return null;
        return k;
    }
    return null;
}

fn leftFlanking(text: []const u8, start: usize, len: usize) bool {
    const next = scan.cpAt(text, start + len);
    return next != 0 and !scan.isWhitespaceCp(next);
}

fn rightFlanking(text: []const u8, start: usize) bool {
    const prev = scan.cpBefore(text, start);
    return prev != 0 and !scan.isWhitespaceCp(prev);
}

fn underscoreCanOpen(text: []const u8, start: usize, len: usize) bool {
    if (!leftFlanking(text, start, len)) return false;
    const prev = scan.cpBefore(text, start);
    return prev == 0 or !scan.isWordCp(prev);
}

fn underscoreCanClose(text: []const u8, start: usize, len: usize) bool {
    if (!rightFlanking(text, start)) return false;
    const next = scan.cpAt(text, start + len);
    return next == 0 or !scan.isWordCp(next);
}

/// Finds a closing delimiter run of `c` with length >= `need`, skipping code
/// spans, escapes and links. Returns the start of the closing run.
fn findCloser(text: []const u8, from: usize, c: u8, need: usize) ?usize {
    var i = from;
    while (i < text.len) {
        const ch = text[i];
        if (ch == '\\') {
            i += 2;
            continue;
        }
        if (ch == '`') {
            const n = runLength(text, i, '`');
            if (codeSpanEnd(text, i, n)) |end| {
                i = end + n;
                continue;
            }
            i += n;
            continue;
        }
        if (ch == c) {
            const n = runLength(text, i, c);
            const closes = if (c == '_') underscoreCanClose(text, i, n) else rightFlanking(text, i);
            if (closes and n >= need and i > from) {
                // `**b *c***`: the run also closes an inner opener, so the
                // outer closer is its tail.
                if (n > need and hasInnerOpener(text, from, i, c)) return i + n - need;
                return i;
            }
            // a run that can open again is skipped (nested emphasis)
            i += n;
            continue;
        }
        i += 1;
    }
    return null;
}

fn hasInnerOpener(text: []const u8, from: usize, to: usize, c: u8) bool {
    var i = from;
    while (i < to) {
        if (text[i] == c) {
            const n = runLength(text, i, c);
            const opens = if (c == '_') underscoreCanOpen(text, i, n) else leftFlanking(text, i, n);
            if (opens and !rightFlanking(text, i)) return true;
            i += n;
            continue;
        }
        i += 1;
    }
    return false;
}

fn isAsciiPunct(c: u8) bool {
    return scan.isAsciiPunct(c);
}

/// Balanced `[...]` starting at `open` (text[open] == '['). Returns index of `]`.
fn matchBracket(text: []const u8, open: usize) ?usize {
    var depth: usize = 0;
    var i = open;
    while (i < text.len) : (i += 1) {
        switch (text[i]) {
            '\\' => i += 1,
            '`' => {
                const n = runLength(text, i, '`');
                if (codeSpanEnd(text, i, n)) |end| i = end + n - 1 else i += n - 1;
            },
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if (depth == 0) return i;
            },
            else => {},
        }
    }
    return null;
}

const Destination = struct { url: []const u8, title: []const u8, end: usize };

/// Parses `(url "title")` starting at `open` (text[open] == '(').
fn parseDestination(text: []const u8, open: usize) ?Destination {
    var i = open + 1;
    while (i < text.len and (text[i] == ' ' or text[i] == '\n')) i += 1;
    var url_start = i;
    var url_end = i;
    if (at(text, i) == '<') {
        const close = std.mem.indexOfScalarPos(u8, text, i + 1, '>') orelse return null;
        url_start = i + 1;
        url_end = close;
        i = close + 1;
    } else {
        var depth: usize = 0;
        while (i < text.len) : (i += 1) {
            const c = text[i];
            if (c == '\\') {
                i += 1;
                continue;
            }
            if (c == '(') depth += 1;
            if (c == ')') {
                if (depth == 0) break;
                depth -= 1;
            }
            if (c == ' ' or c == '\n') break;
        }
        url_end = @min(i, text.len);
    }
    while (i < text.len and (text[i] == ' ' or text[i] == '\n')) i += 1;
    var title: []const u8 = "";
    if (i < text.len and (text[i] == '"' or text[i] == '\'' or text[i] == '(')) {
        const close_char: u8 = if (text[i] == '(') ')' else text[i];
        const close = std.mem.indexOfScalarPos(u8, text, i + 1, close_char) orelse return null;
        title = text[i + 1 .. close];
        i = close + 1;
        while (i < text.len and (text[i] == ' ' or text[i] == '\n')) i += 1;
    }
    if (at(text, i) != ')') return null;
    return .{ .url = text[url_start..url_end], .title = title, .end = i + 1 };
}

fn normalizeLabel(arena: std.mem.Allocator, label: []const u8) ![]const u8 {
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var space = false;
    for (std.mem.trim(u8, label, " \t\n")) |c| {
        if (c == ' ' or c == '\t' or c == '\n') {
            space = true;
            continue;
        }
        if (space and out.items.len > 0) try out.append(arena, ' ');
        space = false;
        try out.append(arena, std.ascii.toLower(c));
    }
    return out.toOwnedSlice(arena);
}

pub fn lookupRef(ctx: *Context, label: []const u8) !?LinkDef {
    if (ctx.refs.count() == 0) return null;
    const key = try normalizeLabel(ctx.arena, label);
    return ctx.refs.get(key);
}

pub fn normalizeRefLabel(arena: std.mem.Allocator, label: []const u8) ![]const u8 {
    return normalizeLabel(arena, label);
}

/// Bare autolink (GFM): http://, https://, www., or an email address.
fn bareAutolinkEnd(text: []const u8, i: usize) ?usize {
    const rest = text[i..];
    const is_url = std.mem.startsWith(u8, rest, "http://") or std.mem.startsWith(u8, rest, "https://") or
        std.mem.startsWith(u8, rest, "www.");
    if (!is_url) return null;
    if (i > 0 and scan.isWordCp(scan.cpBefore(text, i))) return null;
    var end = i;
    var parens: isize = 0;
    while (end < text.len) {
        const c = text[end];
        if (c <= ' ' or c == '<' or c == '>' or c == '"' or c == '`') break;
        // CJK punctuation ends an autolink (Streamdown CJK plugin)
        if (c >= 0x80) {
            const cp = scan.cpAt(text, end);
            if (scan.isUnicodePunct(cp) or scan.isCjk(cp)) break;
            end += std.unicode.utf8ByteSequenceLength(c) catch 1;
            continue;
        }
        if (c == '(') parens += 1;
        if (c == ')') {
            if (parens == 0) break;
            parens -= 1;
        }
        end += 1;
    }
    while (end > i and std.mem.indexOfScalar(u8, ".,:;!?*_~'", text[end - 1]) != null) end -= 1;
    const min_len: usize = if (std.mem.startsWith(u8, rest, "www.")) 5 else 9;
    if (end - i < min_len) return null;
    return end;
}

fn emailAutolink(text: []const u8, at_index: usize) ?struct { start: usize, end: usize } {
    var start = at_index;
    while (start > 0) {
        const c = text[start - 1];
        if (std.ascii.isAlphanumeric(c) or c == '.' or c == '_' or c == '-' or c == '+') start -= 1 else break;
    }
    if (start == at_index) return null;
    var end = at_index + 1;
    var dot = false;
    while (end < text.len) : (end += 1) {
        const c = text[end];
        if (c == '.') dot = true else if (!(std.ascii.isAlphanumeric(c) or c == '-' or c == '_')) break;
    }
    while (end > at_index + 1 and text[end - 1] == '.') end -= 1;
    if (!dot or end <= at_index + 1 or text[end - 1] == '-') return null;
    return .{ .start = start, .end = end };
}

const TagStyle = struct { name: []const u8, apply: *const fn (*Style) void };

fn setBold(s: *Style) void {
    s.bold = true;
}
fn setItalic(s: *Style) void {
    s.italic = true;
}
fn setStrike(s: *Style) void {
    s.strike = true;
}
fn setUnderline(s: *Style) void {
    s.underline = true;
}
fn setCode(s: *Style) void {
    s.code = true;
}
fn setMark(s: *Style) void {
    s.highlight = true;
}
fn setSup(s: *Style) void {
    s.superscript = true;
}
fn setSub(s: *Style) void {
    s.subscript = true;
}
fn setNothing(_: *Style) void {}

const inline_tags = [_]TagStyle{
    .{ .name = "b", .apply = setBold },        .{ .name = "strong", .apply = setBold },
    .{ .name = "i", .apply = setItalic },      .{ .name = "em", .apply = setItalic },
    .{ .name = "s", .apply = setStrike },      .{ .name = "del", .apply = setStrike },
    .{ .name = "strike", .apply = setStrike }, .{ .name = "u", .apply = setUnderline },
    .{ .name = "ins", .apply = setUnderline }, .{ .name = "code", .apply = setCode },
    .{ .name = "kbd", .apply = setCode },      .{ .name = "mark", .apply = setMark },
    .{ .name = "sup", .apply = setSup },       .{ .name = "sub", .apply = setSub },
    .{ .name = "span", .apply = setNothing },  .{ .name = "small", .apply = setNothing },
    .{ .name = "abbr", .apply = setNothing },  .{ .name = "cite", .apply = setNothing },
    .{ .name = "q", .apply = setNothing },     .{ .name = "a", .apply = setNothing },
};

const Tag = struct { name: []const u8, closing: bool, self_closing: bool, end: usize, href: []const u8 = "" };

fn parseTag(text: []const u8, i: usize) ?Tag {
    if (at(text, i) != '<') return null;
    var j = i + 1;
    const closing = at(text, j) == '/';
    if (closing) j += 1;
    const name_start = j;
    while (j < text.len and (std.ascii.isAlphanumeric(text[j]) or text[j] == '-')) j += 1;
    if (j == name_start or !std.ascii.isAlphabetic(text[name_start])) return null;
    const name = text[name_start..j];
    const close = std.mem.indexOfScalarPos(u8, text, j, '>') orelse return null;
    const attrs = text[j..close];
    if (std.mem.indexOfScalar(u8, attrs, '<') != null or std.mem.indexOfScalar(u8, attrs, '\n') != null) return null;
    var href: []const u8 = "";
    if (std.mem.indexOf(u8, attrs, "href=\"")) |h| {
        const s = h + 6;
        if (std.mem.indexOfScalarPos(u8, attrs, s, '"')) |e| href = attrs[s..e];
    }
    return .{
        .name = name,
        .closing = closing,
        .self_closing = attrs.len > 0 and attrs[attrs.len - 1] == '/',
        .end = close + 1,
        .href = href,
    };
}

fn findClosingTag(text: []const u8, from: usize, name: []const u8) ?struct { start: usize, end: usize } {
    var i = from;
    var depth: usize = 0;
    while (std.mem.indexOfScalarPos(u8, text, i, '<')) |lt| {
        if (parseTag(text, lt)) |tag| {
            if (std.ascii.eqlIgnoreCase(tag.name, name)) {
                if (tag.closing) {
                    if (depth == 0) return .{ .start = lt, .end = tag.end };
                    depth -= 1;
                } else if (!tag.self_closing) depth += 1;
            }
            i = tag.end;
        } else i = lt + 1;
    }
    return null;
}

fn parseInto(b: *Builder, text: []const u8, base: Style, url: []const u8) anyerror!void {
    const ctx = b.ctx;
    var i: usize = 0;
    var plain: usize = 0;
    while (i < text.len) {
        const c = text[i];
        switch (c) {
            '\\' => {
                if (i + 1 < text.len and isAsciiPunct(text[i + 1])) {
                    try b.pushText(text[plain..i], base, url);
                    try b.push(text[i + 1 .. i + 2], base, url);
                    i += 2;
                    plain = i;
                    continue;
                }
                if (i + 1 < text.len and text[i + 1] == '\n') { // hard break
                    try b.pushText(text[plain..i], base, url);
                    try b.push(text[i + 1 .. i + 2], base, url);
                    i += 2;
                    plain = i;
                    continue;
                }
            },
            '`' => {
                const n = runLength(text, i, '`');
                if (codeSpanEnd(text, i, n)) |end| {
                    try b.pushText(text[plain..i], base, url);
                    var body = text[i + n .. end];
                    if (body.len >= 2 and body[0] == ' ' and body[body.len - 1] == ' ' and std.mem.trim(u8, body, " ").len > 0) body = body[1 .. body.len - 1];
                    var style = base;
                    style.code = true;
                    try b.push(try collapseNewlines(ctx.arena, body), style, url);
                    i = end + n;
                    plain = i;
                    continue;
                }
                i += n;
                continue;
            },
            '$' => {
                // micromark-extension-math (mathText): a run of N dollars
                // closes on a run of exactly N; single `$` only when enabled.
                const n = runLength(text, i, '$');
                if (n == 1 and ctx.single_dollar_math) {
                    if (singleDollarEnd(text, i)) |end| {
                        try b.pushText(text[plain..i], base, url);
                        var style = base;
                        style.math = true;
                        try b.push(try collapseNewlines(ctx.arena, text[i + 1 .. end]), style, url);
                        i = end + 1;
                        plain = i;
                        continue;
                    }
                }
                if (n >= 2) {
                    if (mathTextEnd(text, i, n)) |end| {
                        try b.pushText(text[plain..i], base, url);
                        var body = text[i + n .. end];
                        if (body.len >= 2 and (body[0] == ' ' or body[0] == '\n') and
                            (body[body.len - 1] == ' ' or body[body.len - 1] == '\n') and
                            std.mem.trim(u8, body, " \n").len > 0) body = body[1 .. body.len - 1];
                        var style = base;
                        style.math = true;
                        try b.push(try collapseNewlines(ctx.arena, body), style, url);
                        i = end + n;
                        plain = i;
                        continue;
                    }
                }
                i += n;
                continue;
            },
            '*', '_' => {
                const n = runLength(text, i, c);
                const can_open = if (c == '_') underscoreCanOpen(text, i, n) else leftFlanking(text, i, n);
                if (can_open) {
                    const use: usize = @min(n, 3);
                    if (findCloser(text, i + use, c, use)) |close| {
                        try b.pushText(text[plain..i], base, url);
                        // emit any extra opening markers literally (****x)
                        if (n > use) try b.push(text[i .. i + n - use], base, url);
                        var style = base;
                        if (use >= 2) style.bold = true;
                        if (use != 2) style.italic = true;
                        try parseInto(b, text[i + n .. close], style, url);
                        i = close + use;
                        plain = i;
                        continue;
                    }
                    // fall back to shorter runs: **a* → *a* with literal '*'
                    if (use > 1) if (findCloser(text, i + n, c, 1)) |close| {
                        const close_len = runLength(text, close, c);
                        const k = @min(close_len, use);
                        try b.pushText(text[plain..i], base, url);
                        try b.push(text[i .. i + n - k], base, url);
                        var style = base;
                        if (k >= 2) style.bold = true;
                        if (k != 2) style.italic = true;
                        try parseInto(b, text[i + n .. close], style, url);
                        i = close + k;
                        plain = i;
                        continue;
                    };
                }
                i += n;
                continue;
            },
            '~' => {
                const n = runLength(text, i, '~');
                if ((n == 1 or n == 2) and leftFlanking(text, i, n)) {
                    var matched = false;
                    var search = i + n;
                    while (std.mem.indexOfScalarPos(u8, text, search, '~')) |close| {
                        const m = runLength(text, close, '~');
                        if (m == n and rightFlanking(text, close) and text[close - 1] != '\\') {
                            try b.pushText(text[plain..i], base, url);
                            var style = base;
                            style.strike = true;
                            try parseInto(b, text[i + n .. close], style, url);
                            i = close + n;
                            plain = i;
                            matched = true;
                            break;
                        }
                        search = close + m;
                    }
                    if (matched) continue;
                }
                i += n;
                continue;
            },
            '=' => {
                if (at(text, i + 1) == '=' and leftFlanking(text, i, 2)) {
                    if (std.mem.indexOfPos(u8, text, i + 2, "==")) |close| {
                        if (close > i + 2 and rightFlanking(text, close)) {
                            try b.pushText(text[plain..i], base, url);
                            var style = base;
                            style.highlight = true;
                            try parseInto(b, text[i + 2 .. close], style, url);
                            i = close + 2;
                            plain = i;
                            continue;
                        }
                    }
                }
            },
            '!', '[' => {
                const is_image = c == '!';
                const open = if (is_image) i + 1 else i;
                if (at(text, open) == '[') {
                    if (try parseLinkLike(b, text, i, open, is_image, base, url, plain)) |next| {
                        i = next;
                        plain = i;
                        continue;
                    }
                }
            },
            '<' => {
                if (std.mem.indexOfScalarPos(u8, text, i + 1, '>')) |close| {
                    const inner = text[i + 1 .. close];
                    const is_uri = (std.mem.indexOf(u8, inner, "://") != null or std.mem.startsWith(u8, inner, "mailto:")) and
                        std.mem.indexOfAny(u8, inner, " <\n") == null;
                    const is_email = !is_uri and std.mem.indexOfScalar(u8, inner, '@') != null and
                        std.mem.indexOfAny(u8, inner, " <\n/") == null;
                    if (is_uri or is_email) {
                        try b.pushText(text[plain..i], base, url);
                        var style = base;
                        style.link = true;
                        const target = if (is_email) try std.mem.concat(ctx.arena, u8, &.{ "mailto:", inner }) else inner;
                        try b.push(inner, style, target);
                        i = close + 1;
                        plain = i;
                        continue;
                    }
                }
                if (parseTag(text, i)) |tag| {
                    if (std.ascii.eqlIgnoreCase(tag.name, "br")) {
                        try b.pushText(text[plain..i], base, url);
                        try b.push("\n", base, url);
                        i = tag.end;
                        plain = i;
                        continue;
                    }
                    if (!tag.closing and !tag.self_closing) {
                        var matched = false;
                        for (inline_tags) |known| {
                            if (!std.ascii.eqlIgnoreCase(known.name, tag.name)) continue;
                            if (findClosingTag(text, tag.end, tag.name)) |close| {
                                try b.pushText(text[plain..i], base, url);
                                var style = base;
                                known.apply(&style);
                                var inner_url = url;
                                if (std.ascii.eqlIgnoreCase(tag.name, "a") and tag.href.len > 0) {
                                    style.link = true;
                                    inner_url = tag.href;
                                }
                                if (style.code) {
                                    try b.push(text[tag.end..close.start], style, inner_url);
                                } else {
                                    try parseInto(b, text[tag.end..close.start], style, inner_url);
                                }
                                i = close.end;
                                plain = i;
                                matched = true;
                                break;
                            }
                        }
                        if (matched) continue;
                    }
                    // unknown or unbalanced tag: drop the tag, keep content
                    try b.pushText(text[plain..i], base, url);
                    i = tag.end;
                    plain = i;
                    continue;
                }
                if (std.mem.startsWith(u8, text[i..], "<!--")) {
                    if (std.mem.indexOfPos(u8, text, i + 4, "-->")) |end| {
                        try b.pushText(text[plain..i], base, url);
                        i = end + 3;
                        plain = i;
                        continue;
                    }
                }
            },
            'h', 'w' => {
                if (!base.link) {
                    if (bareAutolinkEnd(text, i)) |end| {
                        try b.pushText(text[plain..i], base, url);
                        var style = base;
                        style.link = true;
                        const target = if (text[i] == 'w') try std.mem.concat(ctx.arena, u8, &.{ "http://", text[i..end] }) else text[i..end];
                        try b.push(text[i..end], style, target);
                        i = end;
                        plain = i;
                        continue;
                    }
                }
            },
            '@' => {
                if (!base.link) {
                    if (emailAutolink(text, i)) |email| {
                        if (email.start >= plain) {
                            try b.pushText(text[plain..email.start], base, url);
                            var style = base;
                            style.link = true;
                            const addr = text[email.start..email.end];
                            try b.push(addr, style, try std.mem.concat(ctx.arena, u8, &.{ "mailto:", addr }));
                            i = email.end;
                            plain = i;
                            continue;
                        }
                    }
                }
            },
            '\n' => {
                // soft break: drop trailing spaces; between CJK characters no space
                var line_end = i;
                while (line_end > plain and text[line_end - 1] == ' ') line_end -= 1;
                const hard = i - line_end >= 2;
                try b.pushText(text[plain..line_end], base, url);
                const prev = scan.cpBefore(text, line_end);
                var next_start = i + 1;
                while (next_start < text.len and text[next_start] == ' ') next_start += 1;
                const next = scan.cpAt(text, next_start);
                if (hard) {
                    try b.push("\n", base, url);
                } else if (!(scan.isCjk(prev) and scan.isCjk(next))) {
                    try b.push(" ", base, url);
                }
                i = next_start;
                plain = i;
                continue;
            },
            else => {},
        }
        i += 1;
    }
    try b.pushText(text[plain..], base, url);
}

fn collapseNewlines(arena: std.mem.Allocator, text: []const u8) ![]const u8 {
    if (std.mem.indexOfScalar(u8, text, '\n') == null) return text;
    const out = try arena.dupe(u8, text);
    for (out) |*c| if (c.* == '\n') {
        c.* = ' ';
    };
    return out;
}

/// `[text](url)`, `[text][ref]`, `[ref][]`, `[ref]`, `[^note]`, images.
/// Returns the index after the construct, or null when it is not a link.
fn parseLinkLike(b: *Builder, text: []const u8, start: usize, open: usize, is_image: bool, base: Style, url: []const u8, plain: usize) !?usize {
    const ctx = b.ctx;
    const close = matchBracket(text, open) orelse return null;
    const label = text[open + 1 .. close];

    // footnote reference [^label]
    if (!is_image and label.len > 1 and label[0] == '^' and at(text, close + 1) != ':' and
        std.mem.indexOfAny(u8, label, " \t\n") == null)
    {
        try b.pushText(text[plain..start], base, url);
        const name = label[1..];
        var number: usize = 0;
        for (ctx.footnote_order.items, 0..) |existing, idx| {
            if (std.mem.eql(u8, existing, name)) number = idx + 1;
        }
        if (number == 0) {
            try ctx.footnote_order.append(ctx.arena, name);
            number = ctx.footnote_order.items.len;
        }
        var style = base;
        style.footnote_ref = true;
        style.superscript = true;
        try b.push(try std.fmt.allocPrint(ctx.arena, "{d}", .{number}), style, name);
        return close + 1;
    }

    var target: ?LinkDef = null;
    var end = close + 1;
    if (at(text, close + 1) == '(') {
        if (parseDestination(text, close + 1)) |dest| {
            target = .{ .url = dest.url, .title = dest.title };
            end = dest.end;
        }
    }
    if (target == null and at(text, close + 1) == '[') {
        if (std.mem.indexOfScalarPos(u8, text, close + 2, ']')) |ref_close| {
            const ref = text[close + 2 .. ref_close];
            target = try lookupRef(ctx, if (ref.len == 0) label else ref);
            if (target != null) end = ref_close + 1;
        }
    }
    if (target == null) {
        target = try lookupRef(ctx, label);
        end = close + 1;
    }
    const t = target orelse return null;

    try b.pushText(text[plain..start], base, url);
    var style = base;
    if (is_image) {
        style.image = true;
        // alt text rendered plainly; renderer decides to load the image
        try b.push(if (label.len > 0) label else "image", style, t.url);
    } else {
        style.link = true;
        const target_url = if (std.mem.eql(u8, t.url, remend.incomplete_link)) "#incomplete" else t.url;
        try parseInto(b, label, style, target_url);
    }
    return end;
}

test "emphasis and code" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var refs: RefMap = .empty;
    var order: std.ArrayListUnmanaged([]const u8) = .empty;
    var ctx: Context = .{ .arena = arena.allocator(), .refs = &refs, .footnote_order = &order };
    const spans = try parse(&ctx, "a **b *c*** `d` snake_case_word ~~e~~");
    var bold_italic = false;
    var code = false;
    var strike = false;
    for (spans) |s| {
        if (s.style.bold and s.style.italic and std.mem.eql(u8, s.text, "c")) bold_italic = true;
        if (s.style.code) code = true;
        if (s.style.strike) strike = true;
        try std.testing.expect(!(s.style.italic and std.mem.eql(u8, s.text, "case")));
    }
    try std.testing.expect(bold_italic and code and strike);
}

test "cjk emphasis closes before ideographic text" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var refs: RefMap = .empty;
    var order: std.ArrayListUnmanaged([]const u8) = .empty;
    var ctx: Context = .{ .arena = arena.allocator(), .refs = &refs, .footnote_order = &order };
    const spans = try parse(&ctx, "**中文。**后文");
    try std.testing.expect(spans[0].style.bold);
    try std.testing.expectEqualStrings("中文。", spans[0].text);
}

test "links and autolinks" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var refs: RefMap = .empty;
    var order: std.ArrayListUnmanaged([]const u8) = .empty;
    var ctx: Context = .{ .arena = arena.allocator(), .refs = &refs, .footnote_order = &order };
    const spans = try parse(&ctx, "[a](http://x.com \"t\") and https://y.com/p, mail me@ex.com");
    var urls: usize = 0;
    for (spans) |s| if (s.style.link) {
        urls += 1;
    };
    try std.testing.expectEqual(@as(usize, 3), urls);
    try std.testing.expectEqualStrings("http://x.com", spans[0].url);
}
