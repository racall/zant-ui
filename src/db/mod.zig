//! High-level persistence for Zant UI apps.
//!
//! Developers define a struct and work with a collection. There is no SQL,
//! database handle, table object, statement or row exposed in this API.

const std = @import("std");
const sqlite = @import("sqlite.zig");

pub const Error = sqlite.Error;

pub fn Collection(comptime T: type) type {
    return struct {
        const Self = @This();
        db: sqlite.Database,
        allocator: std.mem.Allocator,

        fn table(self: *Self) sqlite.Table(T) {
            return self.db.table(T);
        }

        /// Add one item. Returns its generated id.
        pub fn add(self: *Self, value: T) !i64 {
            return self.table().insert(value);
        }

        /// Find by id. The returned value owns its strings; call free().
        pub fn find(self: *Self, id: i64) !?T {
            return self.table().get(id);
        }

        /// Load all items. Call freeAll() when done.
        pub fn all(self: *Self) ![]T {
            return self.table().all();
        }

        /// Replace the stored item with the same id.
        pub fn update(self: *Self, value: T) !void {
            return self.table().update(value);
        }

        /// Remove by id.
        pub fn remove(self: *Self, id: i64) !void {
            return self.table().delete(id);
        }

        /// Number of stored items.
        pub fn count(self: *Self) !i64 {
            return self.table().count();
        }

        /// Free strings owned by one returned item.
        pub fn free(self: *Self, value: T) void {
            self.table().free(value);
        }

        /// Free a list returned by all().
        pub fn freeAll(self: *Self, values: []T) void {
            self.table().freeAll(values);
        }

        pub fn begin(self: *Self) !void {
            return self.db.begin();
        }

        pub fn commit(self: *Self) !void {
            return self.db.commit();
        }

        pub fn rollback(self: *Self) !void {
            return self.db.rollback();
        }

        /// Close the underlying persistence file.
        pub fn close(self: *Self) void {
            self.db.close();
        }
    };
}

/// Open (or create) a collection backed by `path`.
///
/// The schema is created automatically on first use; the caller never writes
/// CREATE TABLE, INSERT, SELECT or UPDATE.
pub fn collection(comptime T: type, allocator: std.mem.Allocator, path: [:0]const u8) !Collection(T) {
    var db = try sqlite.Database.open(allocator, path);
    errdefer db.close();
    const table = db.table(T);
    try table.create();
    return .{ .db = db, .allocator = allocator };
}
