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
