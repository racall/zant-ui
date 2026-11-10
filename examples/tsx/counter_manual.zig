//! The TSX counter (`counter.tsx`) written by hand in Zig, the way Zant apps
//! are usually written: the baseline `zant-tsx-bench` compares the
//! generated code against.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;

pub const Counter = struct {
    count: u32 = 0,
    step: u32 = 1,
    name_buf: [128]u8 = blk: {
        var buf: [128]u8 = undefined;
        @memcpy(buf[0..4], "Zant");
        break :blk buf;
    },
    name_len: usize = 4,
    large: bool = false,
    notify: bool = true,

    fn name(self: *const Counter) []const u8 {
        return self.name_buf[0..self.name_len];
    }

    fn onName(self: *Counter, value: []const u8) void {
        const n = @min(value.len, self.name_buf.len);
        @memcpy(self.name_buf[0..n], value[0..n]);
        self.name_len = n;
    }

    fn increment(self: *Counter) void {
        self.count += self.step;
    }

    fn reset(self: *Counter) void {
        self.count = 0;
    }

    fn toggleStep(self: *Counter) void {
        self.step = if (self.step == 1) 5 else 1;
    }

    fn setLarge(self: *Counter, on: bool) void {
        self.large = on;
    }

    fn setNotify(self: *Counter, on: bool) void {
        self.notify = on;
    }

    pub fn render(self: *Counter) ui.Node {
        const a = ui.frameAllocator();
        var children: std.ArrayList(ui.Node) = .empty;
        children.appendSlice(a, &.{
            ui.text(ui.frameString(std.fmt.allocPrint(a, "Hello, {s}!", .{self.name()}) catch ""), .{
                .key = "title",
                .size = if (self.large) 28 else 20,
                .weight = .semibold,
            }),
            ui.textFmt("Count: {d} (×2 = {d})", .{ self.count, self.count * 2 }, .{ .key = "count" }),
            ui.input(.{
                .key = "name",
                .value = self.name(),
                .placeholder = "Your name",
                .width = 240,
                .on_change = ui.textAction(self, onName),
            }),
            ui.row(.{
                .gap = 8,
                .align_items = .center,
                .children = &.{
                    ui.button(.{
                        .key = "inc",
                        .text = std.fmt.allocPrint(a, "+{d}", .{self.step}) catch "",
                        .variant = .primary,
                        .on_click = ui.action(self, increment),
                    }),
                    ui.button(.{
                        .key = "reset",
                        .text = "Reset",
                        .disabled = self.count == 0,
                        .on_click = ui.action(self, reset),
                    }),
                    ui.button(.{
                        .key = "step",
                        .text = std.fmt.allocPrint(a, "Step {d}", .{self.step}) catch "",
                        .on_click = ui.action(self, toggleStep),
                    }),
                },
            }),
            ui.@"switch"(.{ .key = "large", .label = "Large title", .checked = self.large, .on_change = ui.boolAction(self, setLarge) }),
            ui.checkbox(.{ .key = "notify", .label = "Notify me", .checked = self.notify, .on_change = ui.boolAction(self, setNotify) }),
        }) catch {};
        if (self.notify) children.append(a, ui.text("Notifications are on", .{ .key = "note" })) catch {};
        if (self.count >= 10) children.append(a, ui.text("That is a lot of clicks.", .{ .key = "many" })) catch {};
        return ui.view(.{ .padding = ui.inset(24), .gap = 14, .align_items = .start, .children = children.items });
    }
};
