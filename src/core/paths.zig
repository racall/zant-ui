//! Per-user locations for app data, following each platform's convention.
const std = @import("std");
const builtin = @import("builtin");

/// Directory for an app's settings and databases (not created):
///
/// - macOS: `~/Library/Application Support/<app_id>`
/// - Windows: `%APPDATA%\<app_id>` (roaming profile)
/// - elsewhere: `$XDG_DATA_HOME/<app_id>` or `~/.local/share/<app_id>`
///
/// `app_id` is usually the bundle id (`com.example.app`). Returns null when
/// the environment has no home / profile directory.
pub fn appDataDir(allocator: std.mem.Allocator, environ: *const std.process.Environ.Map, app_id: []const u8) !?[]u8 {
    switch (builtin.os.tag) {
        .macos => {
            const home = environ.get("HOME") orelse return null;
            return try std.fs.path.join(allocator, &.{ home, "Library", "Application Support", app_id });
        },
        .windows => {
            if (environ.get("APPDATA")) |appdata| return try std.fs.path.join(allocator, &.{ appdata, app_id });
            const profile = environ.get("USERPROFILE") orelse return null;
            return try std.fs.path.join(allocator, &.{ profile, "AppData", "Roaming", app_id });
        },
        else => {
            if (environ.get("XDG_DATA_HOME")) |data| return try std.fs.path.join(allocator, &.{ data, app_id });
            const home = environ.get("HOME") orelse return null;
            return try std.fs.path.join(allocator, &.{ home, ".local", "share", app_id });
        },
    }
}

test "app data dir uses the platform convention" {
    var env = std.process.Environ.Map.init(std.testing.allocator);
    defer env.deinit();
    try env.put("HOME", "/Users/me");
    try env.put("APPDATA", "C:\\Users\\me\\AppData\\Roaming");
    const dir = (try appDataDir(std.testing.allocator, &env, "com.example.app")).?;
    defer std.testing.allocator.free(dir);
    switch (builtin.os.tag) {
        .macos => try std.testing.expectEqualStrings("/Users/me/Library/Application Support/com.example.app", dir),
        .windows => try std.testing.expectEqualStrings("C:\\Users\\me\\AppData\\Roaming\\com.example.app", dir),
        else => {},
    }
}
