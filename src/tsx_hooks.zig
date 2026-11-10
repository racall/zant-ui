//! Runtime of the TSX `useMemo` / `useEffect` hooks (re-exported by
//! `tsx.zig`): dependency lists compared by value, cached values in their own
//! memory.
const std = @import("std");
const deepCopy = @import("tsx.zig").deepCopy;

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
