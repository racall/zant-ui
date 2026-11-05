//! Structural hashing: hashes a value field by field so padding bytes never
//! leak into the result (hashing `std.mem.asBytes(&struct)` reads undefined
//! padding and makes memo keys change at random).
//!
//! Rules: bool/int/enum by value; floats by bit pattern; optionals by
//! presence + payload; arrays element-wise; structs recursively; tagged
//! unions by tag + payload; `[]const u8` (and other byte slices) by content;
//! other slices element-wise; every other pointer (single, many, function,
//! opaque) by address.
const std = @import("std");

pub fn hashValue(h: *std.hash.Wyhash, value: anytype) void {
    const T = @TypeOf(value);
    switch (@typeInfo(T)) {
        .void, .null, .undefined => {},
        .bool => h.update(&.{@intFromBool(value)}),
        .int => |info| {
            const U = @Int(.unsigned, @max(8, std.mem.alignForward(u16, info.bits, 8)));
            const widened: U = @bitCast(@as(@Int(info.signedness, @bitSizeOf(U)), value));
            h.update(std.mem.asBytes(&widened));
        },
        .comptime_int => hashValue(h, @as(i64, value)),
        .float => |info| {
            const bits: @Int(.unsigned, info.bits) = @bitCast(value);
            hashValue(h, bits);
        },
        .comptime_float => hashValue(h, @as(f64, value)),
        .@"enum" => hashValue(h, @backingInt(value)),
        .optional => if (value) |payload| {
            h.update(&.{1});
            hashValue(h, payload);
        } else h.update(&.{0}),
        .array => for (value) |item| hashValue(h, item),
        .vector => |info| {
            var i: usize = 0;
            while (i < info.len) : (i += 1) hashValue(h, value[i]);
        },
        .@"struct" => |info| inline for (info.field_names) |name| hashValue(h, @field(value, name)),
        .@"union" => |info| {
            if (info.tag_type == null) @compileError("hashValue: untagged union " ++ @typeName(T));
            switch (value) {
                inline else => |payload, tag| {
                    hashValue(h, tag);
                    hashValue(h, payload);
                },
            }
        },
        .pointer => |info| switch (info.size) {
            .slice => {
                hashValue(h, value.len);
                if (info.child == u8) {
                    h.update(value);
                } else for (value) |item| hashValue(h, item);
            },
            else => hashValue(h, @intFromPtr(value)),
        },
        .error_set => hashValue(h, @intFromError(value)),
        else => @compileError("hashValue: unsupported type " ++ @typeName(T)),
    }
}

/// Convenience: one-shot hash of `value` with `seed`.
pub fn hash(seed: u64, value: anytype) u64 {
    var h = std.hash.Wyhash.init(seed);
    hashValue(&h, value);
    return h.final();
}

test "structural hash ignores padding and sees content" {
    const S = struct {
        a: u8,
        b: u64,
        name: []const u8,
        opt: ?f32,
        kind: enum { x, y },
        u: union(enum) { n: u16, s: []const u8 },
    };
    var buf1: [3]u8 = "abc".*;
    var buf2: [3]u8 = "abc".*;
    var one: S = undefined;
    @memset(std.mem.asBytes(&one), 0xaa);
    one.a = 1;
    one.b = 2;
    one.name = &buf1;
    one.opt = 1.5;
    one.kind = .y;
    one.u = .{ .s = "hi" };
    var two: S = undefined;
    @memset(std.mem.asBytes(&two), 0x55);
    two.a = 1;
    two.b = 2;
    two.name = &buf2;
    two.opt = 1.5;
    two.kind = .y;
    two.u = .{ .s = "hi" };
    try std.testing.expectEqual(hash(0, one), hash(0, two));
    two.name = "abd";
    try std.testing.expect(hash(0, one) != hash(0, two));
    two.name = "abc";
    two.u = .{ .n = 3 };
    try std.testing.expect(hash(0, one) != hash(0, two));
}
