//! Build-time icon tool (run by `configureApp` in Zant UI's build.zig):
//!   zant-icon ico <app-windows.png> <out.ico>   Windows app .ico (16–256)
const std = @import("std");
const png = @import("png.zig");

pub fn main(init: std.process.Init) !void {
    const gpa = init.gpa;
    const io = init.io;
    const args = try init.minimal.args.toSlice(init.arena.allocator());
    if (args.len >= 4 and std.mem.eql(u8, args[1], "ico")) {
        const data = try std.Io.Dir.cwd().readFileAlloc(io, args[2], gpa, .limited(64 << 20));
        defer gpa.free(data);
        const img = png.decode(gpa, data) catch |err| {
            std.debug.print("zant-icon: {s}: 不能读取（{s}）；需要 8 位、非隔行扫描的 PNG\n", .{ args[2], @errorName(err) });
            std.process.exit(1);
        };
        defer img.deinit(gpa);
        if (img.width != img.height or img.width < 256) {
            std.debug.print("zant-icon: {s} 是 {d}×{d}；Windows 应用图标需要正方形、至少 256×256\n", .{ args[2], img.width, img.height });
            std.process.exit(1);
        }
        const ico = try png.buildIco(gpa, img);
        defer gpa.free(ico);
        try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = args[3], .data = ico });
        return;
    }
    std.debug.print("用法: zant-icon ico <in.png> <out.ico>\n", .{});
    std.process.exit(2);
}

test {
    _ = png;
}
