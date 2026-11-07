const std = @import("std");
const events = @import("events.zig");

pub const Action = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque) void,
};

pub const BoolAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, checked: bool) void,
};

pub const TextAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, value: []const u8) void,
};

pub fn call(action: ?Action) void {
    if (action) |a| a.func(a.ctx);
}

pub fn callBool(action: ?BoolAction, checked: bool) void {
    if (action) |a| a.func(a.ctx, checked);
}

pub fn callText(action: ?TextAction, value: []const u8) void {
    if (action) |a| a.func(a.ctx, value);
}

pub const IndexAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, index: usize) void,
};

pub fn callIndex(action: ?IndexAction, index: usize) void {
    if (action) |a| a.func(a.ctx, index);
}

/// Reports a byte range, e.g. an input selection `(anchor, cursor)`.
pub const RangeAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, start: usize, end: usize) void,
};

pub fn callRange(action: ?RangeAction, start: usize, end: usize) void {
    if (action) |a| a.func(a.ctx, start, end);
}

/// Key handler for a focused input. Receives the input's current text and
/// selection `(anchor, cursor)`; return true to consume the key.
pub const InputKeyAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, key: events.KeyEvent, text: []const u8, anchor: usize, cursor: usize) bool,
};

/// What happened to a pointer area (`ui.pointerArea`).
pub const PointerPhase = enum {
    /// Button pressed inside the area; the area captures the pointer until `up`.
    down,
    /// Pointer moved while captured (dragging), coordinates may leave the area.
    drag,
    /// Button released after `down` (also sent when the capture is lost).
    up,
    /// Pointer moved over the area without a button pressed.
    hover,
    /// Pointer left the area (after `hover`).
    leave,
    /// Wheel / trackpad scroll over an area with `capture_scroll`.
    scroll,
};

/// A pointer event delivered to a pointer area. `x` / `y` are relative to
/// the area's top-left corner (logical px); `window_x` / `window_y` are
/// window coordinates.
pub const PointerInfo = struct {
    phase: PointerPhase,
    x: f32,
    y: f32,
    window_x: f32,
    window_y: f32,
    /// Size of the area.
    width: f32,
    height: f32,
    button: events.PointerButton = .left,
    modifiers: events.Modifiers = .{},
    /// 1 for a single click, 2 for a double click, 3 for a triple click
    /// (`down` / `up` only).
    click_count: u8 = 1,
    /// Scroll delta (`scroll` only): positive `dy` moves content down, like
    /// AppKit's scrollingDeltaY.
    dx: f32 = 0,
    dy: f32 = 0,
};

/// Pointer area handler; return true when application state changed and
/// the UI must be rebuilt.
pub const PointerAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, info: PointerInfo) bool,
};

/// Key handler of a focused pointer area; return true to consume the key.
pub const KeyAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, key: events.KeyEvent) bool,
};
