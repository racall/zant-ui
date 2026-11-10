//! `useTray` / `useHotkey` / `useClipboardChange` / `useReopen`: diagnostics.
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const Icon = "const icon: Uint8Array = [1];\n";
const App = "export default function App() {\n";
const End = "  return <View/>;\n}\n";

test "desktop hook diagnostics" {
    const cases = [_][2][]const u8{
        .{ App ++ "  useTray(~{ tooltip: \"x\" });\n" ++ End, "the tray needs an `icon`" },
        .{ Icon ++ App ++ "  useTray({ icon: icon, ~menu: [] });\n" ++ End, "unknown tray option `menu`" },
        .{
            Icon ++ App ++ "  useTray({ icon: icon, items: [{ ~label: \"x\" }] });\n" ++ End,
            "unknown tray item field `label`",
        },
        .{ App ++ "  ~useHotkey(\"cmd+k\");\n" ++ End, "useHotkey(shortcut, () => …)" },
        .{ App ++ "  useHotkey(\"cmd+k\", ~(x: number) => {});\n" ++ End, "this callback takes no parameters" },
        .{
            "function Row({ n }: { n: number }) {\n  ~useClipboardChange(() => {});\n  return <View/>;\n}\n" ++
                App ++ "  return <Row n={1}/>;\n}\n",
            "useClipboardChange belongs to the root component",
        },
        .{ App ++ "  if (true) {\n    ~useReopen(() => {});\n  }\n" ++ End, "hooks must be called at the top level" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
