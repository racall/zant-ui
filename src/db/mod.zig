//! Persistence for Zant UI apps. No SQL/SQLite surface for business code.
const std = @import("std");
const sqlite = @import("sqlite.zig");
pub const Error = sqlite.Error;

pub fn Collection(comptime T: type) type {
    return struct {
        const Self = @This();
        db: *sqlite.Database,
        fn table(self: *Self) sqlite.Table(T) {
            return self.db.table(T);
        }
        pub fn add(self: *Self, value: T) !i64 {
            return self.table().insert(value);
        }
        pub fn find(self: *Self, id: i64) !?T {
            return self.table().get(id);
        }
        pub fn all(self: *Self) ![]T {
            return self.table().all();
        }
        pub fn update(self: *Self, value: T) !void {
            return self.table().update(value);
        }
        pub fn remove(self: *Self, id: i64) !void {
            return self.table().delete(id);
        }
        pub fn count(self: *Self) !i64 {
            return self.table().count();
        }
        /// Removes every object.
        pub fn clear(self: *Self) !void {
            return self.table().deleteAll();
        }
        pub fn free(self: *Self, value: T) void {
            self.table().free(value);
        }
        pub fn freeAll(self: *Self, values: []T) void {
            self.table().freeAll(values);
        }
    };
}

pub const Store = struct {
    allocator: std.mem.Allocator,
    db: sqlite.Database,

    pub fn open(allocator: std.mem.Allocator, path: [:0]const u8) !Store {
        var db = try sqlite.Database.open(allocator, path);
        errdefer db.close();
        // write-ahead log: small writes (one per user action) without a full
        // sync each, and readers never block the writer
        db.exec("PRAGMA journal_mode=WAL") catch {};
        db.exec("PRAGMA synchronous=NORMAL") catch {};
        db.exec("PRAGMA busy_timeout=2000") catch {};
        // a file that is not a database (damaged) fails here
        try db.exec("CREATE TABLE IF NOT EXISTS _zant_kv (key TEXT PRIMARY KEY, kind INTEGER NOT NULL, value TEXT)");
        return .{ .allocator = allocator, .db = db };
    }
    pub fn close(self: *Store) void {
        self.db.close();
    }
    /// Groups many changes into one write (and makes them all-or-nothing):
    /// `try store.begin(); errdefer store.rollback(); ...; try store.commit();`
    pub fn begin(self: *Store) !void {
        try self.db.begin();
    }
    pub fn commit(self: *Store) !void {
        try self.db.commit();
    }
    pub fn rollback(self: *Store) void {
        self.db.rollback() catch {};
    }
    pub fn collection(self: *Store, comptime T: type) !Collection(T) {
        try self.db.table(T).create();
        return .{ .db = &self.db };
    }
    fn kvPut(self: *Store, key: []const u8, kind: i64, value: []const u8) !void {
        var stmt = try self.db.prepare("INSERT INTO _zant_kv(key, kind, value) VALUES(?, ?, ?) ON CONFLICT(key) DO UPDATE SET kind=excluded.kind, value=excluded.value");
        defer stmt.deinit();
        try stmt.bindValue(1, key);
        try stmt.bindValue(2, kind);
        try stmt.bindValue(3, value);
        _ = try stmt.step();
    }
    pub fn setString(self: *Store, key: []const u8, value: []const u8) !void {
        try self.kvPut(key, 0, value);
    }
    pub fn setInt(self: *Store, key: []const u8, value: i64) !void {
        var buf: [32]u8 = undefined;
        try self.kvPut(key, 1, try std.fmt.bufPrint(&buf, "{d}", .{value}));
    }
    pub fn setBool(self: *Store, key: []const u8, value: bool) !void {
        try self.kvPut(key, 2, if (value) "1" else "0");
    }
    pub fn setFloat(self: *Store, key: []const u8, value: f64) !void {
        var buf: [64]u8 = undefined;
        try self.kvPut(key, 3, try std.fmt.bufPrint(&buf, "{d}", .{value}));
    }
    pub fn setJson(self: *Store, key: []const u8, value: anytype) !void {
        const text = try std.fmt.allocPrint(self.allocator, "{f}", .{std.json.fmt(value, .{})});
        defer self.allocator.free(text);
        try self.kvPut(key, 4, text);
    }
    pub fn set(self: *Store, key: []const u8, value: anytype) !void {
        switch (@typeInfo(@TypeOf(value))) {
            .bool => try self.setBool(key, value),
            .int, .comptime_int => try self.setInt(key, @intCast(value)),
            .float, .comptime_float => try self.setFloat(key, @floatCast(value)),
            .pointer => |ptr| if (ptr.size == .slice and ptr.child == u8) try self.setString(key, value) else @compileError("use setJson"),
            else => @compileError("use setJson"),
        }
    }
    fn kvGet(self: *Store, key: []const u8, out: []u8) !?struct { kind: i64, text: []const u8 } {
        var stmt = try self.db.prepare("SELECT kind, value FROM _zant_kv WHERE key = ? LIMIT 1");
        defer stmt.deinit();
        try stmt.bindValue(1, key);
        if (!try stmt.step()) return null;
        const text = stmt.row().text(1);
        if (text.len + 1 > out.len) return Error.ValueTooLarge;
        @memcpy(out[0..text.len], text);
        out[text.len] = 0;
        return .{ .kind = stmt.row().int(0), .text = out[0..text.len] };
    }
    pub fn getString(self: *Store, key: []const u8, out: []u8) !?[]const u8 {
        if (try self.kvGet(key, out)) |value| return value.text;
        return null;
    }
    pub fn getInt(self: *Store, key: []const u8, default: i64) i64 {
        var buf: [64]u8 = undefined;
        if ((self.kvGet(key, &buf) catch null)) |v| return std.fmt.parseInt(i64, v.text, 10) catch default;
        return default;
    }
    pub fn getBool(self: *Store, key: []const u8, default: bool) bool {
        var buf: [16]u8 = undefined;
        if ((self.kvGet(key, &buf) catch null)) |v| return std.mem.eql(u8, v.text, "1");
        return default;
    }
    pub fn getFloat(self: *Store, key: []const u8, default: f64) f64 {
        var buf: [64]u8 = undefined;
        if ((self.kvGet(key, &buf) catch null)) |v| return std.fmt.parseFloat(f64, v.text) catch default;
        return default;
    }
    pub fn getJson(self: *Store, comptime T: type, key: []const u8, allocator: std.mem.Allocator) !?T {
        var buf: [64 * 1024]u8 = undefined;
        const value = (try self.kvGet(key, &buf)) orelse return null;
        return std.json.parseFromSliceLeaky(T, allocator, value.text, .{}) catch Error.InvalidJson;
    }
    pub fn has(self: *Store, key: []const u8) bool {
        var buf: [8]u8 = undefined;
        return (self.kvGet(key, &buf) catch null) != null;
    }
    pub fn remove(self: *Store, key: []const u8) !void {
        var stmt = try self.db.prepare("DELETE FROM _zant_kv WHERE key = ?");
        defer stmt.deinit();
        try stmt.bindValue(1, key);
        _ = try stmt.step();
    }
    pub fn clear(self: *Store) !void {
        try self.db.exec("DELETE FROM _zant_kv");
    }
};
pub fn open(allocator: std.mem.Allocator, path: [:0]const u8) !Store {
    return Store.open(allocator, path);
}
