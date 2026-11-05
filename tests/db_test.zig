const std = @import("std");
const zant = @import("zant");

const User = struct {
    id: i64 = 0,
    name: []const u8,
    age: i32,
    active: bool,

    pub const table_name = "users";
};

test "collection hides persistence details" {
    var users = try zant.db.collection(User, std.testing.allocator, ":memory:");
    defer users.close();

    const id = try users.add(.{ .name = "Ada", .age = 36, .active = true });
    try std.testing.expectEqual(@as(i64, 1), id);

    const found = (try users.find(id)).?;
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

    try users.remove(id);
    try std.testing.expectEqual(@as(i64, 0), try users.count());
}

test "collection persists across reopen" {
    const path = "/tmp/zant-db-collection-test.sqlite";
    std.Io.Dir.cwd().deleteFile(std.testing.io, path) catch {};
    defer std.Io.Dir.cwd().deleteFile(std.testing.io, path) catch {};

    var first = try zant.db.collection(User, std.testing.allocator, path);
    _ = try first.add(.{ .name = "Persisted", .age = 20, .active = false });
    first.close();

    var second = try zant.db.collection(User, std.testing.allocator, path);
    defer second.close();
    const rows = try second.all();
    defer second.freeAll(rows);
    try std.testing.expectEqual(@as(usize, 1), rows.len);
    try std.testing.expectEqualStrings("Persisted", rows[0].name);
}
