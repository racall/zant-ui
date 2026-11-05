const std = @import("std");
const app_mod = @import("zant").app;
const platform = @import("zant").platform;

pub fn main(init: std.process.Init) !void {
    const allocator = init.gpa;
    const app = try app_mod.App.create(allocator);
    defer app.destroy();

    const host = app_mod.makeHost(app);
    const window = try platform.createWindow("Zant UI — Component Gallery", .{ .width = 900, .height = 640 }, &host);
    app.window = window;
    defer platform.destroy(window);

    std.debug.print("Zant UI running — renderer: {s}\n", .{platform.rendererName(window)});
    if (init.environ_map.get("ZANT_SNAPSHOT")) |path| {
        const snapshot_path = try allocator.allocSentinel(u8, path.len, 0);
        defer allocator.free(snapshot_path);
        @memcpy(snapshot_path[0..path.len], path);
        platform.scheduleSnapshot(window, snapshot_path, 600);
    }
    platform.run(window);
}
