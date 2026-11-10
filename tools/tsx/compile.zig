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

pub const File = struct {
    /// Path as given on the command line (diagnostics use it).
    path: []const u8,
    src: []const u8,

    /// Hand-written Zig is copied, while TSX is compiled.
    pub fn isZig(self: File) bool {
        return std.mem.endsWith(u8, self.path, ".zig");
    }

    /// Both `App.tsx` and `App.zig` produce `App.zig` in the output directory.
    pub fn outputName(self: File, arena: std.mem.Allocator) error{OutOfMemory}![]const u8 {
        const base = std.fs.path.basename(self.path);
        return if (self.isZig()) base else std.fmt.allocPrint(arena, "{s}.zig", .{std.fs.path.stem(base)});
    }
};

pub const ProjectResult = union(enum) {
    /// Formatted TSX output or verbatim Zig source per file, in the order given.
    ok: []const []const u8,
    err: struct { file: usize, diag: Diag },
};

/// Compile project TSX with relative imports; parse hand-written Zig for
/// signatures and pass it through verbatim beside the generated modules.
pub fn compileProject(arena: std.mem.Allocator, files: []const File) error{OutOfMemory}!ProjectResult {
    const imports = @import("codegen/imports.zig");
    const zig_imports = @import("codegen/zig_imports.zig");
    var modules: std.ArrayList(imports.NamedModule) = .empty;
    var sources: std.ArrayList(zig_imports.Source) = .empty;
    const names = try arena.alloc([]const u8, files.len);
    for (files, 0..) |file, i| {
        names[i] = try file.outputName(arena);
        for (names[0..i]) |previous| if (std.mem.eql(u8, names[i], previous)) {
            return .{ .err = .{ .file = i, .diag = .{
                .loc = 0,
                .msg = try std.fmt.allocPrint(arena, "project output collision: `{s}`", .{names[i]}),
            } } };
        };
        const name = std.fs.path.stem(names[i]);
        if (file.isZig()) {
            const tree = try std.zig.Ast.parse(arena, try arena.dupeSentinel(u8, file.src, 0), .{});
            if (tree.errors.len > 0) {
                var aw: std.Io.Writer.Allocating = .init(arena);
                tree.renderError(tree.errors[0], &aw.writer) catch {};
                return .{ .err = .{ .file = i, .diag = .{
                    .loc = tree.tokenStart(tree.errors[0].token),
                    .msg = try arena.dupe(u8, aw.written()),
                } } };
            }
            try sources.append(arena, .{ .name = name, .tree = tree });
            continue;
        }
        var p = parser.Parser.init(arena, file.src);
        const module = p.parseModule() catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            error.Syntax => return .{ .err = .{ .file = i, .diag = p.diag.? } },
        };
        try modules.append(arena, .{ .name = name, .module = module });
    }
    const out = try arena.alloc([]const u8, files.len);
    var module_index: usize = 0;
    for (files, 0..) |file, i| {
        if (file.isZig()) {
            out[i] = file.src;
            continue;
        }
        var diag: ?Diag = null;
        const raw = codegen.generate(arena, file.src, modules.items[module_index].module, .{
            .source_name = std.fs.path.basename(file.path),
            .project = modules.items,
            .zig_sources = sources.items,
            .require_root = false,
        }, &diag) catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            error.Tsx => return .{ .err = .{ .file = i, .diag = diag.? } },
        };
        module_index += 1;
        out[i] = try format(arena, raw);
    }
    return .{ .ok = out };
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
    const view = std.unicode.Utf8View.init(pos.line_text) catch std.unicode.Utf8View.initUnchecked(pos.line_text);
    var it = view.iterator();
    while (col < pos.column) : (col += 1) {
        const cp = it.nextCodepoint() orelse break;
        try caret.append(arena, if (cp == '\t') '\t' else ' ');
    }
    return std.fmt.allocPrint(arena, "{s}:{d}:{d}: error: {s}\n    {s}\n    {s}^\n", .{
        path, pos.line, pos.column, diag.msg, pos.line_text, caret.items,
    });
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

test "a project resolves components and types across files" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const result = try compileProject(a, &.{
        .{ .path = "src/Row.tsx", .src =
        \\export interface Todo { id: u32; title: string }
        \\export interface RowProps { todo: Todo; onPick: (id: u32) => void }
        \\export function Row({ todo, onPick }: RowProps) {
        \\  return <Button onClick={() => onPick(todo.id)}>{todo.title}</Button>;
        \\}
        },
        .{ .path = "src/App.tsx", .src =
        \\import { useState } from "zant";
        \\import { Row, type Todo } from "./Row";
        \\export default function App() {
        \\  const [todo, setTodo] = useState<Todo>({ id: 1, title: "a" });
        \\  const [picked, setPicked] = useState<u32>(0);
        \\  return <Row todo={todo} onPick={(id) => setPicked(id)} />;
        \\}
        },
    });
    const files = result.ok;
    // Row.zig declares its types and the component, App.zig imports them
    try std.testing.expect(std.mem.indexOf(u8, files[0], "pub const Todo = struct") != null);
    try std.testing.expect(std.mem.indexOf(u8, files[0], "pub fn build(props: Props) ui.Node") != null);
    try std.testing.expect(std.mem.indexOf(u8, files[1], "const _Row = @import(\"Row.zig\");") != null);
    try std.testing.expect(std.mem.indexOf(u8, files[1], "pub const Todo = struct") == null);
    try std.testing.expect(std.mem.indexOf(u8, files[1], "tsx.State(_Row.Todo)") != null);
    try std.testing.expect(std.mem.indexOf(u8, files[1], "_Row.Row.build(.{") != null);
}

test "imports name exported things of project files" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compileProject(arena.allocator(), &.{
        .{ .path = "Row.tsx", .src = "function Hidden(p: { a: string }) { return <View/>; }" },
        .{
            .path = "App.tsx",
            .src = "import { Hidden } from \"./Row\"; export default function App() { return <View/>; }",
        },
    });
    try std.testing.expectEqual(@as(usize, 1), result.err.file);
    try std.testing.expectEqualStrings("`Hidden` is not exported by `./Row`", result.err.diag.msg);
}
