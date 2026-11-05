const std = @import("std");
const zant = @import("zant");
const md = zant.markdown;

test {
    // run the module's own unit tests (parser, remend, blocks, cache, …)
    _ = md;
    _ = zant.math;
}

test "markdown blocks and inline styles" {
    const source =
        \\# Title
        \\
        \\Hello **bold** and `code` and $$x^2$$.
        \\
        \\- one
        \\- two
        \\
        \\```zig
        \\const x = 1;
        \\```
    ;
    var doc = try md.parse(std.testing.allocator, source);
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 4), doc.blocks.len);
    try std.testing.expectEqual(@as(u8, 1), doc.blocks[0].heading.level);
    var has_bold = false;
    var has_code = false;
    var has_math = false;
    for (doc.blocks[1].paragraph) |span| {
        if (span.style.bold) has_bold = true;
        if (span.style.code) has_code = true;
        if (span.style.math) has_math = true;
    }
    try std.testing.expect(has_bold and has_code and has_math);
    try std.testing.expectEqual(@as(usize, 2), doc.blocks[2].list.items.len);
    try std.testing.expectEqualStrings("const x = 1;", doc.blocks[3].code.text);
    try std.testing.expectEqualStrings("zig", doc.blocks[3].code.lang);
}

test "markdown table and task list" {
    const source =
        \\| Name | Value |
        \\| --- | ---: |
        \\| A | 1 |
        \\
        \\- [x] done
        \\- [ ] todo
    ;
    var doc = try md.parse(std.testing.allocator, source);
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 2), doc.blocks.len);
    const table = doc.blocks[0].table;
    try std.testing.expectEqual(md.ast.Align.right, table.aligns[1]);
    try std.testing.expectEqualStrings("A", table.raw_rows[0][0]);
    try std.testing.expect(doc.blocks[1].list.items[0].checked.?);
    try std.testing.expect(!doc.blocks[1].list.items[1].checked.?);
}

test "setext heading, autolinks, indented code" {
    var doc = try md.parse(std.testing.allocator, "Title\n=====\n\n<https://example.com>\n\n    let x = 1;\n    let y = 2;\n");
    defer doc.deinit();
    try std.testing.expectEqual(@as(usize, 3), doc.blocks.len);
    try std.testing.expectEqual(@as(u8, 1), doc.blocks[0].heading.level);
    try std.testing.expect(doc.blocks[1].paragraph[0].style.link);
    try std.testing.expectEqualStrings("let x = 1;\nlet y = 2;", doc.blocks[2].code.text);
}

test "nested blockquote and nested lists" {
    var doc = try md.parse(std.testing.allocator, "> outer\n>> inner\n\n- a\n  - b\n    - c\n");
    defer doc.deinit();
    const outer = doc.blocks[0].quote;
    try std.testing.expectEqual(@as(usize, 2), outer.len);
    try std.testing.expectEqual(@as(usize, 1), outer[1].quote.len);
    const level1 = doc.blocks[1].list;
    const level2 = level1.items[0].blocks[1].list;
    const level3 = level2.items[0].blocks[1].list;
    try std.testing.expectEqual(@as(usize, 1), level3.items.len);
}

test "entities, escapes, breaks" {
    var doc = try md.parse(std.testing.allocator, "a &amp; b &lt;tag&gt; \\*not italic\\* `a < b`\none\ntwo  \nthree");
    defer doc.deinit();
    var joined: std.ArrayListUnmanaged(u8) = .empty;
    defer joined.deinit(std.testing.allocator);
    for (doc.blocks[0].paragraph) |span| {
        try std.testing.expect(!span.style.italic);
        try joined.appendSlice(std.testing.allocator, span.text);
    }
    try std.testing.expect(std.mem.indexOf(u8, joined.items, "a & b <tag>") != null);
    try std.testing.expect(std.mem.indexOf(u8, joined.items, "one two") != null);
    try std.testing.expect(std.mem.indexOf(u8, joined.items, "two\nthree") != null);
}

test "thematic break and html block" {
    var doc = try md.parse(std.testing.allocator, "a\n\n* * *\n\n<div>hello</div>\n");
    defer doc.deinit();
    try std.testing.expectEqual(md.Block.rule, doc.blocks[1]);
    try std.testing.expectEqualStrings("<div>hello</div>", doc.blocks[2].html);
}

pub const full_sample =
    \\---
    \\title: Sample
    \\---
    \\
    \\# Heading with $$E=mc^2$$
    \\
    \\Paragraph with **bold**, *italic*, ~~strike~~, ==mark==, `code`, [link](https://example.com), <https://zant.dev>, a footnote[^1] and $$\frac{a}{b}$$ inline.
    \\
    \\> [!TIP]
    \\> A callout with **markdown**.
    \\
    \\> A quote
    \\> > nested
    \\
    \\1. first
    \\2. second
    \\   - [x] task
    \\   - [ ] todo
    \\
    \\| Left | Center | Right |
    \\| :--- | :---: | ---: |
    \\| a | b | c |
    \\
    \\```ts
    \\const x: number = 42; // answer
    \\function f(a: string) { return `v${a}`; }
    \\```
    \\
    \\```mermaid
    \\graph TD
    \\  A[Start] --> B{Ok?}
    \\  B -->|yes| C(Done)
    \\  B -->|no| A
    \\```
    \\
    \\```mermaid
    \\sequenceDiagram
    \\  Alice->>Bob: Hello
    \\  Bob-->>Alice: Hi
    \\```
    \\
    \\```mermaid
    \\pie title Pets
    \\  "Dogs" : 386
    \\  "Cats" : 85
    \\```
    \\
    \\$$
    \\\sum_{k=1}^{n} k = \frac{n(n+1)}{2}
    \\$$
    \\
    \\![alt text](https://example.com/a.png)
    \\
    \\---
    \\
    \\[^1]: The footnote.
;

test "full document renders" {
    var doc = try md.parse(std.testing.allocator, full_sample);
    defer doc.deinit();
    try std.testing.expectEqualStrings("title: Sample", doc.front_matter);
    try std.testing.expectEqual(@as(usize, 1), doc.footnotes.len);
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const node = md.renderDocument(&doc, .{ .is_animating = true, .caret = .block });
    try std.testing.expect(node.element.children.len > 0);
    const dark = md.renderDocument(&doc, .{ .theme = md.theme.dark(), .labels = md.Labels.zh });
    try std.testing.expect(dark.element.children.len > 0);
}

test "streamdown view memoises blocks" {
    var cache = md.Cache.init(std.testing.allocator);
    defer cache.deinit();
    zant.ui.beginFrame();
    const first = md.streamdown(&cache, "# A\n\npara **bo", .{ .mode = .streaming, .render = .{ .is_animating = true, .caret = .block } });
    const first_hash = first.element.children[0].element.memo_hash.?;
    zant.ui.endFrame();
    try std.testing.expectEqual(@as(usize, 2), first.element.children.len);
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const second = md.streamdown(&cache, "# A\n\npara **bold** done", .{ .mode = .streaming, .render = .{ .is_animating = true, .caret = .block } });
    // the unchanged heading block keeps its memo hash
    try std.testing.expectEqual(first_hash, second.element.children[0].element.memo_hash.?);
}

test "block keys are stable across insertions and resolvable by offset" {
    var cache = md.Cache.init(std.testing.allocator);
    defer cache.deinit();
    const src1 = "# A\n\nOne.\n\n## B\n\nTwo.";
    zant.ui.beginFrame();
    const first = md.streamdown(&cache, src1, .{});
    const heading_key = try std.testing.allocator.dupe(u8, first.element.children[2].element.key.?);
    defer std.testing.allocator.free(heading_key);
    zant.ui.endFrame();
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const src2 = "# A\n\nInserted.\n\nOne.\n\n## B\n\nTwo.";
    const second = md.streamdown(&cache, src2, .{});
    try std.testing.expectEqualStrings(heading_key, second.element.children[3].element.key.?);
    var buf: [64]u8 = undefined;
    const offset = std.mem.indexOf(u8, src2, "## B").?;
    try std.testing.expectEqualStrings(heading_key, md.blockKeyAtOffset(&cache, "md", offset + 2, &buf).?);
}

test "editor renders in every mode" {
    var cache = md.Cache.init(std.testing.allocator);
    defer cache.deinit();
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    inline for (.{ md.EditorMode.edit, md.EditorMode.split, md.EditorMode.preview }) |mode| {
        const node = md.editor(.{ .value = "# Hi\n\n- a", .mode = mode, .cache = &cache, .labels = md.EditorLabels.zh });
        try std.testing.expect(node.element.children.len == 2);
    }
}

test "link safety modal" {
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    const node = md.linkSafetyModal(.{ .url = "https://example.com", .labels = md.Labels.zh });
    try std.testing.expect(node.element.layout.absolute);
}

test "heading keys resolve to rendered heading nodes, also in single-block documents" {
    var cache = md.Cache.init(std.testing.allocator);
    defer cache.deinit();
    // the footnote keeps the whole document in one block
    const src = "# A\n\ntext[^1]\n\n## B\n\nmore\n\n[^1]: note";
    zant.ui.beginFrame();
    defer zant.ui.endFrame();
    _ = md.streamdown(&cache, src, .{});
    try std.testing.expectEqual(@as(usize, 1), cache.entries.items.len);
    var buf: [64]u8 = undefined;
    const key = md.headingKey(&cache, "md", 1, &buf).?;
    var expect_buf: [64]u8 = undefined;
    const expected = try std.fmt.bufPrint(&expect_buf, "md-e{d}-2", .{cache.entries.items[0].id});
    try std.testing.expectEqualStrings(expected, key);
    try std.testing.expect(md.headingKey(&cache, "md", 2, &buf) == null);
}
