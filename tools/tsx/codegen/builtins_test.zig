//! Built-in diagnostics and lowering invariants (`~` marks the error location).
const std = @import("std");
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;
const App = "export default function App() { ";

test "built-ins diagnose unsupported members, arguments and Date objects" {
    const cases = [_][2][]const u8{
        .{ App ++ "return <Text>{Math.~foo(1)}</Text>; }", "unknown Math member `foo`; supported: floor, ceil" },
        .{ App ++ "return <Text>{Math.~foo}</Text>; }", "supported: floor, ceil" },
        .{ App ++ "return <Text>{Number.~parseSomething(\"1\")}</Text>; }", "supported: isNaN, isFinite, isInteger" },
        .{ App ++ "return <Text>{Number.~MIN_VALUE}</Text>; }", "MAX_SAFE_INTEGER, EPSILON" },
        .{ App ++ "return <Text>{Date.~UTC()}</Text>; }", "full Date objects are out of scope" },
        .{ App ++ "return <Text>{~Date()}</Text>; }", "full Date objects are out of scope; use Date.now()" },
        .{ App ++ "return <Text>{performance.~foo()}</Text>; }", "supported: now" },
        .{ App ++ "return <Text>{~Math.floor()}</Text>; }", "expects 1..1 arguments" },
        .{ App ++ "return <Text>{~Math.pow(2)}</Text>; }", "expects 2..2 arguments" },
        .{ App ++ "return <Text>{~Date.now(1)}</Text>; }", "expects 0..0 arguments" },
        .{ App ++ "return <Text>{~Math.random(1)}</Text>; }", "expects 0..0 arguments" },
        .{ App ++ "return <Text>{Math.abs(~\"1\")}</Text>; }", "requires a number" },
        .{ App ++ "return <Text>{Math.min(1, ~true)}</Text>; }", "requires a number" },
        .{ App ++ "return <Text>{~Number()}</Text>; }", "expects 1..1 arguments" },
        .{ App ++ "return <Text>{Number(~null)}</Text>; }", "conversion requires a number, string, or boolean" },
        .{ App ++ "return <Text>{parseInt(~true)}</Text>; }", "cannot convert bool to []const u8" },
        .{ App ++ "return <Text>{parseInt(\"1\", ~\"2\")}</Text>; }", "cannot convert []const u8 to f64" },
        .{ App ++ "return <Text>{(~1).toFixed(1, 2)}</Text>; }", "expects 0..1 arguments" },
        .{ App ++ "return <Text>{(1).toFixed(~\"2\")}</Text>; }", "cannot convert []const u8 to f64" },
        .{ App ++ "return <Text>{(~1).toString(16)}</Text>; }", "expects 0..0 arguments" },
        .{
            App ++ "const [n, setN] = useState<number | null>(null); return <Text>{~n?.toFixed(2)}</Text>; }",
            "optional value needs ?? or !",
        },
        .{ "const time = ~Date.now(); " ++ App ++ "return <Text>{time}</Text>; }", "top-level constant" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "built-in arguments are sequenced once and numeric methods evaluate the receiver first" {
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compiler.compile(arena.allocator(),
        \\function source(): number { return 1.25; }
        \\function digits(): number { return 2; }
        \\export default function App() {
        \\  const fixed = source().toFixed(digits());
        \\  const minimum = Math.min(source(), digits());
        \\  const Math: { PI: number } = { PI: 7 };
        \\  return <Text>{fixed}/{minimum}/{Math.PI}</Text>;
        \\}
    , "order.tsx");
    try std.testing.expect(result == .ok);
    const code = result.ok;
    const source = std.mem.indexOf(u8, code, "= source();").?;
    const digits = std.mem.indexOf(u8, code, "= digits();").?;
    try std.testing.expect(source < digits);
    try std.testing.expectEqual(@as(usize, 2), std.mem.count(u8, code, "= source();"));
    try std.testing.expectEqual(@as(usize, 2), std.mem.count(u8, code, "= digits();"));
    try std.testing.expect(std.mem.indexOf(u8, code, "Math.PI") != null);
}
