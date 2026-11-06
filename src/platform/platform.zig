const std = @import("std");
const builtin = @import("builtin");

pub const EventType = struct {
    pub const pointer_move: c_int = 1;
    pub const pointer_down: c_int = 2;
    pub const pointer_up: c_int = 3;
    pub const pointer_enter: c_int = 4;
    pub const pointer_leave: c_int = 5;
    pub const key_down: c_int = 6;
    pub const key_up: c_int = 7;
    pub const text_input: c_int = 8;
    pub const composition: c_int = 9;
    pub const window_resize: c_int = 10;
    pub const window_focus: c_int = 11;
    pub const window_blur: c_int = 12;
    pub const window_close: c_int = 13;
    pub const scroll: c_int = 14;
    pub const window_close_request: c_int = 15;
};

pub const Key = struct {
    pub const left: c_int = 0x1101;
    pub const right: c_int = 0x1102;
    pub const up: c_int = 0x1103;
    pub const down: c_int = 0x1104;
    pub const home: c_int = 0x1105;
    pub const end: c_int = 0x1106;
    pub const backspace: c_int = 0x1107;
    pub const delete: c_int = 0x1108;
    pub const escape: c_int = 0x1109;
    pub const tab: c_int = 0x110A;
    pub const enter: c_int = 0x110B;
};

pub const Mod = struct {
    pub const shift: c_int = 1 << 0;
    pub const control: c_int = 1 << 1;
    pub const alt: c_int = 1 << 2;
    pub const meta: c_int = 1 << 3;
    pub const caps_lock: c_int = 1 << 4;
};

pub const WindowOptions = struct {
    style: WindowStyle = .native,
    width: f32 = 560,
    height: f32 = 420,
    min_width: f32 = 320,
    min_height: f32 = 240,
    background: u32 = 0xfff5f5f5,
    resizable: bool = true,
    always_on_top: bool = false,
    movable_by_background: bool = false,
    center: bool = true,
    /// `.hidden_titlebar` only: height of the app's own title bar; the
    /// traffic lights are vertically centred in it (0 = system default).
    titlebar_height: f32 = 0,
};

pub const WindowStyle = enum(c_int) {
    native = 0,
    hidden_titlebar = 1,
    borderless = 2,
};

pub const PointerButton = struct {
    pub const left: c_int = 0;
    pub const right: c_int = 1;
    pub const middle: c_int = 2;
};

pub const Cursor = enum(c_int) {
    arrow = 0,
    pointing_hand = 1,
    ibeam = 2,
};

pub const Event = extern struct {
    type: c_int,
    x: f32 = 0,
    y: f32 = 0,
    scroll_x: f32 = 0,
    scroll_y: f32 = 0,
    button: c_int = 0,
    key: c_int = 0,
    modifiers: c_int = 0,
    is_repeat: c_int = 0,
    text: ?[*]const u8 = null,
    text_len: usize = 0,
    selection_start: usize = 0,
    selection_end: usize = 0,
    width: c_int = 0,
    height: c_int = 0,
    dpr: f32 = 1,

    pub fn textSlice(self: *const Event) []const u8 {
        if (self.text) |ptr| return ptr[0..self.text_len];
        return "";
    }

    pub fn has(self: *const Event, mod: c_int) bool {
        return (self.modifiers & mod) != 0;
    }

    /// The platform-native command modifier: Command on macOS, Control on Windows.
    pub fn command(self: *const Event) bool {
        return switch (builtin.os.tag) {
            .macos => self.has(Mod.meta),
            else => self.has(Mod.control),
        };
    }
};

pub const Host = extern struct {
    userdata: ?*anyopaque,
    draw: *const fn (?*anyopaque, ?*anyopaque, f32, f32, f32) callconv(.c) void,
    event: *const fn (?*anyopaque, *const Event) callconv(.c) c_int,
    ime_rect: *const fn (?*anyopaque, *f32, *f32, *f32, *f32) callconv(.c) c_int,
    next_frame_delay: *const fn (?*anyopaque) callconv(.c) c_int,
    /// `Hit` for a point in logical window coordinates (app-drawn title bars).
    titlebar_hit: ?*const fn (?*anyopaque, f32, f32) callconv(.c) c_int = null,
};

/// Answers of `Host.titlebar_hit`.
pub const Hit = struct {
    pub const client: c_int = 0;
    pub const caption: c_int = 1;
    pub const maximize: c_int = 2;
};

/// One native window (`window.m` on macOS, `window.c` on Windows).
pub const Window = opaque {};

pub const is_macos = builtin.os.tag == .macos;
pub const is_windows = builtin.os.tag == .windows;
pub const supported = is_macos or is_windows;

/// Width the system keeps for its own window controls in an app-drawn
/// (`.hidden_titlebar`) title bar: the traffic lights on the left on macOS,
/// the caption buttons the framework draws on the right on Windows.
pub const TitlebarInsets = struct { left: f32 = 0, right: f32 = 0 };

/// Windows caption button size (Windows 11: 46 × 32 at 100 %).
pub const caption_button_width: f32 = 46;

pub fn titlebarInsets(style: WindowStyle) TitlebarInsets {
    if (style != .hidden_titlebar) return .{};
    if (is_windows) return .{ .right = caption_button_width * 3 };
    return .{ .left = 84 };
}

const NativeWindowOptions = extern struct {
    style: c_int,
    width: f32,
    height: f32,
    min_width: f32,
    min_height: f32,
    background: u32,
    resizable: c_int,
    always_on_top: c_int,
    movable_by_background: c_int,
    center: c_int,
    titlebar_height: f32,
};

extern fn zant_window_create(title: [*:0]const u8, options: *const NativeWindowOptions, host: *const Host) ?*Window;
extern fn zant_window_run(window: *Window) void;
extern fn zant_window_destroy(window: *Window) void;
extern fn zant_window_request_redraw(window: *Window, delay_ms: c_int) void;
extern fn zant_window_set_cursor(window: *Window, cursor: c_int) void;
extern fn zant_window_set_background(window: *Window, argb: u32) void;
extern fn zant_window_reduce_motion() c_int;
extern fn zant_window_set_document_edited(window: *Window, edited: c_int) void;
extern fn zant_window_renderer_name(window: *Window) [*:0]const u8;
extern fn zant_window_schedule_snapshot(window: *Window, path: [*:0]const u8, delay_ms: c_int) void;
extern fn zant_window_minimize(window: *Window) void;
extern fn zant_window_toggle_maximize(window: *Window) void;
extern fn zant_window_is_maximized(window: *Window) c_int;
extern fn zant_window_close(window: *Window) void;
extern fn zant_window_is_active(window: *Window) c_int;

extern fn zant_clipboard_set_text(text: [*]const u8, length: usize) c_int;
extern fn zant_clipboard_get_text(out: ?[*]u8, capacity: usize) usize;

pub fn createWindow(title: [:0]const u8, options: WindowOptions, host: *const Host) !*Window {
    if (!supported) return error.UnsupportedPlatform;
    const native = NativeWindowOptions{
        .style = @backingInt(options.style),
        .width = options.width,
        .height = options.height,
        .min_width = options.min_width,
        .min_height = options.min_height,
        .background = options.background,
        .resizable = if (options.resizable) 1 else 0,
        .always_on_top = if (options.always_on_top) 1 else 0,
        .movable_by_background = if (options.movable_by_background) 1 else 0,
        .center = if (options.center) 1 else 0,
        .titlebar_height = options.titlebar_height,
    };
    return zant_window_create(title.ptr, &native, host) orelse error.WindowCreationFailed;
}

pub fn run(window: *Window) void {
    zant_window_run(window);
}

pub fn destroy(window: *Window) void {
    zant_window_destroy(window);
}

pub fn requestRedraw(window: ?*Window, delay_ms: c_int) void {
    if (window) |w| zant_window_request_redraw(w, delay_ms);
}

pub fn setCursor(window: ?*Window, cursor: Cursor) void {
    if (window) |w| zant_window_set_cursor(w, @backingInt(cursor));
}

/// Marks the window as having unsaved changes (dot in the close button).
pub fn setDocumentEdited(window: *Window, edited: bool) void {
    zant_window_set_document_edited(window, @intFromBool(edited));
}

/// System "Reduce motion" accessibility setting.
pub fn reduceMotion() bool {
    return zant_window_reduce_motion() != 0;
}

pub fn setBackground(window: *Window, argb: u32) void {
    zant_window_set_background(window, argb);
}

pub fn rendererName(window: *Window) []const u8 {
    return std.mem.span(zant_window_renderer_name(window));
}

pub fn scheduleSnapshot(window: *Window, path: [:0]const u8, delay_ms: c_int) void {
    zant_window_schedule_snapshot(window, path.ptr, delay_ms);
}

pub fn minimize(window: ?*Window) void {
    if (window) |w| zant_window_minimize(w);
}

pub fn toggleMaximize(window: ?*Window) void {
    if (window) |w| zant_window_toggle_maximize(w);
}

pub fn isMaximized(window: ?*Window) bool {
    return if (window) |w| zant_window_is_maximized(w) != 0 else false;
}

/// Asks the window to close; the app's close-request hook may veto it.
pub fn close(window: ?*Window) void {
    if (window) |w| zant_window_close(w);
}

pub fn isActive(window: ?*Window) bool {
    return if (window) |w| zant_window_is_active(w) != 0 else true;
}

/// Monotonic clock in microseconds.
pub fn monotonicUs() i64 {
    if (is_windows) {
        const win = struct {
            extern "kernel32" fn QueryPerformanceCounter(count: *i64) callconv(.winapi) c_int;
            extern "kernel32" fn QueryPerformanceFrequency(frequency: *i64) callconv(.winapi) c_int;
        };
        var count: i64 = 0;
        var frequency: i64 = 1;
        _ = win.QueryPerformanceCounter(&count);
        _ = win.QueryPerformanceFrequency(&frequency);
        return @intCast(@divTrunc(@as(i128, count) * 1_000_000, frequency));
    }
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.MONOTONIC, &ts);
    return @as(i64, @intCast(ts.sec)) * 1_000_000 + @divTrunc(@as(i64, @intCast(ts.nsec)), 1000);
}

/// Monotonic clock in milliseconds.
pub fn monotonicMs() i64 {
    if (is_windows) {
        const win = struct {
            extern "kernel32" fn QueryPerformanceCounter(count: *i64) callconv(.winapi) c_int;
            extern "kernel32" fn QueryPerformanceFrequency(frequency: *i64) callconv(.winapi) c_int;
        };
        var count: i64 = 0;
        var frequency: i64 = 1;
        _ = win.QueryPerformanceCounter(&count);
        _ = win.QueryPerformanceFrequency(&frequency);
        return @divTrunc(count, @divTrunc(frequency, 1000));
    }
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.MONOTONIC, &ts);
    return @as(i64, @intCast(ts.sec)) * 1000 + @divTrunc(@as(i64, @intCast(ts.nsec)), 1_000_000);
}

/// Process environment variable (UTF-8 on Windows with the framework's
/// manifest, which selects the UTF-8 code page).
pub fn getenv(name: [*:0]const u8) ?[:0]const u8 {
    const value = std.c.getenv(name) orelse return null;
    return std.mem.span(value);
}

pub fn setClipboard(text: []const u8) bool {
    return zant_clipboard_set_text(text.ptr, text.len) != 0;
}

pub fn clipboardAlloc(allocator: std.mem.Allocator) ![]u8 {
    const required = zant_clipboard_get_text(null, 0);
    if (required == 0) return allocator.alloc(u8, 0);
    const buffer = try allocator.alloc(u8, required);
    _ = zant_clipboard_get_text(buffer.ptr, buffer.len);
    const len = std.mem.indexOfScalar(u8, buffer, 0) orelse buffer.len;
    return allocator.realloc(buffer, len);
}
