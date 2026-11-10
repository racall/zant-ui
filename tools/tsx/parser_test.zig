//! Parser tests (lexer + statements + expressions + JSX).
const std = @import("std");
const ast = @import("ast.zig");
const parser = @import("parser.zig");
const jsx_text = @import("jsx_text.zig");
const Parser = parser.Parser;
const cleanJsxText = jsx_text.cleanJsxText;
const decodeEntities = jsx_text.decodeEntities;

fn parseOk(arena: std.mem.Allocator, src: []const u8) !ast.Module {
    var p = Parser.init(arena, src);
    return p.parseModule() catch |err| {
        if (p.diag) |d| std.debug.print("parse error at {d}: {s}\n", .{ d.loc, d.msg });
        return err;
    };
}

test "jsx text whitespace follows React" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    try std.testing.expectEqualStrings("Count: ", (try cleanJsxText(a, "Count: ")).?);
    try std.testing.expectEqualStrings("Hello world", (try cleanJsxText(a, "\n    Hello\n    world\n  ")).?);
    try std.testing.expectEqual(@as(?[]const u8, null), try cleanJsxText(a, "\n    \n  "));
    try std.testing.expectEqualStrings(" a b ", (try cleanJsxText(a, " a b ")).?);
    try std.testing.expectEqualStrings("a © b", try decodeEntities(a, "a &copy; b"));
    try std.testing.expectEqualStrings("<&#>", try decodeEntities(a, "&lt;&amp;&#35;&gt;"));
    try std.testing.expectEqualStrings("{x}", try decodeEntities(a, "&lbrace;x&rbrace;")); // what the `}` error suggests
}

test "parses a component" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\import { useState, View, Text } from "zant";
        \\import type { u32 } from "zant";
        \\export default function App() {
        \\  const [n, setN] = useState<u32>(0);
        \\  function inc() { setN(n + 1) }
        \\  return (
        \\    <View gap={8} style={{ padding: 4 }}>
        \\      <Text size={20}>Count: {n} {`x${n * 2}y`}</Text>
        \\      {/* comment */}
        \\      <Button onClick={() => setN(0)} disabled>Reset &amp; go</Button>
        \\    </View>
        \\  );
        \\}
    );
    try std.testing.expectEqual(@as(usize, 2), m.imports.len);
    try std.testing.expect(m.imports[1].type_only);
    try std.testing.expectEqual(@as(usize, 1), m.functions.len);
    const f = m.functions[0];
    try std.testing.expect(f.default);
    try std.testing.expectEqualStrings("App", f.func.name);
    const body = f.func.body.block;
    try std.testing.expectEqual(@as(usize, 3), body.len);
    try std.testing.expectEqual(@as(usize, 1), body[0].data.decl.init.?.data.call.type_args.len);
    const root = body[2].data.ret.?.data.jsx;
    try std.testing.expectEqualStrings("View", root.name);
    try std.testing.expectEqual(@as(usize, 2), root.attrs.len);
    try std.testing.expectEqual(@as(usize, 3), root.children.len);
    const text_el = root.children[0].element;
    try std.testing.expectEqual(@as(usize, 4), text_el.children.len);
    try std.testing.expectEqualStrings("Count: ", text_el.children[0].text.value);
    try std.testing.expectEqualStrings(" ", text_el.children[2].text.value);
    try std.testing.expectEqual(@as(usize, 2), text_el.children[3].expr.?.data.template.quasis.len);
    try std.testing.expect(root.children[1].expr == null);
    const button = root.children[2].element;
    try std.testing.expect(button.attrs[1].value == null);
    try std.testing.expectEqualStrings("Reset & go", button.children[0].text.value);
}

test "comparison is not a type argument list" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(), "function f() { return a < b; }");
    try std.testing.expectEqualStrings("<", m.functions[0].func.body.block[0].data.ret.?.data.binary.op);
}

test "syntax errors carry a location" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var p = Parser.init(arena.allocator(), "function App() { return <View></Text>; }");
    try std.testing.expectError(error.Syntax, p.parseModule());
    try std.testing.expectEqualStrings("expected `</View>` to close <View>", p.diag.?.msg);
    try std.testing.expectEqual(@as(u32, 32), p.diag.?.loc);
}

test "arrow function components" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\import { View } from "zant";
        \\const Box = () => <View />;
        \\export const Panel = (): JSX.Element => {
        \\  return <View />;
        \\};
        \\export default Box;
    );
    try std.testing.expectEqual(@as(usize, 2), m.functions.len);
    try std.testing.expectEqualStrings("Box", m.functions[0].func.name);
    try std.testing.expect(m.functions[0].default);
    const body = m.functions[0].func.body.block;
    try std.testing.expectEqual(@as(usize, 1), body.len);
    try std.testing.expect(body[0].data.ret.?.data == .jsx);
    try std.testing.expectEqualStrings("Panel", m.functions[1].func.name);
    try std.testing.expect(m.functions[1].exported and !m.functions[1].default);
}

test "text errors point at the text, not its indentation" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const src = "function A() { return <View>\n    hello\n  </View>; }";
    const m = try parseOk(arena.allocator(), src);
    const text = m.functions[0].func.body.block[0].data.ret.?.data.jsx.children[0].text;
    try std.testing.expectEqualStrings("hello", src[text.loc .. text.loc + 5]);
}

fn expectSyntaxError(src: []const u8, msg: []const u8, at: []const u8) !void {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var p = Parser.init(arena.allocator(), src);
    try std.testing.expectError(error.Syntax, p.parseModule());
    try std.testing.expectEqualStrings(msg, p.diag.?.msg);
    try std.testing.expectEqual(@as(u32, @intCast(std.mem.indexOf(u8, src, at).?)), p.diag.?.loc);
}

test "statements end at `;`, `}`, the end or a line break (tsc's ASI)" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    // no semicolons at all is fine
    const m = try parseOk(arena.allocator(), "function f() {\n  const a = 1\n  let b = a\n  b = 2\n  return b\n}");
    try std.testing.expectEqual(@as(usize, 4), m.functions[0].func.body.block.len);
    try expectSyntaxError("function f() { g() h() }", "expected `;` or a line break before `h`", "h()");
    try expectSyntaxError(
        "function f() {\n  return\n    <View />\n}",
        "a line break after `return` ends the statement; start the value on the same line (`return (`)",
        "<View",
    );
}

test "postfix ++ needs the operand on the same line" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(), "function f() {\n  a\n  ++b\n}");
    const body = m.functions[0].func.body.block;
    try std.testing.expectEqual(@as(usize, 2), body.len);
    try std.testing.expect(body[0].data.expr.data == .ident);
    try std.testing.expect(body[1].data.expr.data == .update);
}

test "`>` joins `>=` only as an operator" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(), "function f() { return a >= 10 && b > 2; }");
    const e = m.functions[0].func.body.block[0].data.ret.?.data.binary;
    try std.testing.expectEqualStrings("&&", e.op);
    try std.testing.expectEqualStrings(">=", e.lhs.data.binary.op);
    try std.testing.expectEqualStrings(">", e.rhs.data.binary.op);
    // `>` closing a tag followed by `=` text stays apart
    const t = try parseOk(arena.allocator(), "function f() { return <Text>=</Text>; }");
    try std.testing.expectEqualStrings("=", t.functions[0].func.body.block[0].data.ret.?.data.jsx.children[0].text.value);
}

test "typed arrows report errors inside their parameters" {
    try expectSyntaxError("function f() { return (v: string[]) => v; }", "array types are not supported yet", "[]");
    try expectSyntaxError("function f() { return (): number => ; }", "unexpected `;`", ";");
}

test "`>` and `}` in JSX text name the fix" {
    try expectSyntaxError("function f() { return <Text>a > b</Text>; }", "unexpected `>` in JSX text; write `{'>'}` or `&gt;`", "> b");
    try expectSyntaxError("function f() { return <Text>a } b</Text>; }", "unexpected `}` in JSX text; write `{'}'}` or `&rbrace;`", "} b");
}
