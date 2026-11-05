//! Small, high-level SQLite wrapper for Zant UI apps.
//!
//!     var db = try zant.db.open(allocator, "app.sqlite");
//!     defer db.close();
//!     try db.exec("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT)");
//!     try db.run("INSERT INTO notes (body) VALUES (?)", .{"hello"});
//!     var rows = try db.query("SELECT id, body FROM notes ORDER BY id", .{});
//!     defer rows.deinit();
//!     while (try rows.next()) |row| {
//!         const id = row.int(0);
//!         const body = row.text(1);
//!     }

const std = @import("std");
const c = @import("sqlite_c.zig");

pub const Error = error{
    OpenFailed,
    PrepareFailed,
    StepFailed,
    BindFailed,
    ColumnOutOfRange,
    TypeMismatch,
    Busy,
};

pub const Value = union(enum) {
    null,
    integer: i64,
    real: f64,
    text: []const u8,
    blob: []const u8,
};

pub const Row = struct {
    stmt: *c.sqlite3_stmt,

    pub fn count(self: Row) usize {
        return @intCast(c.sqlite3_column_count(self.stmt));
    }

    pub fn isNull(self: Row, index: usize) bool {
        return c.sqlite3_column_type(self.stmt, @intCast(index)) == c.SQLITE_NULL;
    }

    pub fn int(self: Row, index: usize) i64 {
        return c.sqlite3_column_int64(self.stmt, @intCast(index));
    }

    pub fn float(self: Row, index: usize) f64 {
        return c.sqlite3_column_double(self.stmt, @intCast(index));
    }

    pub fn text(self: Row, index: usize) []const u8 {
        const ptr = c.sqlite3_column_text(self.stmt, @intCast(index));
        if (ptr == null) return "";
        const len: usize = @intCast(c.sqlite3_column_bytes(self.stmt, @intCast(index)));
        return ptr[0..len];
    }

    pub fn blob(self: Row, index: usize) []const u8 {
        const ptr = c.sqlite3_column_blob(self.stmt, @intCast(index));
        if (ptr == null) return "";
        const len: usize = @intCast(c.sqlite3_column_bytes(self.stmt, @intCast(index)));
        return @as([*]const u8, @ptrCast(ptr))[0..len];
    }

    pub fn value(self: Row, index: usize) Value {
        return switch (c.sqlite3_column_type(self.stmt, @intCast(index))) {
            c.SQLITE_NULL => .null,
            c.SQLITE_INTEGER => .{ .integer = self.int(index) },
            c.SQLITE_FLOAT => .{ .real = self.float(index) },
            c.SQLITE_BLOB => .{ .blob = self.blob(index) },
            else => .{ .text = self.text(index) },
        };
    }
};

pub const Statement = struct {
    db: *Database,
    stmt: *c.sqlite3_stmt,
    finalized: bool = false,

    pub fn deinit(self: *Statement) void {
        if (!self.finalized) {
            _ = c.sqlite3_finalize(self.stmt);
            self.finalized = true;
        }
    }

    pub fn bind(self: *Statement, params: anytype) !void {
        const info = @typeInfo(@TypeOf(params));
        if (info != .@"struct") @compileError("params must be a tuple");
        inline for (info.@"struct".field_names, 0..) |field_name, i| {
            const value = @field(params, field_name);
            const index: c_int = @intCast(i + 1);
            const rc = switch (@typeInfo(@TypeOf(value))) {
                .null => c.sqlite3_bind_null(self.stmt, index),
                .int, .comptime_int => c.sqlite3_bind_int64(self.stmt, index, @intCast(value)),
                .float, .comptime_float => c.sqlite3_bind_double(self.stmt, index, @floatCast(value)),
                .bool => c.sqlite3_bind_int(self.stmt, index, if (value) 1 else 0),
                .optional => if (value) |v| switch (@typeInfo(@TypeOf(v))) {
                    .int => c.sqlite3_bind_int64(self.stmt, index, @intCast(v)),
                    .float => c.sqlite3_bind_double(self.stmt, index, @floatCast(v)),
                    else => blk: {
                        const text: []const u8 = @ptrCast(v);
                        break :blk c.sqlite3_bind_text(self.stmt, index, text.ptr, @intCast(text.len), null);
                    },
                } else c.sqlite3_bind_null(self.stmt, index),
                .pointer => |ptr| blk: {
                    if (ptr.size == .slice and ptr.child == u8) {
                        break :blk c.sqlite3_bind_text(self.stmt, index, value.ptr, @intCast(value.len), null);
                    }
                    if (ptr.size == .one and @typeInfo(ptr.child) == .array) {
                        const text: []const u8 = value;
                        break :blk c.sqlite3_bind_text(self.stmt, index, text.ptr, @intCast(text.len), null);
                    }
                    @compileError("unsupported SQLite parameter type");
                },
                else => @compileError("unsupported SQLite parameter type"),
            };
            if (rc != c.SQLITE_OK) return Error.BindFailed;
        }
    }

    /// Returns true when a row is available, false when done.
    pub fn step(self: *Statement) !bool {
        const rc = c.sqlite3_step(self.stmt);
        return switch (rc) {
            c.SQLITE_ROW => true,
            c.SQLITE_DONE => false,
            c.SQLITE_BUSY => Error.Busy,
            else => Error.StepFailed,
        };
    }

    pub fn row(self: *Statement) Row {
        return .{ .stmt = self.stmt };
    }

    pub fn reset(self: *Statement) void {
        _ = c.sqlite3_reset(self.stmt);
        _ = c.sqlite3_clear_bindings(self.stmt);
    }
};

pub const Rows = struct {
    stmt: Statement,
    has_row: bool = false,
    done: bool = false,

    pub fn deinit(self: *Rows) void {
        self.stmt.deinit();
    }

    pub fn next(self: *Rows) !?Row {
        if (self.done) return null;
        if (try self.stmt.step()) {
            self.has_row = true;
            return self.stmt.row();
        }
        self.done = true;
        return null;
    }
};

pub const Database = struct {
    allocator: std.mem.Allocator,
    handle: *c.sqlite3,

    pub fn open(allocator: std.mem.Allocator, path: [:0]const u8) !Database {
        var handle: ?*c.sqlite3 = null;
        if (c.sqlite3_open(path.ptr, &handle) != c.SQLITE_OK) {
            if (handle) |h| _ = c.sqlite3_close(h);
            return Error.OpenFailed;
        }
        return .{ .allocator = allocator, .handle = handle.? };
    }

    /// Open an in-memory database, useful for tests and scratch data.
    pub fn openMemory(allocator: std.mem.Allocator) !Database {
        return open(allocator, ":memory:");
    }

    pub fn close(self: *Database) void {
        _ = c.sqlite3_close(self.handle);
        self.* = undefined;
    }

    pub fn lastInsertId(self: *Database) i64 {
        return c.sqlite3_last_insert_rowid(self.handle);
    }

    pub fn changes(self: *Database) u64 {
        return @intCast(c.sqlite3_changes(self.handle));
    }

    pub fn exec(self: *Database, sql: [:0]const u8) !void {
        var message: [*c]u8 = null;
        const rc = c.sqlite3_exec(self.handle, sql.ptr, null, null, &message);
        if (message != null) c.sqlite3_free(message);
        if (rc != c.SQLITE_OK) return Error.StepFailed;
    }

    pub fn prepare(self: *Database, sql: [:0]const u8) !Statement {
        var stmt: ?*c.sqlite3_stmt = null;
        if (c.sqlite3_prepare_v2(self.handle, sql.ptr, -1, &stmt, null) != c.SQLITE_OK) return Error.PrepareFailed;
        return .{ .db = self, .stmt = stmt orelse return Error.PrepareFailed };
    }

    /// Execute a statement with positional `?` parameters.
    pub fn run(self: *Database, sql: [:0]const u8, params: anytype) !void {
        var stmt = try self.prepare(sql);
        defer stmt.deinit();
        try stmt.bind(params);
        _ = try stmt.step();
    }

    /// Query with positional parameters. Caller owns the returned Rows.
    pub fn query(self: *Database, sql: [:0]const u8, params: anytype) !Rows {
        var stmt = try self.prepare(sql);
        errdefer stmt.deinit();
        try stmt.bind(params);
        return .{ .stmt = stmt };
    }

    /// Convenience for a single scalar result; empty result returns null.
    pub fn queryOne(self: *Database, comptime T: type, sql: [:0]const u8, params: anytype) !?T {
        var rows = try self.query(sql, params);
        defer rows.deinit();
        if (try rows.next()) |row| {
            return switch (T) {
                i64 => row.int(0),
                f64 => row.float(0),
                []const u8 => row.text(0),
                else => @compileError("unsupported queryOne type"),
            };
        }
        return null;
    }

    pub fn begin(self: *Database) !void {
        try self.exec("BEGIN");
    }
    pub fn commit(self: *Database) !void {
        try self.exec("COMMIT");
    }
    pub fn rollback(self: *Database) !void {
        try self.exec("ROLLBACK");
    }

    /// Run a callback inside a transaction, rolling back on error.
    pub fn transaction(self: *Database, comptime callback: anytype, args: anytype) !void {
        try self.begin();
        errdefer self.rollback() catch {};
        try @call(.auto, callback, args ++ .{self});
        try self.commit();
    }
};

test "sqlite CRUD and typed rows" {
    var db = try Database.openMemory(std.testing.allocator);
    defer db.close();
    try db.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT, score REAL)");
    try db.run("INSERT INTO notes (body, score) VALUES (?, ?)", .{ "hello", 1.5 });
    try std.testing.expectEqual(@as(i64, 1), db.lastInsertId());
    var rows = try db.query("SELECT id, body, score FROM notes", .{});
    defer rows.deinit();
    try std.testing.expect((try rows.next()) != null);
    const row = rows.stmt.row();
    try std.testing.expectEqual(@as(i64, 1), row.int(0));
    try std.testing.expectEqualStrings("hello", row.text(1));
    try std.testing.expectApproxEqAbs(@as(f64, 1.5), row.float(2), 0.001);
    try std.testing.expect(!(try rows.next() != null));
}

test "sqlite transaction rolls back" {
    var db = try Database.openMemory(std.testing.allocator);
    defer db.close();
    try db.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)");
    const Fail = struct {
        fn run(d: *Database) !void {
            try d.run("INSERT INTO notes (body) VALUES (?)", .{"bad"});
            return error.Boom;
        }
    };
    try std.testing.expectError(error.Boom, db.transaction(Fail.run, .{}));
    const count = (try db.queryOne(i64, "SELECT COUNT(*) FROM notes", .{})).?;
    try std.testing.expectEqual(@as(i64, 0), count);
}
