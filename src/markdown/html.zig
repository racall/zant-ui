//! HTML export of a parsed document (GFM-style markup). `standalone`
//! wraps it in a complete page with readable default styles and KaTeX
//! (auto-render) for math; otherwise only the body fragment is produced.
const std = @import("std");
const ast = @import("ast.zig");

pub const Options = struct {
    /// Full HTML page (head, styles, KaTeX) instead of a fragment.
    standalone: bool = true,
    title: []const u8 = "Document",
    /// Dark page colours.
    dark: bool = false,
};

const Writer = struct {
    out: std.ArrayListUnmanaged(u8) = .empty,
    allocator: std.mem.Allocator,

    fn raw(self: *Writer, s: []const u8) !void {
        try self.out.appendSlice(self.allocator, s);
    }

    fn print(self: *Writer, comptime fmt: []const u8, args: anytype) !void {
        const s = try std.fmt.allocPrint(self.allocator, fmt, args);
        defer self.allocator.free(s);
        try self.raw(s);
    }

    /// Escapes text for element content and attribute values.
    fn text(self: *Writer, s: []const u8) !void {
        for (s) |c| switch (c) {
            '&' => try self.raw("&amp;"),
            '<' => try self.raw("&lt;"),
            '>' => try self.raw("&gt;"),
            '"' => try self.raw("&quot;"),
            else => try self.out.append(self.allocator, c),
        };
    }
};

/// Renders `doc` as HTML (caller owns the result).
pub fn toHtml(allocator: std.mem.Allocator, doc: *const ast.Document, options: Options) ![]u8 {
    var w: Writer = .{ .allocator = allocator };
    errdefer w.out.deinit(allocator);
    if (options.standalone) try head(&w, options);
    try blocks(&w, doc.blocks);
    if (doc.footnotes.len > 0) {
        try w.raw("<section class=\"footnotes\"><hr><ol>\n");
        for (doc.footnotes) |f| {
            try w.raw("<li id=\"fn-");
            try w.text(f.label);
            try w.raw("\">");
            try blocks(&w, f.blocks);
            try w.raw("</li>\n");
        }
        try w.raw("</ol></section>\n");
    }
    if (options.standalone) try w.raw("</article>\n</body>\n</html>\n");
    return w.out.toOwnedSlice(allocator);
}

fn head(w: *Writer, o: Options) !void {
    const bg = if (o.dark) "#1b1d1c" else "#fcfcfa";
    const fg = if (o.dark) "#e4e6e1" else "#292d2a";
    const muted = if (o.dark) "#9aa19b" else "#7d837e";
    const line = if (o.dark) "#333734" else "#e9ebe7";
    const code_bg = if (o.dark) "#202321" else "#f4f5f2";
    try w.raw("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>");
    try w.text(o.title);
    try w.raw("</title>\n");
    try w.raw("<link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css\">\n");
    try w.raw("<script defer src=\"https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js\"></script>\n");
    try w.raw("<script defer src=\"https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js\" onload=\"renderMathInElement(document.body,{delimiters:[{left:'\\\\[',right:'\\\\]',display:true},{left:'\\\\(',right:'\\\\)',display:false}]})\"></script>\n");
    try w.print(
        \\<style>
        \\body{{margin:0;background:{s};color:{s};font:17px/1.8 -apple-system,"PingFang SC","Helvetica Neue",sans-serif}}
        \\article{{max-width:720px;margin:0 auto;padding:56px 24px 96px}}
        \\h1,h2,h3,h4{{font-family:"Songti SC",Georgia,serif;font-weight:500;line-height:1.3;margin:1.6em 0 .6em}}
        \\a{{color:inherit;text-underline-offset:3px}}
        \\blockquote{{margin:1em 0;padding:0 1em;border-left:3px solid {s};color:{s}}}
        \\code{{font:.9em ui-monospace,Menlo,monospace;background:{s};padding:.1em .35em;border-radius:4px}}
        \\pre{{background:{s};border:1px solid {s};border-radius:8px;padding:14px 16px;overflow:auto}}
        \\pre code{{background:none;padding:0}}
        \\table{{border-collapse:collapse;margin:1em 0}}th,td{{border:1px solid {s};padding:6px 12px}}
        \\hr{{border:0;border-top:1px solid {s};margin:2em 0}}img{{max-width:100%}}
        \\.callout{{border-left:3px solid {s};padding:.2em 1em;margin:1em 0}}.callout-title{{font-weight:600}}
        \\li.task{{list-style:none;margin-left:-1.4em}}
        \\</style>
        \\</head>
        \\<body>
        \\<article>
        \\
    , .{ bg, fg, muted, muted, code_bg, code_bg, line, line, line, muted });
}

fn blocks(w: *Writer, list: []const ast.Block) anyerror!void {
    for (list) |b| try block(w, b);
}

fn block(w: *Writer, b: ast.Block) anyerror!void {
    switch (b) {
        .heading => |h| {
            try w.print("<h{d}", .{h.level});
            if (h.id.len > 0) {
                try w.raw(" id=\"");
                try w.text(h.id);
                try w.raw("\"");
            }
            try w.raw(">");
            try spans(w, h.spans);
            try w.print("</h{d}>\n", .{h.level});
        },
        .paragraph => |s| {
            try w.raw("<p>");
            try spans(w, s);
            try w.raw("</p>\n");
        },
        .code => |c| {
            try w.raw("<pre><code");
            if (c.lang.len > 0) {
                try w.raw(" class=\"language-");
                try w.text(c.lang);
                try w.raw("\"");
            }
            try w.raw(">");
            try w.text(c.text);
            try w.raw("</code></pre>\n");
        },
        .quote => |inner| {
            try w.raw("<blockquote>\n");
            try blocks(w, inner);
            try w.raw("</blockquote>\n");
        },
        .callout => |c| {
            try w.raw("<div class=\"callout\"><div class=\"callout-title\">");
            try w.text(c.kind.label());
            try w.raw("</div>\n");
            try blocks(w, c.blocks);
            try w.raw("</div>\n");
        },
        .list => |l| {
            if (l.ordered) {
                if (l.start != 1) try w.print("<ol start=\"{d}\">\n", .{l.start}) else try w.raw("<ol>\n");
            } else try w.raw("<ul>\n");
            for (l.items) |item| {
                if (item.checked) |checked| {
                    try w.raw(if (checked) "<li class=\"task\"><input type=\"checkbox\" checked disabled> " else "<li class=\"task\"><input type=\"checkbox\" disabled> ");
                } else try w.raw("<li>");
                // tight lists: inline the single paragraph
                if (l.tight and item.blocks.len > 0 and item.blocks[0] == .paragraph) {
                    try spans(w, item.blocks[0].paragraph);
                    try blocks(w, item.blocks[1..]);
                } else try blocks(w, item.blocks);
                try w.raw("</li>\n");
            }
            try w.raw(if (l.ordered) "</ol>\n" else "</ul>\n");
        },
        .table => |t| {
            try w.raw("<table>\n<thead><tr>");
            for (t.headers, 0..) |cell, i| {
                try cellOpen(w, "th", t.aligns, i);
                try spans(w, cell);
                try w.raw("</th>");
            }
            try w.raw("</tr></thead>\n<tbody>\n");
            for (t.rows) |row| {
                try w.raw("<tr>");
                for (row, 0..) |cell, i| {
                    try cellOpen(w, "td", t.aligns, i);
                    try spans(w, cell);
                    try w.raw("</td>");
                }
                try w.raw("</tr>\n");
            }
            try w.raw("</tbody>\n</table>\n");
        },
        .math => |m| {
            try w.raw("<p class=\"math\">\\[");
            try w.text(m.source);
            try w.raw("\\]</p>\n");
        },
        .image => |img| {
            try w.raw("<p><img src=\"");
            try w.text(img.url);
            try w.raw("\" alt=\"");
            try w.text(img.alt);
            try w.raw("\"></p>\n");
        },
        // raw HTML blocks are escaped: exported pages never run embedded markup
        .html => |h| {
            try w.raw("<pre>");
            try w.text(h);
            try w.raw("</pre>\n");
        },
        .rule => try w.raw("<hr>\n"),
    }
}

fn cellOpen(w: *Writer, comptime tag: []const u8, aligns: []const ast.Align, i: usize) !void {
    const a: ast.Align = if (i < aligns.len) aligns[i] else .none;
    switch (a) {
        .none => try w.raw("<" ++ tag ++ ">"),
        .left => try w.raw("<" ++ tag ++ " style=\"text-align:left\">"),
        .center => try w.raw("<" ++ tag ++ " style=\"text-align:center\">"),
        .right => try w.raw("<" ++ tag ++ " style=\"text-align:right\">"),
    }
}

fn spans(w: *Writer, list: []const ast.Span) !void {
    for (list) |s| try span(w, s);
}

fn span(w: *Writer, s: ast.Span) !void {
    const st = s.style;
    if (st.marker) return;
    if (st.math) {
        try w.raw("\\(");
        try w.text(s.text);
        try w.raw("\\)");
        return;
    }
    if (st.image) {
        try w.raw("<img src=\"");
        try w.text(s.url);
        try w.raw("\" alt=\"");
        try w.text(s.text);
        try w.raw("\">");
        return;
    }
    if (st.footnote_ref) {
        try w.raw("<sup><a href=\"#fn-");
        try w.text(s.url);
        try w.raw("\">");
        try w.text(s.text);
        try w.raw("</a></sup>");
        return;
    }
    if (st.link) {
        try w.raw("<a href=\"");
        try w.text(s.url);
        try w.raw("\">");
    }
    if (st.bold) try w.raw("<strong>");
    if (st.italic) try w.raw("<em>");
    if (st.strike) try w.raw("<del>");
    if (st.underline) try w.raw("<u>");
    if (st.highlight) try w.raw("<mark>");
    if (st.superscript) try w.raw("<sup>");
    if (st.subscript) try w.raw("<sub>");
    if (st.code) try w.raw("<code>");
    // soft line breaks stay newlines; hard breaks become <br>
    for (s.text) |c| {
        if (c == '\n') try w.raw("\n") else try w.text(&.{c});
    }
    if (st.code) try w.raw("</code>");
    if (st.subscript) try w.raw("</sub>");
    if (st.superscript) try w.raw("</sup>");
    if (st.highlight) try w.raw("</mark>");
    if (st.underline) try w.raw("</u>");
    if (st.strike) try w.raw("</del>");
    if (st.italic) try w.raw("</em>");
    if (st.bold) try w.raw("</strong>");
    if (st.link) try w.raw("</a>");
}

test "html export covers the common blocks and escapes text" {
    const parser = @import("parser/mod.zig");
    var doc = try parser.parse(std.testing.allocator,
        \\# Title a < b & c
        \\
        \\Some **bold** and $x^2$ & [a link](https://e.com).
        \\
        \\- [x] done
        \\- open
        \\
        \\```zig
        \\a < b
        \\```
        \\
        \\$$
        \\\frac{1}{2}
        \\$$
    , .{});
    defer doc.deinit();
    const html = try toHtml(std.testing.allocator, &doc, .{ .standalone = false });
    defer std.testing.allocator.free(html);
    try std.testing.expect(std.mem.indexOf(u8, html, "Title a &lt; b &amp; c</h1>") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "<strong>bold</strong>") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "\\(x^2\\)") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "&amp;") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "<a href=\"https://e.com\">a link</a>") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "type=\"checkbox\" checked") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "class=\"language-zig\">a &lt; b") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "\\[\\frac{1}{2}") != null);
    const page = try toHtml(std.testing.allocator, &doc, .{ .title = "T" });
    defer std.testing.allocator.free(page);
    try std.testing.expect(std.mem.startsWith(u8, page, "<!doctype html>"));
    try std.testing.expect(std.mem.endsWith(u8, page, "</html>\n"));
}
