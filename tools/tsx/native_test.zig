//! Native module specs (`native/*.ts`, `codegen/native_specs.zig`).
const std = @import("std");
const compile = @import("compile.zig");
const entry = @import("entry.zig");

const spec_src =
    \\import type { i64 } from "zant";
    \\import type { Kind } from "./kinds";
    \\export interface ListEntry { row: i64; kind: Kind; preview: string }
    \\export type CopyResult = "ok" | "not_found";
    \\export declare function entries(query: string): ListEntry[];
    \\export declare function copy(row: i64): CopyResult;
    \\export declare function clear(): void;
;
const kinds_src =
    \\export type Kind = "text" | "image";
;
const app_src =
    \\import { entries, copy, type ListEntry } from "native/clips";
    \\export default function App() {
    \\  const list: ListEntry[] = entries("");
    \\  return <View>{list.map((e) => <Button key={e.row} onClick={() => copy(e.row)}>{e.preview}</Button>)}</View>;
    \\}
;

fn project(a: std.mem.Allocator, app: []const u8, spec: []const u8) !compile.ProjectResult {
    return compile.compileProject(a, &.{
        .{ .path = "src/App.tsx", .src = app },
        .{ .path = "native/clips.ts", .src = spec, .native = true },
        .{ .path = "native/kinds.ts", .src = kinds_src, .native = true },
    });
}

fn has(haystack: []const u8, needle: []const u8) !void {
    if (std.mem.indexOf(u8, haystack, needle) == null) {
        std.debug.print("missing `{s}` in:\n{s}\n", .{ needle, haystack });
        return error.TestUnexpectedResult;
    }
}

test "a spec compiles to its types and a conformance check" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const files = (try project(a, app_src, spec_src)).ok;
    const spec = files[1];
    try has(spec, "const _spec_kinds = @import(\"_spec_kinds.zig\");");
    try has(spec, "pub const ListEntry = struct");
    try has(spec, "kind: _spec_kinds.Kind");
    try has(spec, "pub const CopyResult = enum");
    try has(spec, "tsx.native.expect(Impl, \"clips\", \"entries\", fn ([]const u8) []const ListEntry");
    try has(spec, "\"pub fn copy(row: i64) CopyResult\"");
    try has(spec, "fn () void");
    // the app reaches the same types through the spec module, and calls the code directly
    const app = files[0];
    try has(app, "const _spec_clips = @import(\"spec\").clips;");
    try has(app, "const _native_clips = @import(\"native\").clips;");
    try has(app, "_native_clips.entries(\"\")");
    try has(app, "_native_clips.copy(");
    try std.testing.expect(std.mem.indexOf(u8, app, "pub const ListEntry") == null);
}

test "the entry checks every native module and the spec root lists them" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const files: []const compile.File = &.{
        .{ .path = "src/App.tsx", .src = "export default function App() { return <View/>; }" },
        .{ .path = "native/clips.ts", .src = spec_src, .native = true },
    };
    const main = try entry.mainSource(a, files, "App", true);
    try has(main, "pub const config = @import(\"app_config\");");
    try has(main, "zant.tsx.native.module(@import(\"native\"), \"clips\", @import(\"spec\").clips);");
    try has(try entry.specRoot(a, files), "pub const clips = @import(\"_spec_clips.zig\");");
}

test "spec errors point at the spec" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const a = arena.allocator();
    const plain = "export default function App() { return <View/>; }";
    const body = (try project(a, plain, "export function copy(row: number): number { return row; }")).err;
    try std.testing.expectEqual(@as(usize, 1), body.file);
    try has(body.diag.msg, "write them in native/clips.zig");
    const missing = (try project(a, "import { paste } from \"native/clips\";\nexport default function App() " ++
        "{ return <View/>; }", spec_src)).err;
    try std.testing.expectEqual(@as(usize, 0), missing.file);
    try std.testing.expectEqualStrings("native/clips.ts declares no exported `paste`", missing.diag.msg);
    const unknown = (try project(a, "import { x } from \"native/files\";\nexport default function App() " ++
        "{ return <View/>; }", spec_src)).err;
    try has(unknown.diag.msg, "add native/files.ts (the spec) and native/files.zig");
    const outside = (try compile.compileProject(a, &.{.{
        .path = "src/App.tsx",
        .src = "export declare function f(): void;\nexport default function App() { return <View/>; }",
    }})).err;
    try has(outside.diag.msg, "belongs in a native module spec");
}
