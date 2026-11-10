//! XHTML names and SWC's numeric reference rules, for text and attributes.
const std = @import("std");
const decode = @import("jsx_text.zig").decodeEntities;
const compiler = @import("compile.zig");
const Parser = @import("parser.zig").Parser;

fn expectDecoded(raw: []const u8, expected: []const u8) !void {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    try std.testing.expectEqualStrings(expected, try decode(arena.allocator(), raw));
}

test "XHTML named references cover Latin, Greek, symbols and special characters" {
    try expectDecoded("&copy; 2026 &mdash; &Alpha; &alpha; &euro; &hearts; &OElig; &AElig;", "© 2026 — Α α € ♥ Œ Æ");
    try expectDecoded("&amp;&lt;&gt;&quot;&apos;&nbsp;&ensp;&zwnj;&forall;&frac12;", "&<>\"'\u{a0}\u{2002}\u{200c}∀½");
    try expectDecoded("&AMP; &Copy; &unknown; &lbrace; &rbrace;", "&AMP; &Copy; &unknown; &lbrace; &rbrace;");
}

test "decimal and lowercase-x hexadecimal numeric references" {
    try expectDecoded("&#169; 2026 &#8212; &#128512;", "© 2026 — 😀");
    try expectDecoded("&#xA9; &#x00a9; &#x1F600; &#x10FFFF;", "© © 😀 \u{10ffff}");
    try expectDecoded("&#0;|&#x0;|&#1114111;", "\x00|\x00|\u{10ffff}");
    try expectDecoded("&#000000000000000000000000000000065; &#x00000000000000041;", "A A");
}

test "invalid numeric and non-alphanumeric named references stay literal" {
    const invalid = [_][]const u8{
        "&#;",
        "&#x;",
        "&#X41;",
        "&#xg;",
        "&#12a;",
        "&#-1;",
        "&#+65;",
        "&#x+41;",
        "&# 65;",
        "&#1_0;",
        "&#0x41;",
        "&#x110000;",
        "&#1114112;",
        "&#99999999999999999999999999999;",
        "&co_py;",
        "&co-py;",
        "&co py;",
        "&;",
        "&é;",
    };
    for (invalid) |raw| try expectDecoded(raw, raw);
    try expectDecoded("&bad&copy; &#x&copy; &amp;&copy;", "&bad© &#x© &©");
}

test "unterminated references and ampersands stay literal" {
    for ([_][]const u8{ "&", "&amp", "&#", "&#12", "&#x", "&#x41", "a & b", "&copy rest;" }) |raw| {
        try expectDecoded(raw, raw);
    }
    try expectDecoded("&amp;copy;", "&copy;"); // one pass, never recursively decode
}

test "surrogates follow SWC JavaScript code points, pairing only adjacent halves" {
    try expectDecoded("&#xD800;|&#55296;|&#xDFFF;|&#57343;", "\xed\xa0\x80|\xed\xa0\x80|\xed\xbf\xbf|\xed\xbf\xbf");
    try expectDecoded("&#xD83D;&#56832; &#55296;&#xDC00; &#xDBFF;&#xDFFF;", "😀 \u{10000} \u{10ffff}");
    try expectDecoded("&#xD800; &#xDC00;", "\xed\xa0\x80 \xed\xb0\x80");
    try expectDecoded("&#xDC00;&#xD800;", "\xed\xb0\x80\xed\xa0\x80");
    try expectDecoded("&#xD800;&#xD83D;&#xDE00;", "\xed\xa0\x80😀");
}

test "attribute strings use the same entity decoder as JSX text" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var p = Parser.init(
        arena.allocator(),
        "function App() { return <Input placeholder=\"&copy; &#x1F600; &#X41; &unknown;\"/>; }",
    );
    const module = try p.parseModule();
    const jsx = module.functions[0].func.body.block[0].data.ret.?.data.jsx;
    try std.testing.expectEqualStrings("© 😀 &#X41; &unknown;", jsx.attrs[0].value.?.data.string);
}

test "entities compile to UTF-8 and lone surrogates to valid Zig byte escapes" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compiler.compile(arena.allocator(), @embedFile("testdata/entities.tsx"), "entities.tsx");
    if (result == .err) {
        std.debug.print("unexpected diagnostic: {s}\n", .{result.err.msg});
        return error.UnexpectedDiagnostic;
    }
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "ui.text(\"© 2026 — 😀\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, ".placeholder = \"© © 😀 & &unknown;\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "\\xed\\xa0\\x80|\\xed\\xbf\\xbf|") != null);
    try std.testing.expect(std.unicode.utf8ValidateSlice(result.ok));
}
