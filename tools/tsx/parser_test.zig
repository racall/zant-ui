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
    try expectSyntaxError(
        "function f() { return (v: Record<string, number>) => v; }",
        "generic type `Record<…>` is not supported; only `Array<T>` is",
        "Record",
    );
    try expectSyntaxError("function f() { return (): number => ; }", "unexpected `;`", ";");
}

test "`>` and `}` in JSX text name the fix" {
    try expectSyntaxError("function f() { return <Text>a > b</Text>; }", "unexpected `>` in JSX text; write `{'>'}` or `&gt;`", "> b");
    try expectSyntaxError("function f() { return <Text>a } b</Text>; }", "unexpected `}` in JSX text; write `{'}'}` or `&rbrace;`", "} b");
}

test "phase 2 types: interfaces, unions, arrays, function types" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\export interface Todo {
        \\  id: number;
        \\  title: string
        \\  done?: boolean,
        \\  tags: string[]
        \\}
        \\type Filter = "all" | "active" | "done";
        \\type Rows = Array<Array<Todo>>;
        \\interface RowProps { todo: Todo; onToggle: (id: number) => void; note: string | null }
    );
    try std.testing.expectEqual(@as(usize, 4), m.types.len);
    const todo = m.types[0];
    try std.testing.expect(todo.exported);
    const fields = todo.ty.detail.?.object;
    try std.testing.expectEqual(@as(usize, 4), fields.len);
    try std.testing.expect(fields[2].optional);
    try std.testing.expectEqualStrings("string", fields[3].ty.detail.?.array.name);
    const filter = m.types[1].ty.detail.?.union_;
    try std.testing.expectEqual(@as(usize, 3), filter.len);
    try std.testing.expectEqualStrings("active", filter[1].detail.?.string_lit);
    // nested generics close with two separate `>` tokens
    try std.testing.expectEqualStrings("Todo", m.types[2].ty.detail.?.array.detail.?.array.name);
    const row = m.types[3].ty.detail.?.object;
    const on_toggle = row[1].ty.detail.?.func;
    try std.testing.expectEqualStrings("id", on_toggle.params[0].name.name);
    try std.testing.expectEqualStrings("void", on_toggle.ret.name);
    try std.testing.expect(row[2].ty.detail.?.union_[1].detail.?.* == .null_);
}

test "phase 2 expressions: literals with spread, indexing, ?., !, as" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\const empty: Todo[] = [];
        \\function f() {
        \\  const [todos, setTodos] = useState<Todo[]>([])
        \\  const next = [...todos, { id: 1, ...base, done: false }]
        \\  const first = todos[0]?.title ?? "none"
        \\  const found = todos.find((t: Todo): boolean => t.done)!
        \\  const n = count as u32
        \\  return todos.map((t, i) => <Text key={t.id}>{i}</Text>)
        \\}
    );
    try std.testing.expectEqual(@as(usize, 1), m.consts.len);
    try std.testing.expectEqual(@as(usize, 0), m.consts[0].value.data.array.len);
    const body = m.functions[0].func.body.block;
    try std.testing.expectEqualStrings("Todo", body[0].data.decl.init.?.data.call.type_args[0].detail.?.array.name);
    const next = body[1].data.decl.init.?.data.array;
    try std.testing.expect(next[0].data == .spread);
    const obj = next[1].data.object;
    try std.testing.expect(obj[1].spread and !obj[0].spread);
    const coalesce = body[2].data.decl.init.?.data.binary;
    try std.testing.expectEqualStrings("??", coalesce.op);
    try std.testing.expect(coalesce.lhs.data.member.optional);
    try std.testing.expect(coalesce.lhs.data.member.object.data == .index);
    try std.testing.expect(body[3].data.decl.init.?.data == .non_null);
    try std.testing.expectEqualStrings("u32", body[4].data.decl.init.?.data.as.ty.name);
    const map_arrow = body[5].data.ret.?.data.call.args[0].data.arrow;
    try std.testing.expectEqual(@as(usize, 2), map_arrow.params.len);
}

test "destructured props and optional parameters" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\function Row({ todo, onToggle }: RowProps) { return <View />; }
        \\function g(a: number, b?: string) {}
    );
    const p = m.functions[0].func.params[0];
    try std.testing.expectEqualStrings("props", p.name.name);
    try std.testing.expectEqual(@as(usize, 2), p.destructure.len);
    try std.testing.expectEqualStrings("onToggle", p.destructure[1].name);
    try std.testing.expectEqualStrings("RowProps", p.ty.?.name);
    try std.testing.expect(m.functions[1].func.params[1].optional);
    try expectSyntaxError("function Row({ todo: t }: P) {}", "renaming destructured props is not supported; read `props.todo` instead", ": t");
}

test "a parenthesized expression is not taken for an arrow" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(), "function f() { return x ? (a) : b; }");
    const c = m.functions[0].func.body.block[0].data.ret.?.data.cond;
    try std.testing.expect(c.then.data == .ident);
}

test "loop headers, single statements and switch groups" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\function f() {
        \\  for (let i: number = 0; i < 4; i++) total += i;
        \\  for (i = 0; ; --i) { break; }
        \\  for (;;) break;
        \\  for (const x of xs) { continue; }
        \\  for (let y of [1, 2]) y++;
        \\  while (ok) count++;
        \\  do { count--; } while (ok)
        \\  next()
        \\  switch (value) { case 1: break; case 2: case 3: return 4; default: return 0; }
        \\}
    );
    const ss = m.functions[0].func.body.block;
    try std.testing.expectEqual(@as(usize, 9), ss.len);
    const classic = ss[0].data.for_;
    try std.testing.expectEqualStrings("i", classic.init.?.data.decl.pattern.ident.name);
    try std.testing.expectEqualStrings("number", classic.init.?.data.decl.ty.?.name);
    try std.testing.expect(classic.cond.?.data == .binary);
    try std.testing.expect(classic.update.?.data == .update);
    try std.testing.expect(classic.body.data == .expr);
    try std.testing.expect(ss[1].data.for_.cond == null);
    try std.testing.expect(ss[1].data.for_.init.?.data == .expr);
    try std.testing.expect(ss[2].data.for_.init == null and ss[2].data.for_.update == null);
    try std.testing.expect(ss[3].data.for_of.is_const);
    try std.testing.expect(!ss[4].data.for_of.is_const);
    try std.testing.expect(!ss[5].data.while_.do_first);
    try std.testing.expect(ss[6].data.while_.do_first);
    const cases = ss[8].data.switch_.cases;
    try std.testing.expectEqual(@as(usize, 4), cases.len);
    try std.testing.expect(cases[0].body[0].data == .break_);
    try std.testing.expectEqual(@as(usize, 0), cases[1].body.len);
    try std.testing.expect(cases[3].value == null);
}

test "loop syntax diagnostics and jump ASI" {
    try expectSyntaxError("function f() { while (true {} }", "expected `)`, found `{`", "{}");
    try expectSyntaxError("function f() { for (let i=0; i<3; i++ {} }", "expected `)`, found `{`", "{}");
    try expectSyntaxError("function f() { do count++; while (true }", "expected `)`, found `}`", "}");
    try expectSyntaxError("function f() { break label; }", "labeled break and continue are not supported", "label");
    try expectSyntaxError("function f() { continue label; }", "labeled break and continue are not supported", "label");
    try expectSyntaxError("function f() { for (const i=0;;) {} }", "C-style for initializer must use let", "const");
    try expectSyntaxError(
        "function f() { for (;; step()) {} }",
        "for update must be an assignment, ++ or --",
        "step()",
    );
    try expectSyntaxError(
        "function f() { switch (x) { default: default: } }",
        "switch has more than one default",
        "default: }",
    );
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(), "function f() { break\nnext(); continue\nother(); }");
    const ss = m.functions[0].func.body.block;
    try std.testing.expectEqual(@as(usize, 4), ss.len);
    try std.testing.expect(ss[0].data == .break_ and ss[2].data == .continue_);
}

test "React default, namespace and mixed imports retain their bindings" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\import R from "react";
        \\import * as NS from "react";
        \\import React, { useState, type ReactNode } from "react";
        \\import { useState, useEffect } from "react";
        \\import Default, * as Space from "react";
        \\function App() { return <React.Fragment><Text/></React.Fragment>; }
    );
    try std.testing.expectEqualStrings("R", m.imports[0].default_name.?.name);
    try std.testing.expectEqualStrings("NS", m.imports[1].namespace_name.?.name);
    try std.testing.expectEqualStrings("React", m.imports[2].default_name.?.name);
    try std.testing.expectEqual(@as(usize, 2), m.imports[2].names.len);
    try std.testing.expect(m.imports[2].names[1].type_only);
    try std.testing.expectEqual(@as(usize, 2), m.imports[3].names.len);
    try std.testing.expectEqualStrings("Default", m.imports[4].default_name.?.name);
    try std.testing.expectEqualStrings("Space", m.imports[4].namespace_name.?.name);
    try std.testing.expectEqualStrings("", m.functions[0].func.body.block[0].data.ret.?.data.jsx.name);
}

test "React compatibility respects shadowed namespace bindings" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const m = try parseOk(arena.allocator(),
        \\import R from "react";
        \\function f(R: number) { return R.useState(0); }
        \\function g() { const R = 0; return R.useState(0); }
        \\function h() { return R.useState(0); }
    );
    try std.testing.expect(m.functions[0].func.body.block[0].data.ret.?.data.call.callee.data == .member);
    try std.testing.expect(m.functions[1].func.body.block[1].data.ret.?.data.call.callee.data == .member);
    try std.testing.expect(m.functions[2].func.body.block[0].data.ret.?.data.call.callee.data == .ident);
}
