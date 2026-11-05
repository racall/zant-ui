//! 应用状态与动作。
//!
//! 状态由 main.zig 持有，dashboard / counter_card 等组件通过 props 读取，
//! 通过回调修改。这是最直接、最容易理解的跨组件状态方式。

const std = @import("std");

pub const AppState = struct {
    name_buf: [64]u8 = undefined,
    name_len: usize = 0,
    clicks: u32 = 0,

    pub fn init(default_name: []const u8) AppState {
        var state = AppState{};
        state.setName(default_name);
        return state;
    }

    pub fn name(self: *const AppState) []const u8 {
        return self.name_buf[0..self.name_len];
    }

    pub fn setName(self: *AppState, value: []const u8) void {
        const n = @min(value.len, self.name_buf.len);
        @memcpy(self.name_buf[0..n], value[0..n]);
        self.name_len = n;
    }

    pub fn onName(self: *AppState, value: []const u8) void {
        self.setName(value);
    }

    pub fn increment(self: *AppState) void {
        self.clicks +%= 1;
    }

    pub fn reset(self: *AppState) void {
        self.clicks = 0;
    }
};

test "计数动作" {
    var state = AppState.init("Demo");
    state.increment();
    state.increment();
    try std.testing.expectEqual(@as(u32, 2), state.clicks);
    state.reset();
    try std.testing.expectEqual(@as(u32, 0), state.clicks);
}
