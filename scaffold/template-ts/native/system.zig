//! native/system.ts 的实现：类型来自规范（`@import("spec").system`）。
const std = @import("std");
const builtin = @import("builtin");
const spec = @import("spec").system;

var cpus: ?u32 = null;

pub fn info() spec.SystemInfo {
    // 每帧都会调用：只在第一次读取 CPU 核数
    if (cpus == null) cpus = @intCast(std.Thread.getCpuCount() catch 1);
    return .{
        .os = switch (builtin.os.tag) {
            .macos => .macos,
            .windows => .windows,
            else => .other,
        },
        .cpus = cpus.?,
        .zig = builtin.zig_version_string,
    };
}

test "info reports at least one CPU" {
    try std.testing.expect(info().cpus >= 1);
}
