//! Windows (`windows.zig`): diagnostics (`~` marks the position).
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const Panel = "function Panel() { return <View/>; }\n";
const App = "export default function App() {\n  return <View/>;\n}\n";

test "window diagnostics" {
    const cases = [_][2][]const u8{
        .{
            "function Row({ n }: { n: number }) { return <View/>; }\nconst w = createWindow(~Row, {});\n" ++ App,
            "a window's component takes no props (`Row` needs `n`)",
        },
        .{ "const w = createWindow(~Missing, {});\n" ++ App, "`Missing` is not a component of this file or imported" },
        .{ Panel ++ "const w = createWindow(Panel, { ~size: 3 });\n" ++ App, "unknown window option `size`" },
        .{ Panel ++ "const w = createWindow(Panel, { style: ~\"floating\" });\n" ++ App, "style is one of: native" },
        .{ Panel ++ "const w = ~createWindow();\n" ++ App, "createWindow takes a component and options" },
        .{
            Panel ++ "const w = createWindow(Panel, {});\n" ++
                "export default function App() {\n  return <Button onClick={() => w.place(~\"middle\")}/>;\n}\n",
            "the place is one of: center",
        },
        .{
            Panel ++ "const w = createWindow(Panel, {});\n" ++
                "export default function App() {\n  return <Button onClick={() => w.~close()}/>;\n}\n",
            "a window has show, hide, activate, toggle, visible, place and focus",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
