//! Diagnostics of `useRef`, `useMemo` and `useEffect` (`~` marks the position).
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

const App = "export default function App() {\n";

test "hook diagnostics" {
    const cases = [_][2][]const u8{
        .{
            App ++ "  const r = useRef(0);\n  return <Text>{~r}</Text>;\n}",
            "read and write a ref through `r.current`",
        },
        .{ App ++ "  const r = useRef(0);\n  return <Text>{r.~value}</Text>;\n}", "a ref only has `.current`" },
        .{
            App ++ "  const r = useRef(0);\n  ~r.current = 1;\n  return <View/>;\n}",
            "refs are written in event handlers and effects",
        },
        .{ App ++ "  ~let r = useRef(0);\n  return <View/>;\n}", "declare useRef with const" },
        .{
            App ++ "  ~const v = useMemo((x: number) => x, []);\n  return <View/>;\n}",
            "useMemo takes a function without parameters and a dependency list",
        },
        .{
            App ++ "  const [n, setN] = useState(0);\n  const v = useMemo(() => n, ~n);\n  return <View/>;\n}",
            "the dependency list must be an array literal",
        },
        .{
            App ++ "  const v = useMemo(() => 1, [~<View/>]);\n  return <View/>;\n}",
            "dependencies are values",
        },
        .{
            App ++ "  ~const v = useMemo(() => <View/>, []);\n  return <View/>;\n}",
            "useMemo caches values; elements are rebuilt every frame",
        },
        .{
            "function Row({ name }: { name: string }) {\n  useEffect(() => {\n    return () => console.log(~name);\n" ++
                "  }, []);\n  return <View/>;\n}\n" ++ App ++ "  return <Row name=\"a\"/>;\n}",
            "a cleanup runs after the props are gone",
        },
        .{
            App ++ "  useEffect(() => {\n    return ~(x: number) => {};\n  }, []);\n  return <View/>;\n}",
            "the cleanup function takes no parameters",
        },
        .{
            App ++ "  if (true) {\n    ~useEffect(() => {}, []);\n  }\n  return <View/>;\n}",
            "hooks must be called at the top level of the component",
        },
        .{ App ++ "  ~useRef(0);\n  return <View/>;\n}", "keep the value of this hook" },
        .{ App ++ "  return <Text>{~useMemo(() => 1, [])}</Text>;\n}", "hooks must be called at the top level" },
        .{
            "import { useRef as ~useR } from \"zant\";\n" ++ App ++ "  return <View/>;\n}",
            "`useRef` cannot be renamed",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}
