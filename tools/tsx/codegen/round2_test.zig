//! Compound-type diagnostics, with source locations checked by the shared helper.
const expectDiagnostic = @import("errors_test.zig").expectDiagnostic;

test "round 2 type and closure errors" {
    const cases = [_][2][]const u8{
        .{
            "export default function App(){ const x=~{a:1}; return <Text/>; }",
            "object literal needs a type annotation",
        },
        .{
            "type F=\"all\"|\"done\"; export default function App(){ const f:F=~\"bad\"; return <Text/>; }",
            "unknown F member `bad`; expected: all, done",
        },
        .{
            "export default function App(){ const x:number|null=null; const y:number=~x; return <Text/>; }",
            "optional value needs a fallback with ?? or a non-null assertion !",
        },
        .{
            "interface X {a:number} export default function App(){ const x:X={a:1}; const y=~x===x; return <Text/>; }",
            "equality on structs and arrays is not supported",
        },
        .{
            "export default function App(){ const a=[1]; const b=a.map(~(x,i,j)=>x); return <Text/>; }",
            "array callback expects at most 2 parameters",
        },
        .{
            "export default function App(){ const x=1; function f(){console.log(~x);} return <Button onClick={f}/>; }",
            "component functions cannot capture render locals",
        },
        .{
            "export default function App(){ const a=[1]; const x=~a===a; return <Text/>; }",
            "equality on structs and arrays is not supported",
        },
        .{
            "export default function App(){ const a:number|null=null; const x=~a+1; return <Text/>; }",
            "optional operand needs ?? or !",
        },
        .{
            "export default function App(){ const a:string|null=null; const x=~a===\"x\"; return <Text/>; }",
            "optional operand needs ?? or !",
        },
        .{
            "interface X {a:string|null} export default function App(){ const x:X=~{}; return <Text/>; }",
            "X is missing field `a`",
        },
        .{
            "export default function App(){ const a=[1]; const b=a.map((x:~string)=>x); return <Text/>; }",
            "callback parameter must have type f64",
        },
        .{
            "export default function App(){ const a=[1]; const b=a[~true]; return <Text/>; }",
            "array index must be numeric",
        },
        .{
            "export default function App(){ const a=[1]; const b=a.map(x=>{~return;}); return <Text/>; }",
            "array callbacks must return a value",
        },
    };
    for (cases) |case| try expectDiagnostic(case[0], case[1]);
}

test "object values keep source evaluation order" {
    const std = @import("std");
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const source =
        \\interface Pair { first: number; second: number }
        \\export default function App() {
        \\  const [n, setN] = useState(0);
        \\  function next(): number { setN(n + 1); return n; }
        \\  function run() {
        \\    const pair: Pair = { second: next(), first: next() };
        \\    console.log(pair.first, pair.second);
        \\  }
        \\  return <Button onClick={run}/>;
        \\}
    ;
    const result = try compiler.compile(arena.allocator(), source, "order.tsx");
    try std.testing.expect(result == .ok);
    const second = std.mem.indexOf(u8, result.ok, ".second = self.next()").?;
    const first = std.mem.indexOf(u8, result.ok, ".first = self.next()").?;
    try std.testing.expect(second < first);
}

test "nullable inferred arrays remain valid children" {
    const std = @import("std");
    const compiler = @import("../compile.zig");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const source =
        \\export default function App() {
        \\  const values = [null, 1];
        \\  const empty = values.map(() => null);
        \\  return <View>{empty}<Text>{values[0] ?? 0}</Text></View>;
        \\}
    ;
    const result = try compiler.compile(arena.allocator(), source, "nullable.tsx");
    if (result == .err) std.debug.print("{s}", .{
        try compiler.renderDiag(arena.allocator(), "nullable.tsx", source, result.err),
    });
    try std.testing.expect(result == .ok);
}
