//! Project-only Zig imports, diagnostics, and output handling.
const std = @import("std");
const compile = @import("compile.zig");

const zig_source =
    \\// Keep this file byte for byte.
    \\pub fn plural(n: usize, word: []const u8) []const u8 { return word; }
    \\pub fn clamp(n: f32) f32 { return n; }
    \\pub fn record(n: usize) void {}
    \\pub fn values(xs: []const ?i32) []const ?i32 { return xs; }
    \\pub fn words(xs: []const []const u8) []const []const u8 { return xs; }
    \\pub fn maybe(n: ?usize) ?usize { return n; }
    \\pub fn bad(x: Foo) void {}
    \\fn hidden() void {}
    \\pub const value = 1;
;

test "project Zig imports call typed functions in render, children and handlers" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const result = try compile.compileProject(a, &.{
        .{ .path = "src/format.zig", .src = zig_source },
        .{ .path = "src/App.tsx", .src =
        \\import { plural as label, clamp, values, words, maybe } from "./format.zig";
        \\import { record } from "./format.zig";
        \\import { Child } from "./Child";
        \\export default function App() {
        \\  const title = label(3, "note");
        \\  const xs = values([1, null]);
        \\  const names = words(["a", "b"]);
        \\  function save() { record(maybe(1) ?? 0); }
        \\  return <View><Text>{title} {xs.length} {names.join(",")}</Text>
        \\    <Button width={clamp(24)} onClick={save}>Save</Button><Child /></View>;
        \\}
        },
        .{ .path = "src/Child.tsx", .src =
        \\import { plural } from "./format.zig";
        \\export function Child() { return <Text>{plural(1, "note")}</Text>; }
        },
    });
    if (result == .err) std.debug.print("{s}\n", .{result.err.diag.msg});
    try std.testing.expect(result == .ok);
    try std.testing.expectEqualStrings(zig_source, result.ok[0]);
    const app = result.ok[1];
    try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, app, "const _format = @import(\"format.zig\");"));
    try contains(app, "_format.plural(3, \"note\")");
    try contains(app, "_format.clamp(24)");
    try contains(app, "_format.record(");
    try contains(result.ok[2], "_format.plural(1, \"note\")");
}

fn contains(actual: []const u8, expected: []const u8) !void {
    try std.testing.expect(std.mem.indexOf(u8, actual, expected) != null);
}

fn projectError(src: []const u8, expected: []const u8, at_import: bool) !void {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compileProject(arena.allocator(), &.{
        .{ .path = "App.tsx", .src = src },
        .{ .path = "format.zig", .src = zig_source },
    });
    try std.testing.expect(result == .err);
    try std.testing.expectEqual(@as(usize, 0), result.err.file);
    try contains(result.err.diag.msg, expected);
    if (at_import) try std.testing.expect(result.err.diag.loc < std.mem.indexOf(u8, src, "from").?);
}

test "project Zig import diagnostics point at imported names" {
    const app = " export default function App() { return <View/>; }";
    try projectError(
        "import { plurla } from \"./format.zig\";" ++ app,
        "`plurla` is not a pub fn of `./format.zig`; did you mean `plural`?",
        true,
    );
    try projectError(
        "import { hidden } from \"./format.zig\";" ++ app,
        "`hidden` is not a pub fn of `./format.zig`",
        true,
    );
    try projectError(
        "import { value } from \"./format.zig\";" ++ app,
        "`value` is not a pub fn of `./format.zig`",
        true,
    );
    try projectError(
        "import { bad } from \"./format.zig\";" ++ app,
        "`bad` takes `Foo`; Zig functions imported into TSX use bool, string, numbers, optionals and slices of them",
        true,
    );
    // a function imported as a type
    try projectError("import type { plural } from \"./format.zig\";" ++ app, "is not a pub struct or enum", true);
    try projectError("import { type plural } from \"./format.zig\";" ++ app, "is not a pub struct or enum", true);
    try projectError("import { plural } from \"./missing.zig\";" ++ app, "cannot find `./missing.zig`", false);
}

test "project Zig calls check argument counts, types and type arguments" {
    const prefix = "import { plural } from \"./format.zig\"; export default function App() { return <Text>{";
    const suffix = "}</Text>; }";
    try projectError(prefix ++ "plural(1)" ++ suffix, "`plural` expects 2 arguments", false);
    try projectError(prefix ++ "plural(true, \"note\")" ++ suffix, "cannot convert bool to usize", false);
    try projectError(prefix ++ "plural(1, 2)" ++ suffix, "cannot convert", false);
    try projectError(prefix ++ "plural<usize>(1, \"note\")" ++ suffix, "do not take TSX type arguments", false);
}

test "Zig imports require project mode" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const src = "import { plural } from \"./format.zig\"; export default function App() { return <View/>; }";
    const result = try compile.compile(arena.allocator(), src, "App.tsx");
    try std.testing.expectEqualStrings("Zig imports need a project (zant-tsx --out / addTsxProject)", result.err.msg);
}

test "project rejects Zig and TSX output alias collisions in either order" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const tsx = compile.File{ .path = "format.tsx", .src = "export default function Format() { return <View/>; }" };
    const zig = compile.File{ .path = "format.zig", .src = zig_source };
    for ([_][2]compile.File{ .{ tsx, zig }, .{ zig, tsx } }) |files| {
        const result = try compile.compileProject(arena.allocator(), &files);
        try std.testing.expectEqualStrings("project output collision: `format.zig`", result.err.diag.msg);
        try std.testing.expectEqual(@as(usize, 1), result.err.file);
    }
}

test "project validates Zig syntax even for unimported files" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compileProject(arena.allocator(), &.{
        .{ .path = "bad.zig", .src = "pub fn broken(" },
    });
    try std.testing.expect(result == .err);
    try std.testing.expectEqual(@as(usize, 0), result.err.file);
    try contains(result.err.diag.msg, "expected");
}

test "a store exported by one file is shared by the files importing it" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compileProject(arena.allocator(), &.{
        .{ .path = "src/store.tsx", .src =
        \\export interface Prefs { dark: boolean; name: string; }
        \\export const prefs = createStore<Prefs>({ dark: false, name: "" });
        },
        .{ .path = "src/App.tsx", .src =
        \\import { prefs as shared } from "./store";
        \\export default function App() {
        \\  const dark = useStore(shared, (p) => p.dark);
        \\  return <Switch checked={dark} onChange={(on) => shared.update({ dark: on })} />;
        \\}
        },
    });
    if (result == .err) std.debug.print("{s}\n", .{result.err.diag.msg});
    try std.testing.expect(result == .ok);
    try contains(result.ok[0], "pub var prefs: tsx.Store(Prefs)");
    try contains(result.ok[1], "const _store = @import(\"store.zig\");");
    try contains(result.ok[1], "const p: _store.Prefs = _store.prefs.read();");
    try contains(result.ok[1], "_store.prefs.update(.{");
}

test "Zig functions take and return bytes as tsx.Bytes (Uint8Array)" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const result = try compile.compileProject(arena.allocator(), &.{
        .{ .path = "src/image.zig", .src =
        \\const zant = @import("zant");
        \\const tsx = zant.tsx;
        \\pub fn png() ?zant.tsx.Bytes { return null; }
        \\pub fn size(data: tsx.Bytes) usize { return data.len; }
        \\pub fn name() []const u8 { return "a.png"; }
        },
        .{ .path = "src/App.tsx", .src =
        \\import { png, size, name } from "./image.zig";
        \\export default function App() {
        \\  const data: Uint8Array = png() ?? [];
        \\  return <Text>{name()}: {size(data)} bytes, first {data.length > 0 ? data[0] : 0}</Text>;
        \\}
        },
    });
    if (result == .err) std.debug.print("{s}\n", .{result.err.diag.msg});
    try std.testing.expect(result == .ok);
    try contains(result.ok[1], "const data: []const u8 = _image.png() orelse");
    try contains(result.ok[1], "_image.size(data)");
}
