//! The nesting rules (`nesting.zig`): what may go inside what, with the
//! position of the offending element (`~`).
const std = @import("std");
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;
const compile = @import("../compile.zig");

const App = "export default function App() {\n  const items = [1, 2];\n  return (";
const End = ");\n}";
const Counter = "function Counter() { const [n, setN] = useState(0); return <Text>{n}</Text>; }\n";

test "misplaced elements and children" {
    const cases = [_][2][]const u8{
        .{
            App ++ "<Text>a ~<View /></Text>" ++ End,
            "<View> can't be inside <Text>; it holds text, {values}, <Text> spans",
        },
        .{ App ++ "<Button>~<View /></Button>" ++ End, "<View> can't be inside <Button>; it holds its label" },
        .{
            App ++ "<Button><Icon name=\"check\" />~<Icon name=\"plus\" /> a</Button>" ++ End,
            "a <Button> has one icon",
        },
        .{ App ++ "<Button>~<Icon /> a</Button>" ++ End, "<Icon> needs `name`" },
        .{
            App ++ "<Checkbox>~<Text>a</Text></Checkbox>" ++ End,
            "<Text> can't be inside <Checkbox>; it holds its label",
        },
        .{ App ++ "<Heading level={2}>a ~<Text>b</Text></Heading>" ++ End, "use <Text> for spans" },
        .{ App ++ "<Input>~<Text>x</Text></Input>" ++ End, "<Input> has no children" },
        .{ App ++ "<Text ~maxLines={2}>a <Text>b</Text></Text>" ++ End, "`maxLines` can't be used on text with spans" },
        .{ App ++ "<Text>a <Link ~href=\"x\">b</Link></Text>" ++ End, "an inline <Link> takes onClick only" },
        .{ App ++ "<Text>a <Text ~textAlign=\"center\">b</Text></Text>" ++ End, "can't be used on text with spans" },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "keys, scrolling and stateful components in lists" {
    const cases = [_][2][]const u8{
        .{
            App ++ "<View><Text key=\"a\">1</Text><Text key=~\"a\">2</Text></View>" ++ End,
            "two siblings have the key",
        },
        .{
            App ++ "<View scroll>~<List data={items} renderItem={(n) => <Text>{n}</Text>} /></View>" ++ End,
            "<List> scrolls by itself",
        },
        .{
            App ++ "<View style={{ overflow: \"scroll\" }}><View>~<List data={items} " ++
                "renderItem={(n) => <Text>{n}</Text>} /></View></View>" ++ End,
            "<List> scrolls by itself",
        },
        .{ Counter ++ App ++ "<View>{items.map((n) => ~<Counter />)}</View>" ++ End, "<Counter> keeps its own state" },
        .{
            Counter ++ App ++ "<List data={items} renderItem={(n) => <View>~<Counter /></View>} />" ++ End,
            "<Counter> keeps its own state",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "allowed nestings compile" {
    const sources = [_][]const u8{
        App ++ "<Text>a <Text weight=\"bold\">b <Text italic>c</Text></Text> " ++
            "<Link onClick={() => {}}>d</Link></Text>" ++ End,
        App ++ "<Button><Icon name=\"check\" size={14} /> Save</Button>" ++ End,
        App ++ "<Row><Tag>new</Tag><Link onClick={() => {}}>more</Link></Row>" ++ End,
        App ++ "<Clickable onClick={() => {}}><Button>inner</Button></Clickable>" ++ End,
        App ++ "<Row><Modal title=\"t\" content={<Text>x</Text>} /></Row>" ++ End,
        App ++ "<View>{items.map((n) => <Text key={n}>{n}</Text>)}</View>" ++ End,
        Counter ++ App ++ "<View>{items.map((n) => <Counter key={n} />)}</View>" ++ End,
        App ++ "<Text>{items.length > 1 && \"many\"}</Text>" ++ End,
        App ++ "<View><Text key=\"a\">1</Text><View><Text key=\"a\">2</Text></View></View>" ++ End,
    };
    for (sources) |src| {
        var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
        defer arena.deinit();
        const result = try compile.compile(arena.allocator(), src, "ok.tsx");
        if (result == .err) std.debug.print("{s}\n{s}\n", .{ result.err.msg, src });
        try std.testing.expect(result == .ok);
    }
}
