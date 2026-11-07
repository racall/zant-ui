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

const Kind = enum { text, image };

const ClipV1 = struct {
    id: i64 = 0,
    body: []const u8,
    pub const table_name = "clips";
};

const ClipV2 = struct {
    id: i64 = 0,
    kind: Kind = .image,
    body: []const u8,
    pinned: bool,
    pub const table_name = "clips";
};

test "Store collection: enums, new fields of a later version, clear" {
    const path = "/tmp/zant-store-migrate.sqlite";
    std.Io.Dir.cwd().deleteFile(std.testing.io, path) catch {};
    defer std.Io.Dir.cwd().deleteFile(std.testing.io, path) catch {};
    {
        var store = try zant.db.open(std.testing.allocator, path);
        defer store.close();
        var clips = try store.collection(ClipV1);
        _ = try clips.add(.{ .body = "old" });
    }
    var store = try zant.db.open(std.testing.allocator, path);
    defer store.close();
    // the table gains kind / pinned; the old row gets their defaults
    var clips = try store.collection(ClipV2);
    try store.begin();
    _ = try clips.add(.{ .kind = .text, .body = "new", .pinned = true });
    try store.commit();
    const rows = try clips.all();
    defer clips.freeAll(rows);
    try std.testing.expectEqual(@as(usize, 2), rows.len);
    try std.testing.expectEqual(Kind.image, rows[0].kind);
    try std.testing.expect(!rows[0].pinned);
    try std.testing.expectEqualStrings("old", rows[0].body);
    try std.testing.expectEqual(Kind.text, rows[1].kind);
    try std.testing.expect(rows[1].pinned);
    try clips.clear();
    try std.testing.expectEqual(@as(i64, 0), try clips.count());
}

const Message = struct {
    id: i64 = 0,
    from: []const u8,
    order: i64,
};

test "Store collection: SQL keywords as names" {
    var store = try zant.db.open(std.testing.allocator, ":memory:");
    defer store.close();
    var messages = try store.collection(Message);
    _ = try messages.add(.{ .from = "Ada", .order = 2 });
    const rows = try messages.all();
    defer messages.freeAll(rows);
    try std.testing.expectEqualStrings("Ada", rows[0].from);
    try std.testing.expectEqual(@as(i64, 2), rows[0].order);
}
