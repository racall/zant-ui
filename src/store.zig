const std = @import("std");

/// A tiny Zustand-style store for UI state.
///
/// - `get()` returns a mutable pointer to the current state.
/// - `update(.{ ... })` merges the named fields and notifies subscribers.
/// - `notify()` notifies after direct mutation through `get()`.
///
/// The store is intended for the UI thread. It is deliberately not thread-safe.
pub fn Store(comptime T: type) type {
    return struct {
        const Self = @This();
        pub const Listener = struct {
            ctx: *anyopaque,
            func: *const fn (*anyopaque) void,
        };

        allocator: std.mem.Allocator,
        state: T,
        listeners: std.ArrayListUnmanaged(Listener) = .empty,

        pub fn init(allocator: std.mem.Allocator, initial: T) !*Self {
            const self = try allocator.create(Self);
            self.* = .{ .allocator = allocator, .state = initial };
            return self;
        }

        pub fn deinit(self: *Self) void {
            self.listeners.deinit(self.allocator);
            self.allocator.destroy(self);
        }

        pub fn get(self: *Self) *T {
            return &self.state;
        }

        pub fn getConst(self: *const Self) *const T {
            return &self.state;
        }

        pub fn set(self: *Self, next: T) void {
            self.state = next;
            self.notify();
        }

        /// Shallow partial update, similar to Zustand's `set(partial)`.
        pub fn update(self: *Self, changes: anytype) void {
            const info = @typeInfo(@TypeOf(changes)).@"struct";
            inline for (info.field_names, 0..) |field_name, i| {
                _ = i;
                @field(self.state, field_name) = @field(changes, field_name);
            }
            self.notify();
        }

        pub fn mutate(self: *Self, func: *const fn (*T) void) void {
            func(&self.state);
            self.notify();
        }

        pub fn subscribe(self: *Self, ctx: *anyopaque, func: *const fn (*anyopaque) void) !void {
            try self.listeners.append(self.allocator, .{ .ctx = ctx, .func = func });
        }

        pub fn notify(self: *Self) void {
            for (self.listeners.items) |listener| listener.func(listener.ctx);
        }
    };
}

pub fn create(comptime T: type, allocator: std.mem.Allocator, initial: T) !*Store(T) {
    return Store(T).init(allocator, initial);
}
