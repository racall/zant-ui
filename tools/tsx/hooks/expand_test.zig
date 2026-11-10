//! Custom hooks inlined into components (`expand.zig`).
const std = @import("std");
const compile = @import("../compile.zig");

fn has(haystack: []const u8, needle: []const u8) !void {
    if (std.mem.indexOf(u8, haystack, needle) == null) {
        std.debug.print("missing `{s}` in:\n{s}\n", .{ needle, haystack });
        return error.TestUnexpectedResult;
    }
}

fn hasNot(haystack: []const u8, needle: []const u8) !void {
    if (std.mem.indexOf(u8, haystack, needle) != null) {
        std.debug.print("unexpected `{s}` in:\n{s}\n", .{ needle, haystack });
        return error.TestUnexpectedResult;
    }
}

test "a hook in the same file becomes the component's state and effects" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const src =
        \\import { useState, useEffect } from "zant";
        \\function useCounter(start: number) {
        \\  const [count, setCount] = useState(start);
        \\  useEffect(() => { setCount(start); }, []);
        \\  return { count, increment: () => setCount(count + 1) };
        \\}
        \\export default function App() {
        \\  const { count, increment } = useCounter(5);
        \\  const other = useCounter(1);
        \\  return <Button onClick={increment}>{`${count} ${other.count}`}</Button>;
        \\}
    ;
    const result = try compile.compile(arena.allocator(), src, "App.tsx");
    if (result == .err) std.debug.print("{s}\n", .{result.err.msg});
    const code = result.ok;
    try has(code, "useCounter_count: f64 = 5");
    try has(code, "useCounter2_count: f64 = 1");
    try hasNot(code, "fn useCounter(");
}

test "a hook in another file brings the imports it reads" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compileProject(arena.allocator(), &.{
        .{ .path = "src/Store.tsx", .src =
        \\import { createStore } from "zant";
        \\export interface Clicks { clicks: number }
        \\export const store = createStore<Clicks>({ clicks: 0 });
        \\export function label(n: number): string { return `${n} clicks`; }
        },
        .{ .path = "src/hooks/useClicks.tsx", .src =
        \\import { useStore } from "zant";
        \\import { store, label as describe } from "../Store";
        \\import dot from "../../assets/dot.png";
        \\const step = 1;
        \\export function useClicks() {
        \\  const s = useStore(store);
        \\  const add = () => store.update({ clicks: s.clicks + step });
        \\  return [describe(s.clicks), add, dot.length];
        \\}
        },
        .{ .path = "src/App.tsx", .src =
        \\import { useClicks } from "./hooks/useClicks";
        \\export default function App() {
        \\  const [text, add, size] = useClicks();
        \\  return <Button onClick={add}>{`${text} ${size}`}</Button>;
        \\}
        },
    });
    if (result == .err) std.debug.print("{d}: {s}\n", .{ result.err.file, result.err.diag.msg });
    const app = result.ok[2];
    try has(app, "@import(\"Store.zig\")");
    try has(app, "@import(\"useClicks.zig\")");
    try has(app, "_Store.label(");
    try has(app, "_useClicks.step");
    // the hook file has no hook left, but keeps what the app imports from it
    try hasNot(result.ok[1], "useClicks(");
    try has(result.ok[1], "pub const step");
}

test "hooks call hooks and errors point into the hook's file" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const hook =
        \\import { useState } from "zant";
        \\export function useFlag() { const [on, setOn] = useState(false); return [on, setOn]; }
        \\export function useToggle() {
        \\  const [on, setOn] = useFlag();
        \\  return { on, toggle: () => setOn(!on) };
        \\}
    ;
    const ok = try compile.compileProject(a, &.{
        .{ .path = "src/useToggle.tsx", .src = hook },
        .{ .path = "src/App.tsx", .src =
        \\import { useToggle } from "./useToggle";
        \\export default function App() {
        \\  const t = useToggle();
        \\  return <Switch checked={t.on} onChange={t.toggle} />;
        \\}
        },
    });
    if (ok == .err) std.debug.print("{s}\n", .{ok.err.diag.msg});
    try has(ok.ok[1], "useFlag_on: bool = false");

    const broken = try compile.compileProject(a, &.{
        .{ .path = "src/useBroken.tsx", .src = "export function useBroken() { return missing + 1; }" },
        .{ .path = "src/App.tsx", .src =
        \\import { useBroken } from "./useBroken";
        \\export default function App() { const n = useBroken(); return <Text>{n}</Text>; }
        },
    });
    try std.testing.expectEqual(@as(usize, 0), broken.err.file);
    try std.testing.expectEqual(@as(u32, 37), broken.err.diag.loc); // `missing`
}

fn expectError(src: []const u8, message: []const u8) !void {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compile(arena.allocator(), src, "App.tsx");
    if (result == .ok) {
        std.debug.print("expected `{s}`, compiled:\n{s}\n", .{ message, result.ok });
        return error.TestUnexpectedResult;
    }
    try has(result.err.msg, message);
}

test "custom hook errors" {
    const hook = "import { useState } from \"zant\";\n" ++
        "function useN(n: number) { const [v, setV] = useState(n); return v; }\n";
    const app = "export default function App() {\n";
    try expectError(hook ++ app ++ "if (true) { useN(1); }\nreturn <View/>; }", "call it at the top level");
    try expectError(hook ++ app ++ "const f = () => useN(1);\nreturn <View/>; }", "call it at the top level");
    try expectError(hook ++ app ++ "const v = useN();\nreturn <Text>{v}</Text>; }", "`useN` takes 1 arguments");
    try expectError(hook ++ app ++ "const [a, b] = useN(1);\nreturn <Text>{a}</Text>; }", "an array literal");
    try expectError(hook ++ app ++ "const { a } = useN(1);\nreturn <Text>{a}</Text>; }", "an object literal");
    try expectError(
        "function useE(on: boolean) { if (on) { return 1; } return 2; }\n" ++ app ++
            "const v = useE(true);\nreturn <Text>{v}</Text>; }",
        "returns once, at its end",
    );
    try expectError(
        "const limit = 3;\nfunction useL() { return limit; }\n" ++ app ++
            "const limit = 4;\nconst v = useL();\nreturn <Text>{v}</Text>; }",
        "`useL` reads `limit`, which App also declares",
    );
    try expectError(
        "function useO() { return { a: 1 }; }\n" ++ app ++ "const o = useO();\nreturn <Text>{o}</Text>; }",
        "is the object a hook returned",
    );
}
