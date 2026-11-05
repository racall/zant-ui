//! Markdown → `ast.Document`.
const std = @import("std");
const ast = @import("../ast.zig");
pub const block = @import("block.zig");
pub const inline_parser = @import("inline.zig");

pub const Options = block.Options;
pub const LineRange = block.LineRange;

/// Parses `source` into a document that owns all its memory.
pub fn parse(allocator: std.mem.Allocator, source: []const u8, options: Options) !ast.Document {
    var arena = std.heap.ArenaAllocator.init(allocator);
    errdefer arena.deinit();
    var parser = block.Parser.init(arena.allocator(), options);
    const parts = try parser.parseDocument(source, null);
    return .{
        .arena = arena,
        .blocks = parts.blocks,
        .footnotes = parts.footnotes,
        .front_matter = parts.front_matter,
    };
}

/// Top-level block line ranges, without parsing inline content.
pub fn topLevelRanges(arena: std.mem.Allocator, source: []const u8, options: Options) ![]const LineRange {
    var parser = block.Parser.init(arena, options);
    parser.structure_only = true;
    var ranges: std.ArrayListUnmanaged(LineRange) = .empty;
    _ = try parser.parseDocument(source, &ranges);
    return ranges.toOwnedSlice(arena);
}

test {
    _ = inline_parser;
}

test "nested lists with tasks and loose items" {
    var doc = try parse(std.testing.allocator, "- a\n  - [x] b\n    1. c\n\n- d\n", .{});
    defer doc.deinit();
    const list = doc.blocks[0].list;
    try std.testing.expect(!list.tight);
    try std.testing.expectEqual(@as(usize, 2), list.items.len);
    const nested = list.items[0].blocks[1].list;
    try std.testing.expect(nested.items[0].checked.?);
    try std.testing.expect(nested.items[0].blocks[1].list.ordered);
}

test "footnotes, callouts, front matter" {
    const src = "---\ntitle: x\n---\n\n> [!WARNING]\n> careful\n\nText[^n].\n\n[^n]: The note.\n";
    var doc = try parse(std.testing.allocator, src, .{});
    defer doc.deinit();
    try std.testing.expectEqualStrings("title: x", doc.front_matter);
    try std.testing.expectEqual(ast.CalloutKind.warning, doc.blocks[0].callout.kind);
    try std.testing.expectEqual(@as(usize, 1), doc.footnotes.len);
    try std.testing.expectEqual(@as(usize, 1), doc.footnotes[0].number);
}

test "top level ranges" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const ranges = try topLevelRanges(arena.allocator(), "# A\n\npara\nmore\n\n```\nx\n\ny\n```\n- a\n- b\n", .{});
    try std.testing.expectEqual(@as(usize, 4), ranges.len);
    try std.testing.expectEqual(@as(usize, 2), ranges[1].start);
    try std.testing.expectEqual(@as(usize, 4), ranges[1].end);
    try std.testing.expectEqual(@as(usize, 10), ranges[2].end);
}

test "remark-math syntax" {
    var doc = try parse(std.testing.allocator, "$20 and $10\n\nDisplay: $$E = mc^2$$\n\n$$\n\\frac{a}{b}\n$$\n\n$$x$$\n", .{});
    defer doc.deinit();
    for (doc.blocks[0].paragraph) |s| try std.testing.expect(!s.style.math);
    var inline_math = false;
    for (doc.blocks[1].paragraph) |s| if (s.style.math) {
        inline_math = true;
    };
    try std.testing.expect(inline_math);
    try std.testing.expectEqualStrings("\\frac{a}{b}", doc.blocks[2].math.source);
    // single-line $$x$$ is text math inside a paragraph (meta may not contain $)
    try std.testing.expect(doc.blocks[3] == .paragraph);
}

fn mathSpans(doc: *const ast.Document, out: *[8][]const u8) usize {
    var n: usize = 0;
    for (doc.blocks[0].paragraph) |s| {
        if (s.style.math and n < out.len) {
            out[n] = s.text;
            n += 1;
        }
    }
    return n;
}

test "single dollar inline math (pandoc rules)" {
    var out: [8][]const u8 = undefined;
    {
        var doc = try parse(std.testing.allocator, "其中 $U=[u_1\\ \\cdots\\ u_m]$，$V=[v_1\\ \\cdots\\ v_m]$。", .{});
        defer doc.deinit();
        try std.testing.expectEqual(@as(usize, 2), mathSpans(&doc, &out));
        try std.testing.expectEqualStrings("U=[u_1\\ \\cdots\\ u_m]", out[0]);
        try std.testing.expectEqualStrings("V=[v_1\\ \\cdots\\ v_m]", out[1]);
    }
    for ([_][]const u8{ "$20 and $10", "price $5-$10", "cost is $" }) |src| {
        var doc = try parse(std.testing.allocator, src, .{});
        defer doc.deinit();
        try std.testing.expectEqual(@as(usize, 0), mathSpans(&doc, &out));
    }
    for ([_][]const u8{ "a $x$ b", "\\$5 and $x$" }) |src| {
        var doc = try parse(std.testing.allocator, src, .{});
        defer doc.deinit();
        try std.testing.expectEqual(@as(usize, 1), mathSpans(&doc, &out));
        try std.testing.expectEqualStrings("x", out[0]);
    }
}
