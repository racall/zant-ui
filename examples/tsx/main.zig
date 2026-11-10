//! Window for the TSX counter (`examples/tsx/counter.tsx`, compiled to Zig
//! by `zant-tsx` during the build). Follows the system's light / dark
//! appearance; ZANT_DARK=1 forces dark mode.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Counter = @import("counter_tsx").Counter;

/// The app state: the TSX component plus the theme hook the runtime asks
/// before every rebuild (it rebuilds when the system appearance changes).
const App = struct {
    counter: Counter = .{},
    force_dark: bool = false,

    pub fn theme(self: *App) zant.tokens.Theme {
        const dark = self.force_dark or zant.desktop.systemDarkMode();
        return if (dark) zant.tokens.antDark() else zant.tokens.antLight();
    }

    fn render(self: *App) ui.Node {
        return self.counter.render();
    }
};

pub fn main(init: std.process.Init) !void {
    try zant.launch.run(init.gpa, App{ .force_dark = init.environ_map.get("ZANT_DARK") != null }, App.render, .{
        .title = "TSX Counter",
        .window = .{ .width = 520, .height = 480, .min_width = 420, .min_height = 420 },
    });
}
