//! Diagnostics stop unsupported calls before invalid Zig is emitted.
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;
const App = "export default function App() { ";

test "string methods diagnose names, arity, argument types and optional receivers" {
    const cases = [_][2][]const u8{
        .{ App ++ "const x = \"a\".~normalize(); return <View/>; }", "unknown string method `normalize`; supported:" },
        .{ App ++ "const x = \"a\".~localeCompare(\"b\"); return <View/>; }", "unknown string method" },
        .{ App ++ "const x = \"a\".~normalize(); return <View/>; }", "unknown string method" },
        .{ App ++ "const x = ~\"a\".trim(1); return <View/>; }", "expects 0..0 arguments" },
        .{ App ++ "const x = ~\"a\".replace(\"b\"); return <View/>; }", "expects 2..2 arguments" },
        .{ App ++ "const x = \"a\".includes(~1); return <View/>; }", "cannot convert f64 to []const u8" },
        .{ App ++ "const x = \"a\".slice(~\"b\"); return <View/>; }", "cannot convert []const u8 to f64" },
        .{ App ++ "const x = \"a\".padStart(2, ~1); return <View/>; }", "cannot convert f64 to []const u8" },
        .{ App ++ "const x = \"a\".split(~true); return <View/>; }", "cannot convert bool to []const u8" },
        .{
            App ++ "const [s, setS] = useState<string | null>(null); const x = ~s?.trim(); return <View/>; }",
            "optional value needs ?? or !",
        },
        .{
            App ++ "const x: string = ~\"a\".at(9); return <View/>; }",
            "optional value needs a fallback with ?? or a non-null assertion !",
        },
        .{ "const s = ~\"a\".trim(); " ++ App ++ "return <Text>{s}</Text>; }", "top-level constant" },
        .{
            App ++ "const x = \"a\".replace(~/a/, \"b\"); return <View/>; }",
            "unknown string method `replace` for a regex argument; supported",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "string helper temporaries preserve source evaluation order" {
    const std = @import("std");
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compiler.compile(arena.allocator(),
        \\export default function App() {
        \\  function source(): string { return "abc"; }
        \\  function pattern(): string { return "a"; }
        \\  function replacement(): string { return "b"; }
        \\  const result = source().replace(pattern(), replacement());
        \\  return <Text>{result}</Text>;
        \\}
    , "order.tsx");
    try std.testing.expect(result == .ok);
    const code = result.ok;
    const source = std.mem.indexOf(u8, code, "= self.source();").?;
    const pattern = std.mem.indexOf(u8, code, "= self.pattern();").?;
    const replacement = std.mem.indexOf(u8, code, "= self.replacement();").?;
    try std.testing.expect(source < pattern and pattern < replacement);
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, code, "self.source()"));
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, code, "self.pattern()"));
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, code, "self.replacement()"));
}
