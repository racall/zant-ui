const std = @import("std");
const zant = @import("zant");

test "zustand-style store update and notify" {
    const State = struct { count: u32 = 0, enabled: bool = false };
    const Counter = struct {
        calls: u32 = 0,
        fn changed(ctx: *anyopaque) void {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            self.calls += 1;
        }
    };
    const store = try zant.store.Store(State).init(std.testing.allocator, .{});
    defer store.deinit();
    var counter = Counter{};
    try store.subscribe(&counter, Counter.changed);

    store.update(.{ .count = 3 });
    try std.testing.expectEqual(@as(u32, 3), store.get().count);
    try std.testing.expectEqual(@as(u32, 1), counter.calls);

    store.get().enabled = true;
    store.notify();
    try std.testing.expect(store.get().enabled);
    try std.testing.expectEqual(@as(u32, 2), counter.calls);
}
