//! The desktop functions TSX imports from "zant" (docs/tsx.md, "桌面函数"):
//!
//!     import { clipboard, app, mainWindow, wakeAfter, keyLabel, isMacos } from "zant";
//!     clipboard.setText(entry.text);   app.quit();   mainWindow.show();
//!
//! TS-friendly wrappers of `zant.desktop`, `zant.paths` and `ui`: strings come
//! back in the frame arena (valid until the next build, copied when stored
//! in state), failures read as "" / null / false.
const std = @import("std");
const ui = @import("ui.zig");
const desktop = @import("desktop.zig");
const paths = @import("core/paths.zig");

pub const is_macos = desktop.is_macos;
pub const is_windows = @import("builtin").os.tag == .windows;

/// The app's bundle id and environment, set by `zant.tsx.app.run`.
pub var app_id: []const u8 = "";
pub var environ: ?*const std.process.Environ.Map = null;

fn frame() std.mem.Allocator {
    return ui.frameAllocator();
}

pub const clipboard = struct {
    /// The text on the clipboard ("" when it holds none).
    pub fn text() []const u8 {
        return desktop.clipboardText(frame()) catch "";
    }

    pub fn setText(value: []const u8) bool {
        return desktop.setClipboardText(value);
    }

    /// PNG bytes, null when the clipboard holds no image.
    pub fn image() ?[]const u8 {
        return desktop.clipboardImage(frame()) catch null;
    }

    pub fn setImage(png: []const u8) bool {
        return desktop.setClipboardImage(png);
    }

    /// Copied files and folders: absolute paths, one per line; null for none.
    pub fn files() ?[]const u8 {
        return desktop.clipboardFiles(frame()) catch null;
    }

    pub fn setFiles(lines: []const u8) bool {
        return desktop.setClipboardFiles(lines);
    }

    /// Grows with every change (also the app's own writes).
    pub fn changeCount() f64 {
        return @floatFromInt(desktop.clipboardChangeCount());
    }
};

pub const app = struct {
    pub fn quit() void {
        desktop.quit();
    }

    /// "off", "on" or "needsApproval" (macOS: waiting in System Settings).
    pub fn launchAtLogin() []const u8 {
        return switch (desktop.launchAtLogin(app_id)) {
            .off => "off",
            .on => "on",
            .needs_approval => "needsApproval",
        };
    }

    /// False when the system refused (on macOS a `zant run` build cannot).
    pub fn setLaunchAtLogin(on: bool) bool {
        desktop.setLaunchAtLogin(app_id, on) catch return false;
        return true;
    }

    pub fn launchedAtLogin() bool {
        return desktop.launchedAtLogin();
    }

    pub fn deviceName() []const u8 {
        var buf: [256]u8 = undefined;
        return ui.frameString(desktop.deviceName(&buf));
    }

    pub fn downloadsDir() ?[]const u8 {
        return desktop.downloadsDir(frame()) catch null;
    }

    /// The app's own data folder (`zant.paths.appDataDir` with the bundle id).
    pub fn dataDir() ?[]const u8 {
        const env = environ orelse return null;
        return paths.appDataDir(frame(), env, app_id) catch null;
    }
};

/// The app's main window (the root component's).
pub const main_window = struct {
    pub fn show() void {
        if (desktop.mainWindow()) |w| w.show();
    }

    pub fn hide() void {
        if (desktop.mainWindow()) |w| w.hide();
    }

    pub fn activate() void {
        if (desktop.mainWindow()) |w| w.activate();
    }

    pub fn focus(key: []const u8) void {
        if (desktop.mainWindow()) |w| w.focus(key);
    }

    pub fn visible() bool {
        const w = desktop.mainWindow() orelse return false;
        return w.isVisible();
    }

    pub fn active() bool {
        const w = desktop.mainWindow() orelse return false;
        return w.isVisible() and w.isActive();
    }

    pub fn minimized() bool {
        const w = desktop.mainWindow() orelse return false;
        return w.isMinimized();
    }
};

/// A shortcut string ("ctrl+cmd+v", what `useHotkey` takes) as the platform
/// shows it ("⌃⌘V" / "Ctrl+Alt+V"); "" when it is not a valid shortcut.
pub fn shortcutLabel(shortcut: []const u8) []const u8 {
    const parsed = desktop.Shortcut.parse(shortcut) orelse return "";
    var buf: [64]u8 = undefined;
    return ui.frameString(parsed.label(&buf));
}

/// Rebuilds after `ms` milliseconds (call it while rendering: a toast that
/// hides itself reads `performance.now()` against its deadline).
pub fn wakeAfter(ms: f64) void {
    if (!(ms > 0)) return ui.wakeAfter(0);
    ui.wakeAfter(@intFromFloat(@min(ms, @as(f64, std.math.maxInt(u32)))));
}

test "wakeAfter clamps what TS passes" {
    _ = ui.takeWake();
    wakeAfter(-5);
    try std.testing.expectEqual(@as(?u32, 0), ui.takeWake());
    wakeAfter(1e12);
    try std.testing.expectEqual(@as(?u32, std.math.maxInt(u32)), ui.takeWake());
    wakeAfter(250.7);
    try std.testing.expectEqual(@as(?u32, 250), ui.takeWake());
}

test "the data folder needs the environment" {
    environ = null;
    try std.testing.expect(app.dataDir() == null);
}
