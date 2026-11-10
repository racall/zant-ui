//! App hooks (`app_hooks.zig`): diagnostics (`~` marks the position).
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const App = "export default function App() {\n";
const End = "  return <View/>;\n}\n";

test "app hook diagnostics" {
    const cases = [_][2][]const u8{
        .{
            "function Child() {\n  ~useCloseRequest(() => true);\n  return <View/>;\n}\n" ++ App ++ "  return <Child/>;\n}\n",
            "useCloseRequest belongs to the root component",
        },
        .{ App ++ "  ~useKeyDown(1);\n" ++ End, "useKeyDown takes one function" },
        .{ App ++ "  useCloseRequest(~(x: number) => true);\n" ++ End, "the useCloseRequest callback takes no parameters" },
        .{
            App ++ "  useQuitRequest(() => true);\n  ~useQuitRequest(() => false);\n" ++ End,
            "useQuitRequest is used twice",
        },
        .{ App ++ "  if (true) {\n    ~useKeyDown((e: KeyEvent) => false);\n  }\n" ++ End, "hooks must be called at the top level" },
        .{ App ++ "  useCloseRequest(~() => { if (true) return false; });\n" ++ End, "function must return a value on every path" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
