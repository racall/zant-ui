//! Readable output regressions, including places where Zig still needs a type.
const std = @import("std");
const compiler = @import("../compile.zig");

fn output(a: std.mem.Allocator, src: []const u8) ![]const u8 {
    const result = try compiler.compile(a, src, "readable.tsx");
    if (result == .err) {
        std.debug.print("{s}", .{try compiler.renderDiag(a, "readable.tsx", src, result.err)});
        return error.UnexpectedDiagnostic;
    }
    return result.ok;
}

fn contains(actual: []const u8, expected: []const u8) !void {
    if (std.mem.indexOf(u8, actual, expected) == null) {
        std.debug.print("missing expected output:\n{s}\nin:\n{s}\n", .{ expected, actual });
        return error.TestExpectedEqual;
    }
}

test "typed literals, conversions, calls, and returns use their result location" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const zig = try output(arena.allocator(),
        \\export default function App() {
        \\  const [count, setCount] = useState<u32>(0);
        \\  const [signed, setSigned] = useState<i32>(-1);
        \\  const narrow: f32 = count;
        \\  function take(n: u32): u32 { return n; }
        \\  function zero(): u32 { return 0; }
        \\  return <Button width={14} onClick={() => setCount(0)}>{take(2)} {zero()} {narrow} {signed}</Button>;
        \\}
    );
    try contains(zig, "count: u32 = 0,");
    try contains(zig, "signed: i32 = -1,");
    try contains(zig, "const narrow: f32 = @floatFromInt(self.count);");
    try contains(zig, "self.take(2)");
    try contains(zig, "return 0;");
    try contains(zig, "self.count = 0;");
    try contains(zig, ".width = 14,");
    try std.testing.expect(std.mem.indexOf(u8, zig, "@as(") == null);
}

test "parentheses preserve precedence and right operand grouping" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const zig = try output(arena.allocator(),
        \\export default function App() {
        \\  const [x, setX] = useState(3);
        \\  const [flag, setFlag] = useState(true);
        \\  const a = (x + 1) * 2;
        \\  const b = x - (x - 1);
        \\  const c = x + (x + 1);
        \\  const d = (flag ? x : x + 1) * 2;
        \\  const e = !(x > 0 && flag);
        \\  const name = (flag ? "A" : "B").length;
        \\  if (e) return <Text>{name}</Text>;
        \\  return <Text>{a} {b} {c} {d}</Text>;
        \\}
    );
    try contains(zig, "const a: f64 = (self.x + 1) * 2;");
    try contains(zig, "const b: f64 = self.x - (self.x - 1);");
    try contains(zig, "const c: f64 = self.x + (self.x + 1);");
    try contains(zig, "const d: f64 = (if (self.flag) self.x else self.x + 1) * 2;");
    try contains(zig, "const e: bool = !(self.x > 0 and self.flag);");
    try contains(zig, "const name: usize = (if (self.flag) \"A\" else \"B\").len;");
    try contains(zig, "if (e) {");
}

test "untyped operands and format tuples retain necessary numeric types" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const zig = try output(arena.allocator(),
        \\export default function App() {
        \\  const [flag, setFlag] = useState(true);
        \\  const [count, setCount] = useState<u32>(1);
        \\  const half = 1 / 2;
        \\  const mixed = count + 0.5;
        \\  const choice = flag ? 1 : 2;
        \\  return <Text>{half} {mixed} {choice} {flag ? 1 : 2} {1.25} {9007199254740993}</Text>;
        \\}
    );
    try contains(zig, "const half: f64 = @as(f64, 1) / 2;");
    try contains(zig, "const mixed: f64 = @as(f64, @floatFromInt(self.count)) + 0.5;");
    try contains(zig, "const choice: f64 = if (self.flag) 1 else 2;");
    try contains(zig, "@as(f64, if (self.flag) 1 else 2)");
    try contains(zig, "@as(f64, 9007199254740993)");
    try std.testing.expect(std.mem.indexOf(u8, zig, "@as(f64, 1.25)") == null);
    // Integer / would produce zero even under an f64 declaration. The typed
    // operand in the emitted code is required to preserve TS number semantics.
    const expected: f64 = @as(f64, 1) / 2;
    try std.testing.expectEqual(@as(f64, 0.5), expected);
}

test "text content, inline tuples, and method spacing remain readable" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const zig = try output(arena.allocator(),
        \\export default function App() {
        \\  const [name, setName] = useState("Zant");
        \\  return <Button key="button" disabled onClick={() => setName(p => p + "!")}>Hello, {name}!</Button>;
        \\}
    );
    try contains(zig, "const tsx = zant.tsx;\n\npub const App");
    try contains(zig, "name: tsx.String = .{ .initial = \"Zant\" },\n\n    pub fn render");
    try contains(zig, ".key = \"button\",\n            .text = tsx.fmt(\"Hello, {s}!\", " ++
        ".{self.name.get()}),\n            .disabled = true,");
    try contains(zig, "    }\n\n    fn _onClick1");
    try contains(zig, "ui.frameString(self.name.get());\n            self.name.set(tsx.fmt(\"{s}!\", .{p}));");
}

test "literal list props include empty arrays and nested Clickable children" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const zig = try output(arena.allocator(),
        \\export default function App() {
        \\  const [show, setShow] = useState(true);
        \\  return <Clickable key="box" gap={2} style={{gap: 6}} children={[
        \\    <Tabs items={[]} />,
        \\    <Select options={[]} />,
        \\    show ? <Card children={[]} /> : null
        \\  ]} />;
        \\}
    );
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, zig, ".box ="));
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, zig, ".gap ="));
    try contains(zig, ".gap = 6,");
    try contains(zig, ".children = tsx.nodes(&.{");
    try contains(zig, ".items = &[_]ui.TabItem{},");
    try contains(zig, ".options = &.{},");
    try contains(zig, ".children = &.{},");
}

test "style can satisfy required props and same-type assertions are no-ops" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const zig = try output(arena.allocator(),
        \\export default function App() {
        \\  const [show, setShow] = useState(true);
        \\  const yes = true as boolean;
        \\  const name = "X" as string;
        \\  const number = (1 as number)!;
        \\  const choice: u32 = (show ? 1 : 2)!;
        \\  const sum: u32 = (show ? 1 : 2)! + 3;
        \\  const optional = show ? <Text>Optional</Text> : null;
        \\  return <Modal footer={optional!} content={
        \\    <Tooltip style={{text: "Save", child: <Icon style={{name: "eyeOff"}} />}} />
        \\  } title={`${name} ${number} ${yes} ${choice} ${sum}`} />;
        \\}
    );
    try contains(zig, "const yes: bool = true;");
    try contains(zig, "const name: []const u8 = \"X\";");
    try contains(zig, "const number: f64 = 1;");
    try contains(zig, "const choice: u32 = if (self.show) 1 else 2;");
    try contains(zig, "const sum: u32 = @as(u32, if (self.show) 1 else 2) + 3;");
    try contains(zig, ".footer = optional,");
    try contains(zig, ".name = .eye_off,");
    try std.testing.expect(std.mem.indexOf(u8, zig, ".?") == null);
}
