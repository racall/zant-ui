//! Per-instance component state (`ui.localState`): the runtime owns one heap
//! slot per state id; a slot that no rebuild asks for again is freed after
//! that rebuild (the component unmounted).
//!
//! Subtrees skipped by `ui.memo` do not run their builders, so the ids a memo
//! build touched are remembered on its node and touched again on a skip.
const std = @import("std");

const Entry = struct {
    ptr: *anyopaque,
    /// `@typeName(T)`: catches one id used with two different types.
    type_name: [*:0]const u8,
    /// Rebuild that last asked for this slot.
    seen: u64,
    destroy: *const fn (std.mem.Allocator, *anyopaque) void,
};

pub fn Slot(comptime T: type) type {
    return struct {
        value: *T,
        /// Created by this call (first build of the instance).
        fresh: bool,
    };
}

pub const Store = struct {
    allocator: std.mem.Allocator,
    entries: std.AutoHashMapUnmanaged(u64, Entry) = .empty,
    /// Rebuild counter; sweeping frees entries not seen in the current one.
    build: u64 = 0,
    /// Ids touched inside the memo build being reconciled (see `record`).
    recording: ?*std.ArrayListUnmanaged(u64) = null,
    /// Set while a rebuild runs; `sweep` only frees after one.
    building: bool = false,

    pub fn init(allocator: std.mem.Allocator) Store {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Store) void {
        var it = self.entries.valueIterator();
        while (it.next()) |entry| entry.destroy(self.allocator, entry.ptr);
        self.entries.deinit(self.allocator);
    }

    pub fn beginBuild(self: *Store) void {
        self.build += 1;
        self.building = true;
    }

    /// The slot for `id`, created with `T`'s default value on first use.
    pub fn slot(self: *Store, comptime T: type, id: u64) Slot(T) {
        const gop = self.entries.getOrPut(self.allocator, id) catch oom();
        if (gop.found_existing) {
            if (gop.value_ptr.type_name != @typeName(T).ptr) {
                std.debug.panic("Zant UI: local state id {x} used with {s} and {s}", .{
                    id, std.mem.span(gop.value_ptr.type_name), @typeName(T),
                });
            }
            self.touchEntry(id, gop.value_ptr);
            return .{ .value = @ptrCast(@alignCast(gop.value_ptr.ptr)), .fresh = false };
        }
        const value = self.allocator.create(T) catch oom();
        value.* = if (@typeInfo(T) == .@"struct") std.mem.zeroInit(T, .{}) else std.mem.zeroes(T);
        gop.value_ptr.* = .{
            .ptr = value,
            .type_name = @typeName(T).ptr,
            .seen = 0,
            .destroy = Destroy(T).destroy,
        };
        self.touchEntry(id, gop.value_ptr);
        return .{ .value = value, .fresh = true };
    }

    /// Keeps the slots of a memo subtree that was skipped this rebuild.
    pub fn touch(self: *Store, ids: []const u64) void {
        for (ids) |id| {
            if (self.entries.getPtr(id)) |entry| self.touchEntry(id, entry);
        }
    }

    fn touchEntry(self: *Store, id: u64, entry: *Entry) void {
        entry.seen = self.build;
        if (self.recording) |list| list.append(self.allocator, id) catch oom();
    }

    /// Starts recording the ids touched while a memo subtree is built; returns
    /// the enclosing recording for `endRecord`.
    pub fn record(self: *Store, list: *std.ArrayListUnmanaged(u64)) ?*std.ArrayListUnmanaged(u64) {
        const previous = self.recording;
        self.recording = list;
        return previous;
    }

    /// Ends a recording; an enclosing memo also keeps the inner ids.
    pub fn endRecord(self: *Store, previous: ?*std.ArrayListUnmanaged(u64)) void {
        const inner = self.recording;
        self.recording = previous;
        if (previous) |outer| if (inner) |ids| {
            outer.appendSlice(self.allocator, ids.items) catch oom();
        };
    }

    /// After a rebuild: frees the slots it did not ask for (unmounted).
    pub fn sweep(self: *Store) void {
        if (!self.building) return;
        self.building = false;
        var stale: std.ArrayListUnmanaged(u64) = .empty;
        defer stale.deinit(self.allocator);
        var it = self.entries.iterator();
        while (it.next()) |kv| {
            if (kv.value_ptr.seen != self.build) stale.append(self.allocator, kv.key_ptr.*) catch oom();
        }
        for (stale.items) |id| {
            const entry = self.entries.fetchRemove(id).?.value;
            entry.destroy(self.allocator, entry.ptr);
        }
    }

    pub fn count(self: *const Store) usize {
        return self.entries.count();
    }
};

fn Destroy(comptime T: type) type {
    return struct {
        fn destroy(allocator: std.mem.Allocator, raw: *anyopaque) void {
            const value: *T = @ptrCast(@alignCast(raw));
            if (@typeInfo(T) == .@"struct" and @hasDecl(T, "deinit")) value.deinit();
            allocator.destroy(value);
        }
    };
}

fn oom() noreturn {
    @panic("Zant UI: out of memory in local state");
}

/// The id of a child instance: its parent's id mixed with a name, key or
/// position (`[]const u8` or any integer).
pub fn childId(parent: u64, part: anytype) u64 {
    const P = @TypeOf(part);
    var hasher = std.hash.Wyhash.init(parent);
    switch (@typeInfo(P)) {
        .int, .comptime_int => {
            const value: u64 = @intCast(part);
            hasher.update(std.mem.asBytes(&value));
            hasher.update("#");
        },
        else => {
            const bytes: []const u8 = part;
            hasher.update(bytes);
            hasher.update("$");
        },
    }
    return hasher.final();
}

test "slots persist across builds and free when not rebuilt" {
    const S = struct {
        n: u32 = 7,
        var freed: u32 = 0;
        pub fn deinit(_: *@This()) void {
            freed += 1;
        }
    };
    var store = Store.init(std.testing.allocator);
    defer store.deinit();

    store.beginBuild();
    const first = store.slot(S, 1);
    try std.testing.expect(first.fresh);
    try std.testing.expectEqual(@as(u32, 7), first.value.n);
    first.value.n = 9;
    _ = store.slot(S, 2);
    store.sweep();

    store.beginBuild();
    const again = store.slot(S, 1);
    try std.testing.expect(!again.fresh);
    try std.testing.expectEqual(@as(u32, 9), again.value.n);
    store.sweep();
    try std.testing.expectEqual(@as(u32, 1), S.freed);
    try std.testing.expectEqual(@as(usize, 1), store.count());
}

test "memo recordings keep skipped slots alive" {
    var store = Store.init(std.testing.allocator);
    defer store.deinit();
    var outer: std.ArrayListUnmanaged(u64) = .empty;
    defer outer.deinit(std.testing.allocator);
    var inner: std.ArrayListUnmanaged(u64) = .empty;
    defer inner.deinit(std.testing.allocator);

    store.beginBuild();
    const a = store.record(&outer);
    const b = store.record(&inner);
    _ = store.slot(u32, 5);
    store.endRecord(b);
    _ = store.slot(u32, 6);
    store.endRecord(a);
    store.sweep();
    try std.testing.expectEqualSlices(u64, &.{ 5, 6 }, outer.items);

    // the memo is skipped: its ids are touched instead of rebuilt
    store.beginBuild();
    store.touch(outer.items);
    store.sweep();
    try std.testing.expectEqual(@as(usize, 2), store.count());
}

test "child ids differ by key kind and parent" {
    try std.testing.expect(childId(1, 3) != childId(1, "3"));
    try std.testing.expect(childId(1, "a") != childId(2, "a"));
    try std.testing.expectEqual(childId(9, "row"), childId(9, "row"));
}
