const std = @import("std");
const zant = @import("zant");

const User = struct {
    id: i64 = 0,
    name: []const u8,
    age: i32,
    active: bool,
    pub const table_name = "users";
};

const Settings = struct {
    dark: bool = false,
    count: i32 = 0,
};

test "Store key value JSON and collection" {
    var store = try zant.db.open(std.testing.allocator, ":memory:");
    defer store.close();

    try store.setString("theme", "dark");
    try store.setInt("launches", 3);
    try store.setBool("onboarded", true);
    try store.setFloat("scale", 1.25);
    try store.setJson("settings", Settings{ .dark = true, .count = 7 });

    var buf: [64]u8 = undefined;
    try std.testing.expectEqualStrings("dark", (try store.getString("theme", &buf)).?);
    try std.testing.expectEqual(@as(i64, 3), store.getInt("launches", 0));
    try std.testing.expect(store.getBool("onboarded", false));
    try std.testing.expectApproxEqAbs(@as(f64, 1.25), store.getFloat("scale", 0), 0.001);
    try std.testing.expect(store.has("theme"));

    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const settings = (try store.getJson(Settings, "settings", arena.allocator())).?;
    try std.testing.expect(settings.dark);
    try std.testing.expectEqual(@as(i32, 7), settings.count);

    var users = try store.collection(User);
    const id = try users.add(.{ .name = "Ada", .age = 36, .active = true });
    const found = (try users.find(id)).?;
    defer users.free(found);
    try std.testing.expectEqualStrings("Ada", found.name);
    try std.testing.expectEqual(@as(i64, 1), try users.count());

    try store.remove("theme");
    try std.testing.expect(!store.has("theme"));
}

test "Store collection persists across reopen" {
    const path = "/tmp/zant-store-test.sqlite";
    std.Io.Dir.cwd().deleteFile(std.testing.io, path) catch {};
    defer std.Io.Dir.cwd().deleteFile(std.testing.io, path) catch {};

    var first = try zant.db.open(std.testing.allocator, path);
    var users = try first.collection(User);
    _ = try users.add(.{ .name = "Persisted", .age = 20, .active = false });
    first.close();

    var second = try zant.db.open(std.testing.allocator, path);
    defer second.close();
    var users2 = try second.collection(User);
    const rows = try users2.all();
    defer users2.freeAll(rows);
    try std.testing.expectEqual(@as(usize, 1), rows.len);
    try std.testing.expectEqualStrings("Persisted", rows[0].name);
}
