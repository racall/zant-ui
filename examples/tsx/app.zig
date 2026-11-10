//! Runs a TSX component as a whole app: the component is the state, and the
//! window follows the system's light / dark appearance. ZANT_THEME=light or
//! ZANT_THEME=dark (or ZANT_DARK=1) forces one, for screenshots. Used by the
//! TSX examples' `main` files.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

/// The app state around a root TSX component: the component plus the theme
/// hook the runtime asks before every rebuild (it rebuilds when the system
/// appearance changes).
pub fn App(comptime Root: type) type {
    return struct {
        const Self = @This();

        root: Root = .{},
        forced: ?Appearance = null,

        pub const Appearance = enum { light, dark };

        pub fn theme(self: *Self) zant.tokens.Theme {
            const dark = if (self.forced) |f| f == .dark else zant.desktop.systemDarkMode();
            return if (dark) zant.tokens.antDark() else zant.tokens.antLight();
        }

        fn render(self: *Self) ui.Node {
            return self.root.render();
        }
    };
}

pub fn run(comptime Root: type, init: std.process.Init, options: zant.launch.Options) !void {
    const State = App(Root);
    var state = State{};
    if (init.environ_map.get("ZANT_THEME")) |v| state.forced = std.meta.stringToEnum(State.Appearance, v);
    if (init.environ_map.get("ZANT_DARK") != null) state.forced = .dark;
    try zant.launch.run(init.gpa, state, State.render, options);
}
