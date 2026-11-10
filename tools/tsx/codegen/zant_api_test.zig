//! Desktop functions from "zant" (`clipboard.*`, `app.*`, `mainWindow.*`, …): diagnostics.
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const Import = "import { clipboard, app, mainWindow, keyLabel, wakeAfter } from \"zant\";\n";
const App = "export default function App() {\n";
const End = "  return <View/>;\n}\n";

test "desktop function diagnostics" {
    const cases = [_][2][]const u8{
        .{ Import ++ App ++ "  const p = clipboard.~paste();\n" ++ End, "unknown function `clipboard.paste`" },
        .{
            Import ++ App ++ "  const on = ~app.setLaunchAtLogin(true);\n" ++ End,
            "`setLaunchAtLogin` changes the desktop",
        },
        .{ Import ++ App ++ "  const c = ~clipboard;\n" ++ End, "`clipboard` is a namespace" },
        .{ Import ++ App ++ "  const t = ~clipboard.text(1);\n" ++ End, "`text` takes 0 argument(s), got 1" },
        .{
            Import ++ App ++ "  const k = \"f\";\n  const l = ~keyLabel(k);\n" ++ End,
            "keyLabel takes a string literal",
        },
        .{ Import ++ "const t = ~clipboard.text();\n" ++ App ++ End, "cannot run in a top-level constant" },
        .{ Import ++ App ++ "  const go = () => mainWindow.focus(~1);\n" ++ End, "" },
        // not imported: an ordinary unknown name
        .{ App ++ "  const go = () => ~clipboard.text();\n" ++ End, "unknown name `clipboard`" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "default value diagnostics" {
    const P = "interface P { size?: number; label: string }\n";
    const cases = [_][2][]const u8{
        .{
            P ++ "function B({ size = 1, label = ~\"x\" }: P) { return <View/>; }\n" ++
                "export default function App() { return <B label=\"a\"/>; }\n",
            "`label` is a required prop",
        },
        .{ "function f(n?: number = ~1): number { return n; }\n" ++ App ++ End, "already optional" },
        .{ "function f(n = ~null): number { return 1; }\n" ++ App ++ End, "needs a type for this default" },
        .{ "function f(a: number, b = 2): number { return a + b; }\n" ++ App ++ "  const x = ~f();\n" ++ End, "takes 2 arguments" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "timer diagnostics" {
    const cases = [_][2][]const u8{
        .{ App ++ "  const t = ~setTimeout(() => {}, 10);\n" ++ End, "setTimeout runs in event handlers and effects" },
        .{
            "function Row({ n }: { n: number }) {\n  return <Button onClick={() => ~setTimeout(() => {}, 5)}/>;\n}\n" ++
                App ++ "  return <Row n={1}/>;\n}\n",
            "timers belong to the root component",
        },
        .{
            App ++ "  const go = () => {\n    const s = \"x\";\n    setTimeout(~() => console.log(s), 5);\n  };\n" ++
                "  return <Button onClick={go}/>;\n}\n",
            "a timer callback can capture numbers, booleans and enums",
        },
        .{ App ++ "  const go = () => setTimeout(~(n: number) => {}, 5);\n" ++ End, "takes no parameters" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
