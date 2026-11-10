//! Diagnostic positions and semantic regression tests for the code generator.
const std = @import("std");
const ast = @import("../ast.zig");
const tags = @import("../tags.zig");
const core = @import("codegen.zig");
const events = @import("events.zig");
const Generator = core.Generator;

/// A `~` marks the expected diagnostic byte in these fixtures. Checking the
/// human-facing position as well as the message catches misplaced diagnostics.
pub fn expectDiagnostic(marked: []const u8, message: []const u8) !void {
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const at = std.mem.indexOfScalar(u8, marked, '~').?;
    const src = try std.mem.concat(a, u8, &.{ marked[0..at], marked[at + 1 ..] });
    const result = try compiler.compile(a, src, "error.tsx");
    if (result != .err) {
        std.debug.print("expected TSX error containing: {s}\nsource: {s}\n", .{ message, src });
        return error.TestExpectedError;
    }
    const d = result.err;
    if (std.mem.indexOf(u8, d.msg, message) == null) {
        std.debug.print("expected diagnostic: {s}\nactual diagnostic: {s}\nsource: {s}\n", .{ message, d.msg, src });
        return error.TestExpectedEqual;
    }
    const expected = compiler.position(src, @intCast(at));
    const actual = compiler.position(src, d.loc);
    errdefer std.debug.print("diagnostic: {s}\nsource: {s}\n", .{ d.msg, src });
    try std.testing.expectEqual(expected.line, actual.line);
    try std.testing.expectEqual(expected.column, actual.column);
}

test "codegen diagnostics include precise source positions" {
    const cases = [_][2][]const u8{
        .{
            "export default function App() {\n return ~<Unknown/>;\n}",
            "unknown tag <Unknown>; valid tags: Column, Row, Clickable, Space, Text, Heading, " ++
                "Title, Paragraph, Button, Link, Input, Checkbox, Switch, RadioGroup, Tabs, Tooltip, " ++
                "Image, Select, Icon, IconButton, DismissLayer, RichText, PdfPreview, Divider, Tag, " ++
                "Card, Alert, Avatar, Progress, DropdownMenu, Segmented, Modal, ShortcutList, Message, " ++
                "HotkeyInput, View",
        },
        .{
            "export default function App() { return <Text ~oops={1}/>; }",
            "unknown prop `oops` on <Text>; valid props: key, size, weight, color, textAlign, " ++
                "wrap, lineHeight, maxLines, family, fontSize, fontWeight, numberOfLines, fontFamily",
        },
        .{
            "export default function App() { return <Button variant=~\"fancy\"/>; }",
            "invalid `variant` value `fancy`; expected: primary, default, text, link",
        },
        .{
            "export default function App() { return <View>~hello</View>; }",
            "text must be inside <Text>",
        },
        .{
            "export default function App() { const [x,setX] = useState(0); ~setX(1); return <View/>; }",
            "state can only be set in event handlers",
        },
        .{
            "export default function App() { const local=1; function f() { console.log(~local); } " ++
                "return <Button onClick={f}/>; }",
            "component functions cannot capture render locals; use an inline event handler",
        },
        .{
            "export default function App() { function f(x: boolean) {} return <Button ~onClick={f}/>; }",
            "handler must have signature () => void",
        },
        .{
            "export default function App() { return <Switch onChange={(~x: string) => console.log(x)}/>; }",
            "handler must have signature (checked: boolean) => void",
        },
        .{
            "export default function App() { const x: u32 = ~1.5; return <View/>; }",
            "cannot convert f64 to u32",
        },
        .{
            "export default function App() { const [x,sx]=useState<u32>(0); const " ++
                "[y,sy]=useState<u16>(0); const z=~x+y; return <Text>{z}</Text>; }",
            "different integer types u32 and u16; use the same integer type",
        },
        .{
            "export default function App() { const [x,sx]=useState(0); if (~x) return <View/>; return <View/>; }",
            "conditions must be boolean; compare the value explicitly",
        },
        .{
            "import { Text } from ~\"react\"; export default function App() { return <Text/>; }",
            "only imports from \"zant\" are supported",
        },
        .{
            "~function App() { return <View/>; }",
            "missing default exported component",
        },
        .{
            "export default function App() { return <View/>; }\n~function Child() { return <View/>; }",
            "other top-level functions are not supported; child components arrive in phase 2",
        },
        .{
            "export default function App() { return <View/>; }\nexport default ~function Child() { return <View/>; }",
            "exactly one component is supported; child components arrive in phase 2",
        },
        .{
            "export default function App() { if (true) { const [x,sx]=~useState(0); } return <View/>; }",
            "hooks must be called at the top level of the component",
        },
        .{
            "export default function App() { function f() { const [x,sx]=~useState(0); } return <View/>; }",
            "hooks must be called at the top level of the component",
        },
        .{
            "export default function App() { const x=1; return <View style={~x}/>; }",
            "style must be an object literal",
        },
        .{
            "export default function App() { const x=~2 ** 3; return <Text>{x}</Text>; }",
            "** is not supported yet",
        },
        .{
            "export default ~function App() { const x=1; }",
            "render must return a node on every path; the final statement must be a return",
        },
        .{
            "import { ~Unknown } from \"zant\"; export default function App() { return <View/>; }",
            "unknown import `Unknown`",
        },
        .{
            "import { Text as ~Label } from \"zant\"; export default function App() { return <Text/>; }",
            "tag imports cannot be renamed; use `Text`",
        },
        .{
            "export default function App(~props) { return <View/>; }",
            "component parameters are not supported; props arrive in phase 2",
        },
        .{
            "export default function App() { ~const [x] = useState(0); return <View/>; }",
            "useState needs exactly two names",
        },
        .{
            "export default function App() { const [x,sx] = useState(~1+2); return <View/>; }",
            "useState initializer must be a number, string, or boolean literal",
        },
        .{
            "export default function App() { const [x,sx] = useState(0); return <Button onClick={() => { ~x=1; }}/>; }",
            "use sx(…) to update state",
        },
        .{
            "export default function App() { return ~<> <Text/> </>; }",
            "a fragment is only allowed as a child; wrap it in <View>",
        },
        .{
            "export default function App() { return <Input>~text</Input>; }",
            "<Input> does not accept children",
        },
        .{
            "export default function App() { return <Text>~<View/></Text>; }",
            "text content cannot contain elements",
        },
        .{
            "export default function App() { return ~<Button text=\"a\">b</Button>; }",
            "<Button> cannot have both `text` and text children",
        },
        .{
            "export default function App() { return <Text ~fontSize/>; }",
            "bare attribute `fontSize` is only valid for boolean props",
        },
        .{
            "export default function App() { return <Text color=~\"red\"/>; }",
            "color must be #rgb, #rrggbb, or #rrggbbaa",
        },
        .{
            "export default function App() { return <Text color=~\"#ggg\"/>; }",
            "invalid hexadecimal color",
        },
        .{
            "export default function App() { return <View width=~\"auto\"/>; }",
            "dimension must be a number or a percentage literal",
        },
        .{
            "export default function App() { return <Text numberOfLines={~1.5}/>; }",
            "`numberOfLines` requires an integer expression",
        },
        .{
            "export default function App() { const [x,sx]=useState<u32>(0); const z=~-x; return <Text>{z}</Text>; }",
            "cannot negate an unsigned value; use a signed type",
        },
        .{
            "export default function App() { const z=!~1; return <Text>{z}</Text>; }",
            "conditions must be boolean",
        },
        .{
            "export default function App() { const z=~1 && true; return <Text>{z}</Text>; }",
            "conditions must be boolean",
        },
        .{
            "export default function App() { const [n,sn]=useState(0); return <Input onChange={~sn}/>; }",
            "setter `sn` does not match (value: string) => void",
        },
        .{
            "export default function App() { return <Button onClick={~true}/>; }",
            "event handler must be a component function, setter, or inline arrow",
        },
        .{
            "export default function App() { function f(~x) {} return <View/>; }",
            "parameter `x` needs a type annotation",
        },
        .{
            "export default function App() { ~function f(): u32 {} return <Button onClick={() => " ++
                "console.log(f())}/>; }",
            "function must return a value on every path",
        },
        .{
            "export default function App() { const x: ~Date=1; return <View/>; }",
            "unsupported type `Date`",
        },
        .{
            "export default function App() { return <View position={~true}/>; }",
            "`position` requires a literal string",
        },
        .{
            "export default function App() { const z=~true+1; return <Text>{z}</Text>; }",
            "operator `+` requires matching numbers",
        },
        .{
            "export default function App() { return <Button onClick={() => { const x=1; ~x++; }}/>; }",
            "`x` is const; declare it with let to assign it",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "later aliases and style replace fields and their obsolete usages" {
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compiler.compile(arena.allocator(),
        \\import { useState as state } from "zant";
        \\import type { u32 as Count } from "zant";
        \\export default function App() {
        \\  const [count, setCount] = state<Count>(1);
        \\  const old = count;
        \\  return <View radius={old} grow={old} width="50%"
        \\    style={{ borderRadius: 4, flexGrow: 2, width: 80 }} />;
        \\}
    , "aliases.tsx");
    try std.testing.expect(result == .ok);
    const output = result.ok;
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, output, ".radius ="));
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, output, ".grow ="));
    try std.testing.expectEqual(@as(usize, 0), std.mem.count(u8, output, ".width_percent ="));
    try std.testing.expect(std.mem.indexOf(u8, output, "_ = old;") != null);
}

test "state field names do not prevent safe snapshot and parameter names" {
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compiler.compile(arena.allocator(),
        \\export default function App() {
        \\  const [ui, setUi] = useState<u32>(0);
        \\  function change(ui: u32) { setUi(1); }
        \\  function snapshot() { setUi(ui + 1); }
        \\  return <Button onClick={snapshot}>{ui}</Button>;
        \\}
    , "names.tsx");
    try std.testing.expect(result == .ok);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "_ = ui_2;") != null);
    try std.testing.expect(std.mem.indexOf(u8, result.ok, "const ui_2: u32 = self.ui;") != null);
    // The field name in self.ui must never count as usage of the parameter.
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, result.ok, "const ui_2: u32 = self.ui;"));
}

test "index setters use a checked integer cast or integer to float conversion" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    var diag: ?core.Diag = null;
    var g = Generator{ .a = a, .diag = &diag, .name = "App" };
    try g.states.append(a, .{ .name = "index", .setter = "setIndex", .ty = .u16, .initial = "0" });
    try g.states.append(a, .{ .name = "offset", .setter = "setOffset", .ty = .f32, .initial = "0" });
    var integer = ast.Expr{ .loc = 0, .data = .{ .ident = "setIndex" } };
    var float = ast.Expr{ .loc = 0, .data = .{ .ident = "setOffset" } };
    const prop = tags.Prop{ .name = "onIndex", .kind = .index_action };
    try events.event(&g, &integer, &prop, 0);
    try events.event(&g, &integer, &prop, 0);
    try events.event(&g, &float, &prop, 0);
    try std.testing.expectEqual(@as(usize, 2), g.methods.items.len);
    try std.testing.expect(std.mem.indexOf(
        u8,
        try core.emitMethod(&g, g.methods.items[0]),
        "self.index = @intCast(value);",
    ) != null);
    try std.testing.expect(std.mem.indexOf(
        u8,
        try core.emitMethod(&g, g.methods.items[1]),
        "self.offset = @floatFromInt(value);",
    ) != null);
}

test "additional semantic errors stop before emitting invalid Zig" {
    const cases = [_][2][]const u8{
        .{
            "export default function App() { const [x,sx]=useState<u8>(~256); return <View/>; }",
            "integer literal does not fit u8",
        },
        .{
            "export default function App() { const [x,sx]=useState<u32>(~-1); return <View/>; }",
            "integer literal does not fit u32",
        },
        .{
            "export default function App() { function f(): void { return ~1; } return <View/>; }",
            "void functions cannot return a value",
        },
        .{
            "export default function App() { const [x,sx]=useState(0); return <Button onClick={() " ++
                "=> ~_onClick1()}/>; }",
            "unknown function `_onClick1`",
        },
        .{
            "export default function App() { const [x,sx]=useState(0); const f=1; ~f(); return <View/>; }",
            "`f` is a local value, not a component function",
        },
        .{
            "export default function App() { function f(x:u32):u32 { return x; } const " ++
                "y=~f<u32>(1); return <Text>{y}</Text>; }",
            "type arguments are only supported on useState",
        },
        .{
            "export default function App() { const [a,sa]=useState<u8>(0); const " ++
                "[b,sb]=useState<u32>(0); const x:u32=~a; return <View/>; }",
            "cannot convert u8 to u32",
        },
        .{
            "export default function App() { if (true) { const local=1; } return <Button " ++
                "onClick={() => console.log(~local)}/>; }",
            "component functions cannot capture render locals",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "generated component props and explicit casts report source positions" {
    const cases = [_][2][]const u8{
        .{
            "export default function App() { return ~<Icon/>; }",
            "<Icon> is missing required prop `name`",
        },
        .{
            "export default function App() { return ~<Tooltip><Button/></Tooltip>; }",
            "<Tooltip> is missing required prop `text`",
        },
        .{
            "export default function App() { return ~<DropdownMenu/>; }",
            "<DropdownMenu> is missing required prop `items`",
        },
        .{
            "export default function App() { return <RadioGroup options={[{~oops:true}]}/>; }",
            "options[0]: unknown key `oops`; valid keys: label, disabled",
        },
        .{
            "export default function App() { return <Tabs items={[~{label: \"A\"}]}/>; }",
            "items[0] is missing required key `content`",
        },
        .{
            "export default function App() { return <RadioGroup options={[~{disabled:false}]}/>; }",
            "options[0] is missing required key `label`",
        },
        .{
            "export default function App() { return <ShortcutList groups={[{title:\"A\",items:[~{label:\"B\"}]}]}/>; }",
            "in groups[0].items: items[0] is missing required key `keys`",
        },
        .{
            "export default function App() { return <Tabs items={[{label:\"A\",content:~\"text\"}]}/>; }",
            "in items[0].content: prop `content` requires an element; wrap text in <Text>",
        },
        .{
            "export default function App() { return <RadioGroup options={[{label:\"A\",disabled:~1}]}/>; }",
            "in options[0].disabled: cannot convert f64 to bool",
        },
        .{
            "export default function App() { return <RadioGroup options={[~1]}/>; }",
            "options[0] must be an object literal",
        },
        .{
            "export default function App() { return ~<Tooltip text=\"A\"/>; }",
            "<Tooltip> requires exactly one child element or the `child` prop",
        },
        .{
            "export default function App() { return ~<Tooltip text=\"A\"><Button/><Button/></Tooltip>; }",
            "<Tooltip> requires exactly one child element or the `child` prop",
        },
        .{
            "export default function App() { return ~<Tooltip text=\"A\" child={<Button/>}><Button/></Tooltip>; }",
            "<Tooltip> cannot have both `child` and child elements",
        },
        .{
            "export default function App() { return <Tooltip text=\"A\" child={~123}/>; }",
            "prop `child` requires an element; wrap text in <Text>",
        },
        .{
            "export default function App() { return <Modal content={<Text/>} footer={~true}/>; }",
            "prop `footer` requires an element or null",
        },
        .{
            "export default function App() { const x=~true as u32; return <Text>{x}</Text>; }",
            "cannot cast bool to u32; only numeric conversions are supported",
        },
        .{
            "export default function App() { const x=~1 as string; return <Text>{x}</Text>; }",
            "cannot cast f64 to []const u8; only numeric conversions are supported",
        },
        .{
            "export default function App() { return <Select options={[~1]}/>; }",
            "cannot convert f64 to []const u8",
        },
        .{
            "export default function App() { return <Card children={~true}/>; }",
            "prop `children` requires an array of nodes",
        },
        .{
            "export default function App() { return <View padding={{~oops:1}}/>; }",
            "unknown edge `oops`; valid keys: top, right, bottom, left, horizontal, vertical",
        },
        .{
            "export default function App() { return <DropdownMenu items={[{onClick:~(x:boolean)=>{}}]}/>; }",
            "in items[0].onClick: handler must have signature () => void",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test {
    _ = @import("round2_test.zig");
}
