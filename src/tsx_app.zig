//! The entry of a TS + TSX app (`zant create`, language "ts"): the generated
//! `main.zig` calls `run` with the root component (the default export of
//! `App.tsx`) and the app's config (`app_config.zig`, generated from
//! `app.config.json` by `zant sync`).
//!
//! The root component's struct holds the app's state; `run` wraps it so the
//! runtime's hooks (`zant.launch.run`) reach it: the theme follows the
//! system unless `ZANT_THEME=light|dark` is set (screenshots, tests).
const std = @import("std");
const ui = @import("ui.zig");
const tokens = @import("theme/tokens.zig");
const launch = @import("launch.zig");
const desktop = @import("desktop.zig");

/// Runs the app until its last window closes.
pub fn run(init: std.process.Init, comptime Root: type, comptime config: type) !void {
    if (@hasDecl(config, "single_instance") and config.single_instance) {
        // already running (maybe hidden in the tray): it comes forward instead
        if (!desktop.singleInstance(config.bundle_id)) return;
    }
    const App = Shell(Root);
    const forced = forcedTheme(init.environ_map);
    try launch.run(init.gpa, App{ .forced = forced }, App.render, .{
        .title = if (@hasDecl(config, "title")) config.title else "Zant UI",
        .window = if (@hasDecl(config, "window")) config.window else .{},
        .theme = forced orelse systemTheme(),
    });
}

/// The root component as the runtime's state type.
pub fn Shell(comptime Root: type) type {
    return struct {
        const Self = @This();
        root: Root = .{},
        forced: ?tokens.Theme = null,

        pub fn render(self: *Self) ui.Node {
            return self.root.render();
        }

        /// Light or dark with the system, before every build.
        pub fn theme(self: *Self) tokens.Theme {
            return self.forced orelse systemTheme();
        }
    };
}

pub fn systemTheme() tokens.Theme {
    return if (desktop.systemDarkMode()) tokens.antDark() else tokens.antLight();
}

/// `ZANT_THEME=light|dark` fixes the theme.
fn forcedTheme(environ: *const std.process.Environ.Map) ?tokens.Theme {
    const value = environ.get("ZANT_THEME") orelse return null;
    if (std.mem.eql(u8, value, "dark")) return tokens.antDark();
    if (std.mem.eql(u8, value, "light")) return tokens.antLight();
    return null;
}

test "the shell renders its root and follows a forced theme" {
    const Root = struct {
        n: u32 = 0,
        pub fn render(self: *@This()) ui.Node {
            self.n += 1;
            return ui.column(.{});
        }
    };
    ui.beginFrame();
    defer ui.endFrame();
    var shell = Shell(Root){ .forced = tokens.antDark() };
    _ = shell.render();
    try std.testing.expectEqual(@as(u32, 1), shell.root.n);
    try std.testing.expectEqual(tokens.antDark().color_bg_base, shell.theme().color_bg_base);
}
