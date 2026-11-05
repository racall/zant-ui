const std = @import("std");
const zant = @import("zant");

test "database CRUD and typed values" {
    var db = try zant.db.Database.openMemory(std.testing.allocator);
    defer db.close();
    try db.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT, score REAL)");
    try db.run("INSERT INTO notes (body, score) VALUES (?, ?)", .{ "hello", 1.5 });
    try std.testing.expectEqual(@as(i64, 1), db.lastInsertId());
    var rows = try db.query("SELECT id, body, score FROM notes", .{});
    defer rows.deinit();
    const row = (try rows.next()).?;
    try std.testing.expectEqual(@as(i64, 1), row.int(0));
    try std.testing.expectEqualStrings("hello", row.text(1));
    try std.testing.expectApproxEqAbs(@as(f64, 1.5), row.float(2), 0.001);
    try std.testing.expect((try rows.next()) == null);
}

test "database transaction rollback" {
    var db = try zant.db.Database.openMemory(std.testing.allocator);
    defer db.close();
    try db.exec("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT)");
    const Fail = struct {
        fn run(d: *zant.db.Database) !void {
            try d.run("INSERT INTO notes (body) VALUES (?)", .{"bad"});
            return error.Boom;
        }
    };
    try std.testing.expectError(error.Boom, db.transaction(Fail.run, .{}));
    const count = (try db.queryOne(i64, "SELECT COUNT(*) FROM notes", .{})).?;
    try std.testing.expectEqual(@as(i64, 0), count);
}

const User = struct {
    id: i64 = 0,
    name: []const u8,
    age: i32,
    active: bool,

    pub const table_name = "users";
};

test "high-level table API does not require SQL" {
    var db = try zant.db.Database.openMemory(std.testing.allocator);
    defer db.close();

    var users = db.table(User);
    try users.create();
    const id = try users.insert(.{ .name = "Ada", .age = 36, .active = true });
    try std.testing.expectEqual(@as(i64, 1), id);

    const found = (try users.get(id)).?;
    defer users.free(found);
    try std.testing.expectEqualStrings("Ada", found.name);
    try std.testing.expectEqual(@as(i32, 36), found.age);
    try std.testing.expect(found.active);
    try std.testing.expectEqual(@as(i64, 1), try users.count());

    var updated = found;
    updated.age = 37;
    try users.update(updated);

    const all_rows = try users.all();
    defer users.freeAll(all_rows);
    try std.testing.expectEqual(@as(usize, 1), all_rows.len);
    try std.testing.expectEqual(@as(i32, 37), all_rows[0].age);

    try users.delete(id);
    try std.testing.expectEqual(@as(i64, 0), try users.count());
}
