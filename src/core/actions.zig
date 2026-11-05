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
