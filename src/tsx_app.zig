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
const events = @import("core/events.zig");

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

        /// `useCloseRequest`: false keeps the window open.
        pub fn onCloseRequest(self: *Self) bool {
            return if (@hasDecl(Root, "onCloseRequest")) self.root.onCloseRequest() else true;
        }

        /// `useQuitRequest`, else the close request (⌘Q asks it too).
        pub fn onQuitRequest(self: *Self) bool {
            if (@hasDecl(Root, "onQuitRequest")) return self.root.onQuitRequest();
            return self.onCloseRequest();
        }

        /// `useKeyDown`: true when the app handled the key.
        pub fn onKey(self: *Self, key: events.KeyEvent) bool {
            return if (@hasDecl(Root, "onKey")) self.root.onKey(keyEvent(key)) else false;
        }

        /// Light or dark with the system, before every build.
        pub fn theme(self: *Self) tokens.Theme {
            return self.forced orelse systemTheme();
        }
    };
}

/// A key press as TSX sees it (`useKeyDown((e) => …)`): `key` is "Escape",
/// "Enter", "Tab", "Backspace", "Delete", "ArrowUp" … (like the DOM's
/// `KeyboardEvent.key`), "F1" … "F12", or the character typed ("f", "/").
/// `mod` is the platform's command key: ⌘ on macOS, Ctrl on Windows.
pub const KeyEvent = struct {
    key: []const u8,
    mod: bool,
    shift: bool,
    alt: bool,
    ctrl: bool,
    meta: bool,
    repeat: bool,
};

pub fn keyEvent(e: events.KeyEvent) KeyEvent {
    return .{
        .key = keyName(e.key),
        .mod = e.modifiers.primary(),
        .shift = e.modifiers.shift,
        .alt = e.modifiers.alt,
        .ctrl = e.modifiers.control,
        .meta = e.modifiers.meta,
        .repeat = e.repeat,
    };
}

fn keyName(key: events.Key) []const u8 {
    return switch (key) {
        .character => |c| blk: {
            var buf: [4]u8 = undefined;
            const n = std.unicode.utf8Encode(c, &buf) catch break :blk "";
            break :blk ui.frameString(buf[0..n]);
        },
        .left => "ArrowLeft",
        .right => "ArrowRight",
        .up => "ArrowUp",
        .down => "ArrowDown",
        .home => "Home",
        .end => "End",
        .backspace => "Backspace",
        .delete => "Delete",
        .escape => "Escape",
        .tab => "Tab",
        .enter => "Enter",
        .page_up => "PageUp",
        .page_down => "PageDown",
        .function => |n| std.fmt.allocPrint(ui.frameAllocator(), "F{d}", .{n}) catch "F",
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

test "key events read like the DOM's" {
    ui.beginFrame();
    defer ui.endFrame();
    const esc = keyEvent(.{ .key = .escape });
    try std.testing.expectEqualStrings("Escape", esc.key);
    const f = keyEvent(.{ .key = .{ .character = 'f' }, .modifiers = .{ .shift = true } });
    try std.testing.expectEqualStrings("f", f.key);
    try std.testing.expect(f.shift and !f.alt);
    try std.testing.expectEqualStrings("F5", keyEvent(.{ .key = .{ .function = 5 } }).key);
}
