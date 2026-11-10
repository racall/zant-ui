//! React import compatibility uses the same validation and lowering as Zant.
const std = @import("std");
const compiler = @import("../compile.zig");
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const App = "export default function App() { return <View/>; }";

test "React import diagnostics" {
    const cases = [_][2][]const u8{
        .{ "import { ~useReducer } from \"react\"; " ++ App, "unknown import `useReducer`" },
        .{ "import { ~useReducer } from \"zant\"; " ++ App, "not supported by Zant" },
        .{ "import { useEffect as ~effect } from \"react\"; " ++ App, "`useEffect` cannot be renamed" },
        .{ "import ~\"./styles.css\"; " ++ App, "side-effect imports are not supported" },
        .{ "import ~x from \"lodash\"; " ++ App, "default imports are only supported from \"react\"" },
        .{ "import * as ~R from \"zant\"; " ++ App, "namespace imports are only supported from \"react\"" },
        .{ "import { x } from ~\"lodash\"; " ++ App, "imports come from" },
        .{
            "import React from \"react\"; const X: ~React.FC<P> = (p: P) => <View/>; " ++ App,
            "write a function component: function X(props: P)",
        },
        .{
            "import * as R from \"react\"; const X: ~R.FC = () => <View/>; " ++ App,
            "write a function component: function X(props: P)",
        },
        .{
            "import R from \"react\"; export default function App() { " ++
                "if (true) { ~R.useEffect(() => {}, []); } return <View/>; }",
            "hooks must be called at the top level of the component",
        },
        .{
            "export default function App() { ~const [n, setN] = React.useState(0); return <Text>{n}</Text>; }",
            "destructuring is only supported for useState",
        },
        .{ "export default function App() { return ~<React.Fragment/>; }", "unknown tag <React.Fragment>" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "React namespaces lower exactly like plain hooks and fragments" {
    const body =
        \\function Counter(): NS.ReactNode {
        \\  const [n, setN] = NS.useState(0);
        \\  const r = NS.useRef(0);
        \\  const doubled = NS.useMemo(() => n * 2, [n]);
        \\  NS.useEffect(() => { r.current = n; }, [n]);
        \\  return <NS.Fragment><Button onClick={() => setN(n + 1)}>{doubled}</Button></NS.Fragment>;
        \\}
        \\export default function App() { return <Counter/>; }
    ;
    for ([_][]const u8{ "import NS from \"react\";\n", "import * as NS from \"react\";\n" }) |prefix| {
        var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
        defer arena.deinit();
        const a = arena.allocator();
        const result = try compiler.compile(a, try std.mem.concat(a, u8, &.{ prefix, body }), "react.tsx");
        if (result == .err) {
            std.debug.print("unexpected diagnostic: {s}\n", .{result.err.msg});
            return error.UnexpectedDiagnostic;
        }
        try std.testing.expect(std.mem.indexOf(u8, result.ok, "_Local = struct") != null);
        try std.testing.expect(std.mem.indexOf(u8, result.ok, "NS") == null);
        try std.testing.expect(std.mem.indexOf(u8, result.ok, "Fragment") == null);
    }
}

test "named React imports share Zant validation and output" {
    const names = "{ useState, useEffect, useMemo, useRef, type ReactNode, ReactElement, Text, u32 }";
    const body = "export default function App(): ReactNode { const [n, setN] = useState<u32>(0); " ++
        "return <Text>{n}</Text>; }";
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const react = try compiler.compile(a, try std.fmt.allocPrint(a, "import {s} from \"react\"; {s}", .{
        names, body,
    }), "same.tsx");
    const zant = try compiler.compile(a, try std.fmt.allocPrint(a, "import {s} from \"zant\"; {s}", .{
        names, body,
    }), "same.tsx");
    try std.testing.expect(react == .ok and zant == .ok);
    try std.testing.expectEqualStrings(zant.ok, react.ok);
}

test "root shorthand fragments and React fragments emit the same native root" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const plain = try compiler.compile(
        a,
        "export default function App() { return <><Text>a</Text><Text>b</Text></>; }",
        "root.tsx",
    );
    const react = try compiler.compile(a, "import R from \"react\"; export default function App() { " ++
        "return <R.Fragment><Text>a</Text><Text>b</Text></R.Fragment>; }", "root.tsx");
    try std.testing.expect(plain == .ok and react == .ok);
    try std.testing.expectEqualStrings(plain.ok, react.ok);
}
