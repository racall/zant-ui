//! Compile-time checks of a TS + TSX app's native modules: every function a
//! spec (`native/clips.ts`) declares must be a `pub fn` of the module
//! (`native/clips.zig`) with exactly the spec's signature. The generated
//! code calls these; the messages name the files to change.
const std = @import("std");

/// `native/lib.zig` lists the module, and the module implements its spec.
pub fn module(comptime Native: type, comptime name: []const u8, comptime Spec: type) void {
    if (!@hasDecl(Native, name)) @compileError("native/lib.zig: add `pub const " ++ name ++ " = @import(\"" ++
        name ++ ".zig\");` (the implementation of " ++ Spec.spec_file ++ ")");
    Spec.check(@field(Native, name));
}

/// The function `name` of `Impl` has the type `F` the spec declares.
pub fn expect(
    comptime Impl: type,
    comptime module_name: []const u8,
    comptime name: []const u8,
    comptime F: type,
    comptime signature: []const u8,
) void {
    const file = "native/" ++ module_name ++ ".zig";
    const spec = "native/" ++ module_name ++ ".ts";
    if (!@hasDecl(Impl, name)) @compileError(file ++ ": missing `" ++ signature ++ "` (declared in " ++ spec ++ ")");
    const actual = @TypeOf(@field(Impl, name));
    if (actual != F) @compileError(file ++ ": `" ++ name ++ "` is `" ++ @typeName(actual) ++ "`, but " ++ spec ++
        " declares `" ++ signature ++ "` (use the spec's types: `const spec = @import(\"spec\")." ++
        module_name ++ ";`)");
}

test "a module that matches its spec passes" {
    const Spec = struct {
        pub const spec_file = "native/demo.ts";
        pub fn check(comptime Impl: type) void {
            expect(Impl, "demo", "twice", fn (i64) i64, "pub fn twice(n: i64) i64");
        }
    };
    const Native = struct {
        pub const demo = struct {
            pub fn twice(n: i64) i64 {
                return n * 2;
            }
        };
    };
    comptime module(Native, "demo", Spec);
    try std.testing.expectEqual(@as(i64, 4), Native.demo.twice(2));
}
