//! Diagnostics for imperative statements and their lexical boundaries.
const std = @import("std");
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;
const compile = @import("../compile.zig").compile;

const App = "export default function App() { ";
const Handler = App ++ "return <Button onClick={() => { ";
const End = " }}/>; }";

test "loop and switch diagnostics point at the offending statement or value" {
    const cases = [_][2][]const u8{
        .{ App ++ "~for (;;) {} return <View/>; }", "loops run in handlers and functions; use `.map` / `.filter`" },
        .{ App ++ "if (true) { ~while (false) {} } return <View/>; }", "loops run in handlers and functions" },
        .{ Handler ++ "~break;" ++ End, "break is only allowed inside a loop or switch" },
        .{ Handler ++ "~continue;" ++ End, "continue is only allowed inside a loop" },
        .{ Handler ++ "switch (1) { case 1: ~continue; }" ++ End, "continue is only allowed inside a loop" },
        .{
            Handler ++ "switch (1) { ~case 1: console.log(1); case 2: break; }" ++ End,
            "add `break`; falling through to the next case is not supported",
        },
        .{ Handler ++ "for (const x of ~3) {}" ++ End, "for...of requires an array" },
        .{ Handler ++ "for (const x of [1]) ~x++;" ++ End, "`x` is const" },
        .{ Handler ++ "switch (~[1]) {}" ++ End, "switch requires a number, string, boolean or string-literal union" },
        .{ Handler ++ "const v: {n: number} = {n: 1}; switch (~v) {}" ++ End, "switch requires a number" },
        .{ Handler ++ "for (let i=0; i<1; i++) {} console.log(~i);" ++ End, "unknown name `i`" },
        .{ Handler ++ "for (const x of [1]) {} console.log(~x);" ++ End, "unknown name `x`" },
        .{ Handler ++ "while (~1) {}" ++ End, "conditions must be boolean" },
        .{
            Handler ++ "while (true) { const a = [1].filter(x => { if (x > 0) ~break; return true; }); break; }" ++ End,
            "break is only allowed inside a loop or switch",
        },
        .{
            Handler ++ "while (true) { const a = [1].filter(x => { " ++
                "if (x > 0) ~continue; return true; }); break; }" ++ End,
            "continue is only allowed inside a loop",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "loop bindings use assigned tracking and do not leak into the enclosing scope" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile(arena.allocator(), Handler ++
        "let total=0; for (let x of [1]) { x++; total+=x; } " ++
        "for (let y of [2]) total+=y; for (let i=0; i<3; i++) total+=i; " ++
        "const i=8; console.log(total, i);" ++ End, "bindings.tsx");
    if (result == .err) {
        std.debug.print("{s}\n", .{result.err.msg});
        return error.CompilationFailed;
    }
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "var x: f64") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "const y: f64") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "var i: f64") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "const i_2: f64") != null);
}

test "returns in post-test and unconditional loops and exhaustive switches satisfy function returns" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile(arena.allocator(),
        \\export default function App() {
        \\  function a(): number { do { return 1; } while (false); }
        \\  function b(): number { for (;;) { return 2; } }
        \\  function c(): number { while (true) return 3; }
        \\  function d(x: number): number {
        \\    switch (x) { case 1: case 2: return 4; default: return 5; }
        \\  }
        \\  const values = [1].map(x => {
        \\    switch (x) { case 1: for (;;) return x; default: return 0; }
        \\  });
        \\  return <Text>{a() + b() + c() + d(values[0])}</Text>;
        \\}
    , "returns.tsx");
    if (result == .err) {
        std.debug.print("{s}\n", .{result.err.msg});
        return error.CompilationFailed;
    }
}

test "unreachable returns after break do not make a switch returning" {
    try expectDiagnostic(
        App ++
            "~function f(x: number): number { switch (x) { case 1: break; return 1; default: return 2; } } " ++
            "return <View/>; }",
        "function must return a value on every path",
    );
}

test "conditional jumps do not hide missing function returns" {
    try expectDiagnostic(
        App ++
            "~function f(x: number): number { switch (x) { " ++
            "case 1: if (x > 0) break; return 1; default: return 2; } } return <View/>; }",
        "function must return a value on every path",
    );
    try expectDiagnostic(
        App ++
            "~function f(x: boolean): number { while (true) { if (x) break; return 1; } } return <View/>; }",
        "function must return a value on every path",
    );
    try expectDiagnostic(
        App ++
            "~function f(x: boolean): number { do { if (x) continue; return 1; } while (false); } return <View/>; }",
        "function must return a value on every path",
    );
}

test "string contents do not introduce unused control labels" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile(arena.allocator(), Handler ++
        "while (false) console.log(':_loop;'); switch (1) { case 1: console.log(':_switch;'); }" ++ End, "labels.tsx");
    try std.testing.expect(result == .ok);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "_loop: while") == null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "_switch: {") == null);
}
