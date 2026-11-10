//! TSX source → formatted Zig source: parse, generate, then format with
//! `std.zig.Ast` (which also proves the output is syntactically valid).
const std = @import("std");
const parser = @import("parser.zig");
const codegen = @import("codegen.zig");

pub const Diag = parser.Diag;

pub const Result = union(enum) {
    ok: []const u8,
    /// A problem in the TSX source.
    err: Diag,
};

/// Everything lives in `arena`.
pub fn compile(arena: std.mem.Allocator, src: []const u8, source_name: []const u8) error{OutOfMemory}!Result {
    var p = parser.Parser.init(arena, src);
    const module = p.parseModule() catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        error.Syntax => return .{ .err = p.diag.? },
    };
    var diag: ?Diag = null;
    const raw = codegen.generate(arena, src, module, .{ .source_name = source_name }, &diag) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        error.Tsx => return .{ .err = diag.? },
    };
    return .{ .ok = try format(arena, raw) };
}

/// Formats generated Zig. Invalid output is a bug in the generator: it
/// panics with the code and Zig's message instead of hiding it.
pub fn format(arena: std.mem.Allocator, raw: []const u8) error{OutOfMemory}![]const u8 {
    const source = try arena.allocSentinel(u8, raw.len, 0);
    @memcpy(source, raw);
    var tree = try std.zig.Ast.parse(arena, source, .{});
    if (tree.errors.len > 0) {
        var aw: std.Io.Writer.Allocating = .init(arena);
        tree.renderError(tree.errors[0], &aw.writer) catch {};
        const tok = tree.errors[0].token;
        const loc = tree.tokenLocation(0, tok);
        std.debug.panic("zant-tsx generated invalid Zig (line {d}: {s}):\n{s}", .{ loc.line + 1, aw.written(), raw });
    }
    return tree.renderAlloc(arena);
}

pub const Position = struct {
    line: usize,
    /// 1-based, in Unicode scalar values.
    column: usize,
    line_text: []const u8,
};

pub fn position(src: []const u8, loc: u32) Position {
    const at = @min(loc, src.len);
    const line_start = if (std.mem.lastIndexOfScalar(u8, src[0..at], '\n')) |i| i + 1 else 0;
    const line_end = std.mem.indexOfScalarPos(u8, src, at, '\n') orelse src.len;
    const line = std.mem.count(u8, src[0..line_start], "\n") + 1;
    const column = (std.unicode.utf8CountCodepoints(src[line_start..at]) catch at - line_start) + 1;
    return .{ .line = line, .column = column, .line_text = std.mem.trimEnd(u8, src[line_start..line_end], "\r") };
}

/// `path:line:col: error: msg` followed by the source line and a caret,
/// the way Zig reports errors.
pub fn renderDiag(arena: std.mem.Allocator, path: []const u8, src: []const u8, diag: Diag) ![]const u8 {
    const pos = position(src, diag.loc);
    var caret: std.ArrayList(u8) = .empty;
    var col: usize = 1;
    var it = (std.unicode.Utf8View.init(pos.line_text) catch std.unicode.Utf8View.initUnchecked(pos.line_text)).iterator();
    while (col < pos.column) : (col += 1) {
        const cp = it.nextCodepoint() orelse break;
        try caret.append(arena, if (cp == '\t') '\t' else ' ');
    }
    return std.fmt.allocPrint(arena, "{s}:{d}:{d}: error: {s}\n    {s}\n    {s}^\n", .{ path, pos.line, pos.column, diag.msg, pos.line_text, caret.items });
}

test "positions count lines and code points" {
    const src = "a\nbé c";
    const pos = position(src, 6); // `c` (é is two bytes)
    try std.testing.expectEqual(@as(usize, 2), pos.line);
    try std.testing.expectEqual(@as(usize, 4), pos.column);
    try std.testing.expectEqualStrings("bé c", pos.line_text);
}

test "syntax errors render with a caret" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const src = "function App() {\n  return <View></Text>;\n}\n";
    const result = try compile(a, src, "app.tsx");
    const text = try renderDiag(a, "app.tsx", src, result.err);
    try std.testing.expectEqualStrings(
        "app.tsx:2:18: error: expected `</View>` to close <View>\n" ++
            "      return <View></Text>;\n" ++
            "                     ^\n",
        text,
    );
}
