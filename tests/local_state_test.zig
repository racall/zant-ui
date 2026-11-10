//! ui.localState: slots live while their instance keeps being built, are
//! freed on the rebuild that drops it, and survive `ui.memo` skips.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;

const expectEqual = std.testing.expectEqual;

const Counter = struct {
    clicks: u32 = 0,
    var unmounted: u32 = 0;
    pub fn deinit(_: *Counter) void {
        unmounted += 1;
    }
};

const App = struct {
    rows: []const u64 = &.{ 1, 2, 3 },
    memo_hash: u64 = 1,
    in_memo: bool = false,
    seen: [8]*Counter = undefined,

    fn row(self: *App, id: u64, index: usize) ui.Node {
        const state = ui.localState(Counter, ui.childId(0, id), .{});
        self.seen[index] = state;
        return ui.text(ui.frameString("row"), .{});
    }

    fn memoBody(self: *App) ui.Node {
        return ui.column(.{ .children = &.{self.row(99, 7)} });
    }

    fn render(ctx: *anyopaque) ui.Node {
        const self: *App = @ptrCast(@alignCast(ctx));
        var children: std.ArrayList(ui.Node) = .empty;
        for (self.rows, 0..) |id, i| children.append(ui.frameAllocator(), self.row(id, i)) catch unreachable;
        if (self.in_memo) {
            const memo = ui.memo(.{ .key = "memo", .hash = self.memo_hash }, self, memoBody);
            children.append(ui.frameAllocator(), memo) catch unreachable;
        }
        return ui.column(.{ .children = children.items });
    }
};

test "local state persists across rebuilds and frees on unmount" {
    Counter.unmounted = 0;
    var app = App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, App.render, .{});
    defer runtime.deinit();

    runtime.rebuildForTest();
    try expectEqual(@as(usize, 3), runtime.local_states.count());
    app.seen[1].clicks = 5;
    const second = app.seen[1];

    runtime.rebuildForTest();
    try expectEqual(second, app.seen[1]);
    try expectEqual(@as(u32, 5), app.seen[1].clicks);

    // row 2 leaves: its slot is freed after that rebuild
    app.rows = &.{ 1, 3 };
    runtime.rebuildForTest();
    try expectEqual(@as(usize, 2), runtime.local_states.count());
    try expectEqual(@as(u32, 1), Counter.unmounted);

    // coming back is a fresh mount
    app.rows = &.{ 1, 2, 3 };
    runtime.rebuildForTest();
    try expectEqual(@as(u32, 0), app.seen[1].clicks);
}

test "local state inside a skipped memo stays mounted" {
    Counter.unmounted = 0;
    var app = App{ .rows = &.{}, .in_memo = true };
    const runtime = try Runtime.init(std.testing.allocator, &app, App.render, .{});
    defer runtime.deinit();

    runtime.rebuildForTest();
    app.seen[7].clicks = 4;
    const inside = app.seen[7];

    // same hash: the memo builder does not run, the slot is kept
    runtime.rebuildForTest();
    runtime.rebuildForTest();
    try expectEqual(@as(usize, 1), runtime.local_states.count());
    try expectEqual(@as(u32, 0), Counter.unmounted);

    // a new hash rebuilds it with the same slot
    app.memo_hash = 2;
    runtime.rebuildForTest();
    try expectEqual(inside, app.seen[7]);
    try expectEqual(@as(u32, 4), app.seen[7].clicks);

    // the memo leaves
    app.in_memo = false;
    runtime.rebuildForTest();
    try expectEqual(@as(usize, 0), runtime.local_states.count());
    try expectEqual(@as(u32, 1), Counter.unmounted);
}
