//! `createWindow(Panel, { … })` in TSX: another window of the app rendering
//! the component `Comp` (a component without props). The window opens the
//! first time it is used (`show`, `toggle`, …) and keeps its component's
//! state, like any instance, in its own runtime. Hooks inside the component
//! (`useKeyDown`, `useCloseRequest`, `useWindowFocus`) become this window's.
const std = @import("std");
const ui = @import("ui.zig");
const desktop = @import("desktop.zig");
const events = @import("core/events.zig");
const tokens = @import("theme/tokens.zig");
const app = @import("tsx_app.zig");

/// Where `place` puts a window (TSX: "center", "topLeft", …, "trayCorner").
pub const Place = desktop.Place;

pub fn Window(comptime Comp: type) type {
    return struct {
        const Self = @This();
        options: desktop.OpenOptions,
        handle: ?desktop.Window = null,
        /// The component built last: its hooks answer this window's events
        /// (frame memory, valid until the window's next build).
        last: ?*Comp = null,

        /// The window, opened (hidden) on first use; null if it can't open.
        fn window(self: *Self) ?desktop.Window {
            if (self.handle) |h| return h;
            var options = self.options;
            options.window.hidden = true;
            self.handle = desktop.openWindow(std.heap.c_allocator, self, .{
                .render = render,
                .key = keyHook,
                .focus = focusHook,
                .close = closeHook,
            }, options) catch |err| blk: {
                std.log.warn("zant: window \"{s}\": {s}", .{ options.title, @errorName(err) });
                break :blk null;
            };
            return self.handle;
        }

        pub fn show(self: *Self) void {
            if (self.window()) |w| w.show();
        }

        pub fn hide(self: *Self) void {
            if (self.handle) |w| w.hide();
        }

        /// Shows the window and gives it the keyboard.
        pub fn activate(self: *Self) void {
            if (self.window()) |w| w.activate();
        }

        /// Hidden → shown and focused; shown → hidden.
        pub fn toggle(self: *Self) void {
            if (self.visible()) self.hide() else self.activate();
        }

        pub fn visible(self: *Self) bool {
            const w = self.handle orelse return false;
            return w.isVisible();
        }

        /// Moves it to a spot of the screen under the pointer.
        pub fn place(self: *Self, where: Place, margin: f32) void {
            if (self.window()) |w| w.place(where, margin);
        }

        /// Focuses the node with `key` after the window's next layout.
        pub fn focus(self: *Self, node_key: []const u8) void {
            if (self.window()) |w| w.focus(node_key);
        }

        pub fn theme(_: *Self) tokens.Theme {
            return app.currentTheme();
        }

        fn render(self: *Self) ui.Node {
            var value: Comp = undefined;
            value.props = .{};
            const id: u64 = 0;
            if (@hasField(Comp, "_id")) value._id = id;
            if (@hasField(Comp, "_s")) value._s = ui.localState(Comp._Local, id, .{});
            const instance = ui.frameAllocator().create(Comp) catch @panic("Zant UI: out of memory");
            instance.* = value;
            self.last = instance;
            return instance.render();
        }

        fn keyHook(self: *Self, e: events.KeyEvent) bool {
            if (!@hasDecl(Comp, "onKey")) return false;
            const instance = self.last orelse return false;
            return instance.onKey(app.keyEvent(e));
        }

        fn focusHook(self: *Self, focused: bool) bool {
            if (!@hasDecl(Comp, "onFocus")) return false;
            const instance = self.last orelse return false;
            instance.onFocus(focused);
            return true;
        }

        fn closeHook(self: *Self) bool {
            if (!@hasDecl(Comp, "onCloseRequest")) return true;
            const instance = self.last orelse return true;
            return instance.onCloseRequest();
        }
    };
}
