const std = @import("std");
const zant = @import("zant");

test "markdown blocks and inline styles" {
    const source =
        \\# Title
        \\
        \\Hello **bold** and `code` and $x^2$.
        \\
        \\- one
        \\- two
        \\
        \\```zig
        \\const x = 1;
        \\```
    ;
    var doc = try zant.markdown.parse(std.testing.allocator, source);
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 4), doc.blocks.len);
    const heading = doc.blocks[0].heading;
    try std.testing.expectEqual(@as(u8, 1), heading.level);
    const para = doc.blocks[1].paragraph;
    var has_bold = false;
    var has_code = false;
    var has_math = false;
    for (para) |span| {
        if (span.style.bold) has_bold = true;
        if (span.style.code) has_code = true;
        if (span.style.math) has_math = true;
    }
    try std.testing.expect(has_bold and has_code and has_math);
    const list = doc.blocks[2].list;
    try std.testing.expectEqual(@as(usize, 2), list.items.len);
    try std.testing.expectEqualStrings("const x = 1;", doc.blocks[3].code.text);
}

test "markdown table and task list" {
    const source =
        \\| Name | Value |
        \\| --- | --- |
        \\| A | 1 |
        \\
        \\- [x] done
        \\- [ ] todo
    ;
    var doc = try zant.markdown.parse(std.testing.allocator, source);
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 2), doc.blocks.len);
    try std.testing.expectEqual(@as(usize, 2), doc.blocks[1].list.items.len);
    try std.testing.expect(doc.blocks[1].list.checked[0].?);
}

test "markdown document renders to Zant nodes" {
    var doc = try zant.markdown.parse(std.testing.allocator, "# Title\n\nHello **world**.");
    defer doc.deinit();
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const node = doc.render();
    try std.testing.expect(node.element.children.len > 0);
}

test "markdown display math block renders" {
    var doc = try zant.markdown.parse(std.testing.allocator, "$$\\frac{a}{b}$$");
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 1), doc.blocks.len);
    try std.testing.expectEqualStrings("\\frac{a}{b}", doc.blocks[0].math);
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    _ = doc.render();
}

test "math script formula renders" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    _ = zant.math.render(std.testing.allocator, "a^2 + b^2 = c^2");
}

test "math fraction uses native layout" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    _ = zant.math.render(std.testing.allocator, "\\frac{a}{b} + \\sqrt{x}");
}

test "inline math flattens to unicode" {
    const text = try zant.math.inlineText(std.testing.allocator, "a^2 + \\frac{a}{b}");
    defer std.testing.allocator.free(text);
    try std.testing.expect(std.mem.indexOf(u8, text, "a²") != null);
    try std.testing.expect(std.mem.indexOf(u8, text, "(a)/(b)") != null);
}

test "math matrix renders rows and columns" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    _ = zant.math.render(std.testing.allocator,
        "\\\\begin{matrix} a & b \\\\\\\\ c & d \\\\end{matrix}");
}

test "setext heading and gfm autolink" {
    const source = "Title\n=====\n\n<https://example.com>\n";
    var doc = try zant.markdown.parse(std.testing.allocator, source);
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 2), doc.blocks.len);
    try std.testing.expectEqual(@as(u8, 1), doc.blocks[0].heading.level);
    const para = doc.blocks[1].paragraph;
    try std.testing.expect(para.len > 0 and para[0].style.link);
}

test "indented code block" {
    var doc = try zant.markdown.parse(std.testing.allocator, "    let x = 1;\n    let y = 2;\n");
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 1), doc.blocks.len);
    try std.testing.expectEqualStrings("let x = 1;\nlet y = 2;", doc.blocks[0].code.text);
}

test "math radical has an overline" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    _ = zant.math.render(std.testing.allocator, "\\sqrt{x^2 + y^2}");
}

test "link reference definitions resolve" {
    const source = "[site]: https://example.com\n\nRead [the site][site].\n";
    var doc = try zant.markdown.parse(std.testing.allocator, source);
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 1), doc.blocks.len);
    const para = doc.blocks[0].paragraph;
    var linked = false;
    for (para) |span| if (span.style.link) { linked = true; };
    try std.testing.expect(linked);
}

test "nested blockquote" {
    var doc = try zant.markdown.parse(std.testing.allocator, "> outer\n>> inner\n");
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 1), doc.blocks.len);
    const outer = doc.blocks[0].quote;
    try std.testing.expectEqual(@as(usize, 2), outer.len);
    try std.testing.expectEqual(@as(usize, 1), outer[1].quote.len);
}

test "gfm table alignment" {
    const source = "| A | B |\n| :--- | ---: |\n| 1 | 2 |\n";
    var doc = try zant.markdown.parse(std.testing.allocator, source);
    defer doc.deinit();
    const table = doc.blocks[0].table;
    try std.testing.expectEqual(@as(usize, 2), table.aligns.len);
    try std.testing.expectEqual(.start, table.aligns[0]);
    try std.testing.expectEqual(.end, table.aligns[1]);
}

test "nested list indentation" {
    var doc = try zant.markdown.parse(std.testing.allocator, "- a\n  - b\n    - c\n");
    defer doc.deinit();
    const list = doc.blocks[0].list;
    try std.testing.expectEqual(@as(usize, 3), list.items.len);
    try std.testing.expectEqual(@as(u2, 0), list.indents[0]);
    try std.testing.expectEqual(@as(u2, 1), list.indents[1]);
    try std.testing.expectEqual(@as(u2, 2), list.indents[2]);
}

test "html entities decode in plain text" {
    var doc = try zant.markdown.parse(std.testing.allocator, "a &amp; b &lt;tag&gt;");
    defer doc.deinit();
    const para = doc.blocks[0].paragraph;
    var found_amp = false;
    var found_lt = false;
    for (para) |span| {
        if (std.mem.indexOf(u8, span.text, "&") != null) found_amp = true;
        if (std.mem.indexOf(u8, span.text, "<") != null) found_lt = true;
    }
    try std.testing.expect(found_amp and found_lt);
}

test "soft break becomes space and hard break keeps newline" {
    var doc = try zant.markdown.parse(std.testing.allocator, "one\ntwo  \nthree");
    defer doc.deinit();
    const para = doc.blocks[0].paragraph;
    var joined: [64]u8 = undefined;
    var n: usize = 0;
    for (para) |span| {
        for (span.text) |c| {
            if (n < joined.len) { joined[n] = c; n += 1; }
        }
    }
    try std.testing.expect(std.mem.indexOf(u8, joined[0..n], "one two") != null);
    try std.testing.expect(std.mem.indexOf(u8, joined[0..n], "three") != null);
}
