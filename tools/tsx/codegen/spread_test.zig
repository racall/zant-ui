//! JSX spread attributes (`spread.zig`): diagnostics (`~` marks the position).
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const Look = "interface Look { variant: \"primary\" | \"default\"; label?: string; }\n" ++
    "function look(): Look { return { variant: \"primary\" }; }\n";
const App = "export default function App() {\n  const l: Look = { variant: \"primary\" };\n  return ";
const End = ";\n}\n";

test "spread diagnostics" {
    const cases = [_][2][]const u8{
        .{ Look ++ App ++ "<Button {...~look()} />" ++ End, "spread a name or a member path" },
        .{ Look ++ App ++ "<Button {...~l} />" ++ End, "optional field `label` can't be spread" },
        .{ Look ++ App ++ "<Button {...~l.variant} />" ++ End, "only objects can be spread" },
        .{ Look ++ App ++ "<Button {~l} />" ++ End, "expected `...` in a spread attribute" },
        .{
            Look ++ App ++ "<List ~{...{ gap: 4 }} data={[1]} renderItem={(n) => <Text>{n}</Text>} />" ++ End,
            "<List> takes its props one by one",
        },
        .{
            "interface S { variant: \"primary\" | \"ghost\"; }\n" ++
                "export default function App() {\n  const s: S = { variant: \"ghost\" };\n" ++
                "  return <Button variant={~s.variant} />;\n}\n",
            "`variant` can be \"ghost\", which is not a `variant` value",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
