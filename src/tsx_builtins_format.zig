//! Adapt only floating-point {d} fields; preserve integer precision and other Zig format specifiers.
const std = @import("std");
const ui = @import("ui.zig");
const builtins = @import("tsx_builtins.zig");

fn Arguments(comptime Args: type, comptime converted: anytype) type {
    const info = @typeInfo(Args).@"struct";
    var types: [info.field_types.len]type = undefined;
    var attrs: [info.field_types.len]std.builtin.Type.Struct.FieldAttributes = undefined;
    for (info.field_types, info.field_attrs, converted, 0..) |T, attr, change, i| {
        types[i] = if (change) []const u8 else T;
        attrs[i] = if (change) .{} else attr;
    }
    return @Struct(.auto, null, info.field_names, &types, &attrs);
}

/// Generated format strings use sequential {d}, {s}, and {} fields, with escaped braces.
pub fn format(comptime pattern: []const u8, args: anytype) []const u8 {
    const info = @typeInfo(@TypeOf(args)).@"struct";
    const spec = comptime blk: {
        var result: []const u8 = "";
        var converted: [info.field_types.len]bool = @splat(false);
        var arg: usize = 0;
        var i: usize = 0;
        while (i < pattern.len) {
            if (pattern[i] == '{' and i + 1 < pattern.len and pattern[i + 1] != '{') {
                const end = std.mem.indexOfScalarPos(u8, pattern, i, '}').? + 1;
                const T = info.field_types[arg];
                converted[arg] = (@typeInfo(T) == .float or T == comptime_float) and
                    std.mem.eql(u8, pattern[i..end], "{d}");
                result = result ++ if (converted[arg]) "{s}" else pattern[i..end];
                arg += 1;
                i = end;
            } else {
                const count: usize = if (i + 1 < pattern.len and
                    (pattern[i] == '{' or pattern[i] == '}') and pattern[i] == pattern[i + 1]) 2 else 1;
                result = result ++ pattern[i..][0..count];
                i += count;
            }
        }
        break :blk .{ .pattern = result, .converted = converted };
    };
    var mapped: Arguments(@TypeOf(args), spec.converted) = undefined;
    inline for (info.field_names, spec.converted) |name, change| {
        @field(mapped, name) = if (change) builtins.string(@field(args, name)) else @field(args, name);
    }
    return std.fmt.allocPrint(ui.frameAllocator(), spec.pattern, mapped) catch @panic("TSX format");
}
