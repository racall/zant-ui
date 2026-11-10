//! Runtime of the TSX `useMemo` / `useEffect` hooks (re-exported by
//! `tsx.zig`): dependency lists compared by value, cached values in their own
//! memory.
const std = @import("std");
const deepCopy = @import("tsx.zig").deepCopy;
const ui = @import("ui.zig");
const actions = @import("core/actions.zig");

/// Dependency list of `useMemo` / `useEffect` (`D` is a tuple of the values):
/// a copy of the last list in its own memory, compared by value (strings and
/// arrays by content), so a list that comes back equal counts as unchanged.
pub fn Deps(comptime D: type) type {
    return struct {
        const Self = @This();
        last: ?D = null,
        arena: ?std.heap.ArenaAllocator = null,

        /// True on the first call and whenever `deps` differs from the last
        /// list; remembers `deps`.
        pub fn changed(self: *Self, deps: D) bool {
            if (self.last) |last| if (deepEql(D, last, deps)) return false;
            var next = std.heap.ArenaAllocator.init(std.heap.smp_allocator);
            const copy = deepCopy(D, next.allocator(), deps) catch @panic("Zant UI: out of memory in hook deps");
            if (self.arena) |*previous| previous.deinit();
            self.arena = next;
            self.last = copy;
            return true;
        }

        pub fn deinit(self: *Self) void {
            if (self.arena) |*arena| arena.deinit();
            self.* = .{};
        }
    };
}

/// `useMemo`: the value computed for the current dependency list, kept in its
/// own memory until the list changes.
pub fn Memo(comptime T: type, comptime D: type) type {
    return struct {
        const Self = @This();
        deps: Deps(D) = .{},
        value: ?T = null,
        arena: ?std.heap.ArenaAllocator = null,

        /// The cached value when `deps` is unchanged; null means compute and
        /// `store` it.
        pub fn hit(self: *Self, deps: D) ?T {
            if (!self.deps.changed(deps)) return self.value;
            return null;
        }

        pub fn store(self: *Self, value: T) T {
            var next = std.heap.ArenaAllocator.init(std.heap.smp_allocator);
            const copy = deepCopy(T, next.allocator(), value) catch @panic("Zant UI: out of memory in useMemo");
            if (self.arena) |*previous| previous.deinit();
            self.arena = next;
            self.value = copy;
            return copy;
        }

        pub fn deinit(self: *Self) void {
            self.deps.deinit();
            if (self.arena) |*arena| arena.deinit();
            self.* = .{};
        }
    };
}

/// Value equality of compiler-produced types (slices by content).
pub fn deepEql(comptime T: type, a: T, b: T) bool {
    return switch (@typeInfo(T)) {
        .pointer => blk: {
            if (a.len != b.len) break :blk false;
            if (a.ptr == b.ptr) break :blk true;
            const Child = @typeInfo(T).pointer.child;
            for (a, b) |x, y| if (!deepEql(Child, x, y)) break :blk false;
            break :blk true;
        },
        .@"struct" => |structure| blk: {
            inline for (structure.field_names, structure.field_types) |name, F| {
                if (!deepEql(F, @field(a, name), @field(b, name))) break :blk false;
            }
            break :blk true;
        },
        .optional => |optional| if (a) |x| (if (b) |y| deepEql(optional.child, x, y) else false) else b == null,
        .array => |array| blk: {
            for (a, b) |x, y| if (!deepEql(array.child, x, y)) break :blk false;
            break :blk true;
        },
        .@"enum", .int, .float, .bool => a == b,
        else => @compileError("unsupported TSX dependency type " ++ @typeName(T)),
    };
}

test "Deps and Memo compare by value" {
    var deps: Deps(struct { []const u8, u32 }) = .{};
    defer deps.deinit();
    var name = [_]u8{ 'a', 'b' };
    try std.testing.expect(deps.changed(.{ &name, 1 }));
    try std.testing.expect(!deps.changed(.{ "ab", 1 }));
    name[0] = 'x'; // the copy is the list's own
    try std.testing.expect(deps.changed(.{ &name, 1 }));
    try std.testing.expect(deps.changed(.{ &name, 2 }));

    var memo: Memo([]const u32, struct { u32 }) = .{};
    defer memo.deinit();
    try std.testing.expect(memo.hit(.{1}) == null);
    var items = [_]u32{ 4, 5 };
    _ = memo.store(&items);
    items[0] = 9;
    try std.testing.expectEqualSlices(u32, &.{ 4, 5 }, memo.hit(.{1}).?);
    try std.testing.expect(memo.hit(.{2}) == null);
}

/// A `useEffect` with a cleanup function (or deps): the dependency list plus
/// the cleanup the last run returned, kept in its own memory. The cleanup runs
/// before the effect runs again and when the component unmounts (`deinit`).
pub fn Effect(comptime D: type) type {
    return struct {
        const Self = @This();
        deps: Deps(D) = .{},
        cleanup: ?actions.Action = null,
        arena: ?std.heap.ArenaAllocator = null,

        pub fn changed(self: *Self, deps: D) bool {
            return self.deps.changed(deps);
        }

        /// Queues the previous cleanup, then the effect (`ui.effect`).
        pub fn queue(self: *Self, effect: actions.Action) void {
            if (self.cleanup != null) ui.effect(ui.action(self, runCleanup));
            ui.effect(effect);
        }

        pub fn runCleanup(self: *Self) void {
            const cleanup = self.cleanup orelse return;
            self.cleanup = null;
            cleanup.func(cleanup.ctx);
            if (self.arena) |*arena| arena.deinit();
            self.arena = null;
        }

        /// Keeps `ctx` (a capture struct or the component) for the cleanup
        /// `f`. Captured values are copied; a child component's struct is
        /// copied too (its frame copy goes away; `_s`, its local state, stays
        /// valid until the cleanup ran), the root's is used as it is.
        pub fn setCleanup(self: *Self, ctx: anytype, comptime f: anytype, comptime child: bool) void {
            const P = @TypeOf(ctx.*);
            if (self.arena) |*arena| arena.deinit();
            self.arena = std.heap.ArenaAllocator.init(std.heap.smp_allocator);
            const a = self.arena.?.allocator();
            const kept: *P = if (@hasField(P, "self")) blk: {
                const copy = a.create(P) catch oom();
                copy.* = ctx.*;
                inline for (@typeInfo(P).@"struct".field_names) |name| {
                    if (comptime std.mem.eql(u8, name, "self")) {
                        if (child) copy.self = shallow(a, ctx.self);
                    } else {
                        @field(copy, name) = deepCopy(@FieldType(P, name), a, @field(ctx, name)) catch oom();
                    }
                }
                break :blk copy;
            } else if (child) shallow(a, ctx) else ctx;
            self.cleanup = ui.action(kept, f);
        }

        pub fn deinit(self: *Self) void {
            self.runCleanup();
            self.deps.deinit();
        }
    };
}

fn shallow(a: std.mem.Allocator, pointer: anytype) @TypeOf(pointer) {
    const copy = a.create(@TypeOf(pointer.*)) catch oom();
    copy.* = pointer.*;
    return copy;
}

fn oom() noreturn {
    @panic("Zant UI: out of memory in useEffect");
}

test "Effect keeps its cleanup until the next run or deinit" {
    const Probe = struct {
        var cleaned: u32 = 0;
        var seen: u32 = 0;
        const Cap = struct { self: *u32, n: []const u8 };
        fn cleanup(cap: *Cap) void {
            cleaned += 1;
            seen = @intCast(cap.n.len);
        }
    };
    var effect: Effect(struct { u32 }) = .{};
    var owner: u32 = 0;
    var text = [_]u8{ 'a', 'b', 'c' };
    try std.testing.expect(effect.changed(.{1}));
    var cap = Probe.Cap{ .self = &owner, .n = &text };
    effect.setCleanup(&cap, Probe.cleanup, false);
    text[0] = 'x';
    cap.n = ""; // the effect keeps its own copy
    try std.testing.expect(!effect.changed(.{1}));
    effect.runCleanup();
    try std.testing.expectEqual(@as(u32, 1), Probe.cleaned);
    try std.testing.expectEqual(@as(u32, 3), Probe.seen);
    effect.runCleanup(); // once
    try std.testing.expectEqual(@as(u32, 1), Probe.cleaned);
    effect.setCleanup(&cap, Probe.cleanup, true);
    effect.deinit();
    try std.testing.expectEqual(@as(u32, 2), Probe.cleaned);
}
