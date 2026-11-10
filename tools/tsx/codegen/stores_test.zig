//! Diagnostics of `createStore` / `useStore` (`~` marks the position).
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const Decl = "interface S { n: number; }\nconst s = createStore<S>({ n: 0 });\n";
const App = "export default function App() {\n";

test "store diagnostics" {
    const cases = [_][2][]const u8{
        .{
            "const s = ~createStore({ n: 0 });\n" ++ App ++ "  return <View/>;\n}",
            "createStore takes the state type and its initial value",
        },
        .{ Decl ++ App ++ "  const v = useStore(~42);\n  return <View/>;\n}", "useStore needs a store" },
        .{
            Decl ++ App ++ "  ~s.set({ n: 1 });\n  return <View/>;\n}",
            "stores change in event handlers and effects",
        },
        .{
            Decl ++ App ++ "  return <Button onClick={() => s.~reset()}/>;\n}",
            "a store has get(), set(value) and update",
        },
        .{
            Decl ++ App ++ "  return <Button onClick={() => s.update({ ~m: 1 })}/>;\n}",
            "has no field `m`",
        },
        .{
            Decl ++ App ++ "  return <Button onClick={() => s.update({ ...~s.get() })}/>;\n}",
            "list the fields to change",
        },
        .{
            Decl ++ App ++ "  if (true) {\n    ~const v = useStore(s);\n  }\n  return <View/>;\n}",
            "hooks must be called at the top level",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
