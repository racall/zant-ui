//! Plain top-level functions (`functions.zig`): diagnostics (`~` marks the
//! position) and cross-file use.
const std = @import("std");
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;
const compile = @import("../compile.zig");

const App = "export default function App() {\n  return <View/>;\n}\n";

test "function diagnostics" {
    const cases = [_][2][]const u8{
        .{ "function twice(~n): number { return n * 2; }\n" ++ App, "parameter `n` of `twice` needs a type" },
        .{ "~function row(): JSX.Element { return <View/>; }\n" ++ App, "components start with a capital letter" },
        .{ "function f(): number { return 1; }\n~function f(): number { return 2; }\n" ++ App, "duplicate function `f`" },
        .{
            "~function count(): number { if (true) return 1; }\n" ++ App,
            "`count` must return a value on every path",
        },
        .{
            "function f(): number { return 1; }\n" ++
                "export default function App() {\n  return <Text>{~f(1)}</Text>;\n}\n",
            "`f` takes 0 arguments",
        },
        .{
            "function f(): void { const x = ~useState(0); }\n" ++ App,
            "hooks must be called at the top level of the component",
        },
        .{
            "function todoRow(): number { return 1; }\n~function TodoRow() { return <View/>; }\n" ++ App,
            "<TodoRow> compiles to the function `todoRow`",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "functions are shared between files" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compileProject(arena.allocator(), &.{
        .{ .path = "src/format.tsx", .src =
        \\export interface Size { bytes: number; }
        \\export function kb(s: Size): string { return `${s.bytes / 1024} KB`; }
        },
        .{ .path = "src/App.tsx", .src =
        \\import { kb as kilobytes, type Size } from "./format";
        \\export default function App() {
        \\  const s: Size = { bytes: 2048 };
        \\  return <Button onClick={() => console.log(kilobytes(s))}>{kilobytes(s)}</Button>;
        \\}
        },
    });
    if (result == .err) std.debug.print("{s}\n", .{result.err.diag.msg});
    try std.testing.expect(result == .ok);
    try std.testing.expect(std.mem.indexOf(u8, result.ok[0], "pub fn kb(s: Size) []const u8") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok[1], "_format.kb(s)") != null);
}
