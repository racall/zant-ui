//! Desktop integration for apps that live beside the user's other work:
//! extra windows (a quick-access panel), the tray icon (Windows notification
//! area) / status bar item (macOS menu bar), system-wide hotkeys, hiding to
//! the tray, quitting, and noticing clipboard changes.
//!
//!     // a panel window sharing the app state (opened once, shown on demand)
//!     self.panel = try zant.desktop.openWindow(gpa, self, .{ .render = State.renderPanel }, .{
//!         .title = "History",
//!         .window = .{ .style = .panel, .width = 360, .height = 480, .hidden = true },
//!     });
//!     self.panel.?.place(.tray_corner, 12);
//!     self.panel.?.activate();
//!
//!     try zant.desktop.setTray(.{
//!         .icon = if (zant.keys.is_macos) @embedFile("icon_statusbar") else @embedFile("icon_tray"),
//!         .template = zant.keys.is_macos,
//!         .tooltip = "My App",
//!         .items = &.{ .{ .id = 1, .title = "Open" }, .{ .separator = true }, .{ .id = 2, .title = "Quit" } },
//!     });
//!     _ = zant.desktop.registerHotkey(1, .{ .key = .{ .character = 'v' }, .modifiers = .{ .meta = true, .shift = true } });
//!
//! Tray clicks, menu items, hotkeys and Dock clicks reach the state's
//! `onAppEvent(self, zant.desktop.AppEvent) bool` (see `zant.launch.run`).
const std = @import("std");
const builtin = @import("builtin");
const platform = @import("platform/platform.zig");
const events = @import("core/events.zig");
const png = @import("core/png.zig");
const ui = @import("ui.zig");
const tokens = @import("theme/tokens.zig");
const Runtime = @import("ui_runtime.zig").Runtime;

pub const Place = platform.Place;
pub const is_macos = builtin.os.tag == .macos;

// ---------------------------------------------------------------- windows

/// A window of the app: the main window (`mainWindow`) or one opened with
/// `openWindow`.
pub const Window = struct {
    runtime: *Runtime,

    /// Shows the window without taking the keyboard.
    pub fn show(self: Window) void {
        platform.setVisible(self.runtime.window, true);
    }

    /// Hides the window; it keeps its state and can be shown again.
    pub fn hide(self: Window) void {
        platform.setVisible(self.runtime.window, false);
    }

    /// Shows the window, brings it forward and gives it the keyboard (a
    /// `.panel` on macOS does this without bringing the app forward).
    pub fn activate(self: Window) void {
        self.runtime.invalidate();
        platform.activate(self.runtime.window);
    }

    pub fn isVisible(self: Window) bool {
        return platform.isVisible(self.runtime.window);
    }

    /// Has the keyboard focus.
    pub fn isActive(self: Window) bool {
        return platform.isActive(self.runtime.window);
    }

    /// Moves the window to a spot of the work area of the screen under the
    /// pointer, `margin` points from its edges.
    pub fn place(self: Window, where: Place, margin: f32) void {
        platform.place(self.runtime.window, where, margin);
    }

    /// Closes a window from `openWindow` (not the main window: quit instead).
    pub fn close(self: Window) void {
        if (main_runtime == self.runtime) return;
        self.runtime.closeWindow();
    }
};

var main_runtime: ?*Runtime = null;

/// Set by `zant.launch.run` (the window `run` created).
pub fn setMainRuntime(runtime: ?*Runtime) void {
    main_runtime = runtime;
}

/// The app's main window (null before `zant.launch.run` created it).
pub fn mainWindow() ?Window {
    const runtime = main_runtime orelse return null;
    if (runtime.window == null) return null;
    return .{ .runtime = runtime };
}

/// Hooks of a window opened with `openWindow`; `render` is required.
pub fn WindowHooks(comptime State: type) type {
    return struct {
        render: *const fn (*State) ui.Node,
        /// Keys before the focused widget (Esc to hide a panel...).
        key: ?*const fn (*State, events.KeyEvent) bool = null,
        /// The window gained (true) / lost (false) the keyboard; return true
        /// when state changed. A panel typically hides when it loses focus.
        focus: ?*const fn (*State, bool) bool = null,
        /// Close requested (system menu, Alt+F4); return false to keep it.
        close: ?*const fn (*State) bool = null,
    };
}

pub const OpenOptions = struct {
    title: [:0]const u8 = "Zant UI",
    window: platform.WindowOptions = .{},
    /// Used when the state has no `theme()` hook.
    theme: tokens.Theme = .{},
};

/// Opens another window of the running app that renders the same state
/// (`state` must outlive it). Call it on the UI thread, e.g. from an action
/// or the first `render`; with `.window.hidden = true` it opens hidden.
pub fn openWindow(allocator: std.mem.Allocator, state: anytype, comptime hooks: WindowHooks(@typeInfo(@TypeOf(state)).pointer.child), options: OpenOptions) !Window {
    const State = @typeInfo(@TypeOf(state)).pointer.child;
    const Adapter = struct {
        fn render(ctx: *anyopaque) ui.Node {
            return hooks.render(@ptrCast(@alignCast(ctx)));
        }
        fn theme(ctx: *anyopaque) tokens.Theme {
            const value: *State = @ptrCast(@alignCast(ctx));
            return value.theme();
        }
        fn key(ctx: *anyopaque, event: events.KeyEvent) bool {
            return hooks.key.?(@ptrCast(@alignCast(ctx)), event);
        }
        fn focus(ctx: *anyopaque, focused: bool) bool {
            return hooks.focus.?(@ptrCast(@alignCast(ctx)), focused);
        }
        fn close(ctx: *anyopaque) bool {
            return hooks.close.?(@ptrCast(@alignCast(ctx)));
        }
    };
    const runtime = try Runtime.init(allocator, state, Adapter.render, .{
        .title = options.title,
        .window = options.window,
        .theme = options.theme,
        .theme_fn = if (@hasDecl(State, "theme")) Adapter.theme else null,
        .key_fn = if (hooks.key != null) Adapter.key else null,
        .focus_fn = if (hooks.focus != null) Adapter.focus else null,
        .close_fn = if (hooks.close != null) Adapter.close else null,
    });
    errdefer runtime.deinit();
    try runtime.openWindow();
    return .{ .runtime = runtime };
}

/// Rebuilds all windows (after changing state outside an action, e.g. in a
/// timer or after a background result; `ui.postWake` does it from threads).
pub fn invalidateAll() void {
    Runtime.invalidateAll();
}

// ---------------------------------------------------------------- app

/// Ends the app now (all windows), without asking `onCloseRequest` again.
pub fn quit() void {
    platform.quit();
}

/// macOS: show the Dock icon and menu bar (a regular app) or neither (a
/// status bar app, e.g. while only the menu bar item is left). Windows:
/// no-op; the taskbar shows visible windows.
pub fn setDockVisible(visible: bool) void {
    platform.setDockVisible(visible);
}

/// Increments whenever any app writes to the clipboard: poll it (cheap) to
/// notice copies, then read the text with `ui.clipboardText`.
pub fn clipboardChangeCount() u64 {
    return platform.clipboardChangeCount();
}

pub const AppEvent = union(enum) {
    /// Windows: left click on the tray icon (the menu opens on right click).
    tray_click,
    /// A tray / status bar menu item, by id.
    tray_menu: u32,
    /// A registered hotkey, by id.
    hotkey: u32,
    /// macOS: the Dock icon was clicked (show the hidden main window).
    reopen,
};

var app_handler_ctx: ?*anyopaque = null;
var app_handler_fn: ?*const fn (*anyopaque, AppEvent) bool = null;

fn appCallback(_: ?*anyopaque, raw: *const platform.AppEvent) callconv(.c) void {
    const handler = app_handler_fn orelse return;
    const ctx = app_handler_ctx orelse return;
    const event: AppEvent = switch (raw.type) {
        platform.AppEventType.tray_click => .tray_click,
        platform.AppEventType.tray_menu => .{ .tray_menu = @intCast(@max(raw.id, 0)) },
        platform.AppEventType.hotkey => .{ .hotkey = @intCast(@max(raw.id, 0)) },
        platform.AppEventType.reopen => .reopen,
        else => return,
    };
    if (handler(ctx, event)) Runtime.invalidateAll();
}

/// Routes app events to `handler(state, event)`; it returns true when state
/// changed (all windows rebuild). `zant.launch.run` installs the state's
/// `onAppEvent` hook automatically.
pub fn setAppEventHandler(state: anytype, comptime handler: fn (@TypeOf(state), AppEvent) bool) void {
    const Ptr = @TypeOf(state);
    const Adapter = struct {
        fn call(ctx: *anyopaque, event: AppEvent) bool {
            return handler(@as(Ptr, @ptrCast(@alignCast(ctx))), event);
        }
    };
    app_handler_ctx = @ptrCast(state);
    app_handler_fn = Adapter.call;
    platform.setAppHandler(appCallback, null);
}

// ---------------------------------------------------------------- tray

pub const TrayItem = struct {
    id: u32 = 0,
    title: []const u8 = "",
    checked: bool = false,
    disabled: bool = false,
    separator: bool = false,
};

pub const TrayOptions = struct {
    /// PNG (8-bit, non-interlaced), square; resized to the system size.
    /// The scaffold's `@embedFile("icon_statusbar")` (macOS) and
    /// `@embedFile("icon_tray")` (Windows) follow each platform's guidelines.
    icon: []const u8,
    /// macOS: a template image (black + alpha) the menu bar tints.
    template: bool = false,
    tooltip: []const u8 = "",
    /// The menu (macOS: opens on click; Windows: on right click, a left
    /// click sends `.tray_click`).
    items: []const TrayItem = &.{},
};

/// Adds or updates the tray icon / status bar item and its menu.
pub fn setTray(options: TrayOptions) !void {
    const gpa = std.heap.c_allocator;
    var arena_state = std.heap.ArenaAllocator.init(gpa);
    defer arena_state.deinit();
    const arena = arena_state.allocator();

    const decoded = try png.decode(arena, options.icon);
    // macOS: 22 pt tall at @2x; Windows: the small-icon size for the DPI
    const px = platform.trayIconPixels();
    const height = px;
    const width = @max(1, @as(u32, @intFromFloat(@round(@as(f32, @floatFromInt(decoded.width)) * @as(f32, @floatFromInt(px)) / @as(f32, @floatFromInt(decoded.height))))));
    const icon = if (decoded.width == width and decoded.height == height) decoded else try png.resize(arena, decoded, width, height);
    const scale: f32 = if (is_macos) 2 else 1;

    const items = try arena.alloc(platform.TrayItem, options.items.len);
    for (options.items, items) |item, *out| {
        out.* = .{
            .title = (try arena.dupeSentinel(u8, item.title, 0)).ptr,
            .id = @intCast(item.id),
            .separator = @intFromBool(item.separator),
            .checked = @intFromBool(item.checked),
            .disabled = @intFromBool(item.disabled),
        };
    }
    const tooltip = try arena.dupeSentinel(u8, options.tooltip, 0);
    if (!platform.setTray(icon.rgba, icon.width, icon.height, scale, options.template, tooltip.ptr, items))
        return error.TrayUnavailable;
}

pub fn removeTray() void {
    platform.removeTray();
}

// ---------------------------------------------------------------- hotkeys

/// A key combination, e.g. ⌥⌘V / Ctrl+Alt+V.
pub const Shortcut = struct {
    key: events.Key,
    modifiers: events.Modifiers = .{},

    /// Usable as a system-wide hotkey: a supported key plus at least one of
    /// Ctrl / Alt (⌥) / ⌘ (Win).
    pub fn isValid(self: Shortcut) bool {
        const m = self.modifiers;
        return (m.control or m.alt or m.meta) and platformKey(self.key) != null;
    }

    /// From a key press in a shortcut recorder; null for modifier-only or
    /// unsupported keys.
    pub fn fromKeyEvent(event: events.KeyEvent) ?Shortcut {
        var mods = event.modifiers;
        mods.caps_lock = false;
        const s: Shortcut = .{ .key = normalized(event.key), .modifiers = mods };
        return if (s.isValid()) s else null;
    }

    pub fn eql(a: Shortcut, b: Shortcut) bool {
        const ma = a.modifiers;
        const mb = b.modifiers;
        return std.meta.eql(normalized(a.key), normalized(b.key)) and ma.shift == mb.shift and
            ma.control == mb.control and ma.alt == mb.alt and ma.meta == mb.meta;
    }

    /// Display label for this platform: "⌥⌘V" on macOS, "Ctrl+Alt+V" on Windows.
    pub fn label(self: Shortcut, buf: []u8) []const u8 {
        var w: std.Io.Writer = .fixed(buf);
        const m = self.modifiers;
        if (is_macos) {
            if (m.control) w.writeAll("⌃") catch {};
            if (m.alt) w.writeAll("⌥") catch {};
            if (m.shift) w.writeAll("⇧") catch {};
            if (m.meta) w.writeAll("⌘") catch {};
        } else {
            if (m.control) w.writeAll("Ctrl+") catch {};
            if (m.alt) w.writeAll("Alt+") catch {};
            if (m.shift) w.writeAll("Shift+") catch {};
            if (m.meta) w.writeAll("Win+") catch {};
        }
        writeKeyName(&w, self.key) catch {};
        return w.buffered();
    }

    /// Text form for settings files: "ctrl+alt+shift+meta+v" (any subset).
    pub fn format(self: Shortcut, buf: []u8) []const u8 {
        var w: std.Io.Writer = .fixed(buf);
        const m = self.modifiers;
        if (m.control) w.writeAll("ctrl+") catch {};
        if (m.alt) w.writeAll("alt+") catch {};
        if (m.shift) w.writeAll("shift+") catch {};
        if (m.meta) w.writeAll("meta+") catch {};
        switch (self.key) {
            .character => |c| {
                if (c == ' ') w.writeAll("space") catch {} else w.print("{u}", .{c}) catch {};
            },
            .function => |n| w.print("f{d}", .{n}) catch {},
            else => w.writeAll(@tagName(self.key)) catch {},
        }
        return w.buffered();
    }

    /// Inverse of `format`; null when malformed.
    pub fn parse(text: []const u8) ?Shortcut {
        var s: Shortcut = .{ .key = .{ .character = 0 } };
        var parts = std.mem.splitScalar(u8, text, '+');
        var key_part: ?[]const u8 = null;
        while (parts.next()) |part| {
            if (key_part != null) return null; // modifiers come first
            if (std.mem.eql(u8, part, "ctrl")) s.modifiers.control = true else if (std.mem.eql(u8, part, "alt")) s.modifiers.alt = true else if (std.mem.eql(u8, part, "shift")) s.modifiers.shift = true else if (std.mem.eql(u8, part, "meta")) s.modifiers.meta = true else key_part = part;
        }
        const k = key_part orelse return null;
        if (std.mem.eql(u8, k, "space")) {
            s.key = .{ .character = ' ' };
        } else if (k.len >= 2 and k[0] == 'f' and std.ascii.isDigit(k[1])) {
            const n = std.fmt.parseInt(u8, k[1..], 10) catch return null;
            if (n < 1 or n > 12) return null;
            s.key = .{ .function = n };
        } else if (std.unicode.utf8CountCodepoints(k) catch 0 == 1) {
            s.key = .{ .character = std.unicode.utf8Decode(k) catch return null };
        } else {
            inline for (.{ "left", "right", "up", "down", "enter", "tab", "escape" }) |name| {
                if (std.mem.eql(u8, k, name)) s.key = @field(events.Key, name);
            }
        }
        return if (s.isValid()) s else null;
    }
};

fn normalized(key: events.Key) events.Key {
    return switch (key) {
        .character => |c| .{ .character = if (c >= 'A' and c <= 'Z') c + 32 else c },
        else => key,
    };
}

fn writeKeyName(w: *std.Io.Writer, key: events.Key) !void {
    switch (key) {
        .character => |c| {
            if (c == ' ') return w.writeAll("Space");
            const upper: u21 = if (c >= 'a' and c <= 'z') c - 32 else c;
            try w.print("{u}", .{upper});
        },
        .function => |n| try w.print("F{d}", .{n}),
        .enter => try w.writeAll(if (is_macos) "↵" else "Enter"),
        .tab => try w.writeAll(if (is_macos) "⇥" else "Tab"),
        .escape => try w.writeAll("Esc"),
        .left => try w.writeAll("←"),
        .right => try w.writeAll("→"),
        .up => try w.writeAll("↑"),
        .down => try w.writeAll("↓"),
        else => try w.writeAll(@tagName(key)),
    }
}

fn platformKey(key: events.Key) ?c_int {
    return switch (key) {
        .character => |c| switch (c) {
            'a'...'z', '0'...'9' => @intCast(c),
            'A'...'Z' => @intCast(c + 32),
            ' ' => platform.Key.space,
            '`', '-', '=', '[', ']', ';', '\'', ',', '.', '/', '\\' => @intCast(c),
            else => null,
        },
        .function => |n| if (n >= 1 and n <= 12) platform.Key.f1 + n - 1 else null,
        .enter => platform.Key.enter,
        .tab => platform.Key.tab,
        .escape => platform.Key.escape,
        .left => platform.Key.left,
        .right => platform.Key.right,
        .up => platform.Key.up,
        .down => platform.Key.down,
        else => null,
    };
}

fn platformModifiers(m: events.Modifiers) c_int {
    var out: c_int = 0;
    if (m.shift) out |= platform.Mod.shift;
    if (m.control) out |= platform.Mod.control;
    if (m.alt) out |= platform.Mod.alt;
    if (m.meta) out |= platform.Mod.meta;
    return out;
}

/// Registers a system-wide hotkey (works while other apps are in front);
/// presses arrive as `.hotkey = id`. Re-registering an id replaces it.
/// False when another app already uses the combination or it is invalid.
pub fn registerHotkey(id: u32, shortcut: Shortcut) bool {
    if (!shortcut.isValid()) return false;
    return platform.registerHotkey(@intCast(id), platformKey(shortcut.key).?, platformModifiers(shortcut.modifiers));
}

pub fn unregisterHotkey(id: u32) void {
    platform.unregisterHotkey(@intCast(id));
}

test "shortcut text form and labels" {
    const s = Shortcut.parse("ctrl+alt+v").?;
    try std.testing.expect(s.modifiers.control and s.modifiers.alt and !s.modifiers.shift);
    var buf: [32]u8 = undefined;
    try std.testing.expectEqualStrings("ctrl+alt+v", s.format(&buf));
    try std.testing.expectEqualStrings(if (is_macos) "⌃⌥V" else "Ctrl+Alt+V", s.label(&buf));
    const f = Shortcut.parse("meta+shift+f5").?;
    try std.testing.expectEqualStrings("shift+meta+f5", f.format(&buf));
    try std.testing.expect(Shortcut.parse("v") == null); // needs ctrl / alt / meta
    try std.testing.expect(Shortcut.parse("shift+v") == null);
    try std.testing.expect(Shortcut.parse("ctrl+") == null);
    try std.testing.expect(Shortcut.fromKeyEvent(.{ .key = .{ .character = 'V' }, .modifiers = .{ .meta = true } }).?.eql(.{ .key = .{ .character = 'v' }, .modifiers = .{ .meta = true } }));
}

test {
    _ = png;
}
