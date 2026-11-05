const std = @import("std");

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
