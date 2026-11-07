//! Decoded `data:image/…;base64,…` URIs, cached by content hash (UI thread).
const std = @import("std");
const image = @import("../../image.zig");

const max_entries = 64;
var entries: std.AutoHashMapUnmanaged(u64, image.Image) = .empty;

pub fn get(uri: []const u8) ?image.Image {
    const key = std.hash.Wyhash.hash(0x64617461, uri);
    if (entries.get(key)) |img| return img;
    const img = image.fromDataUri(std.heap.c_allocator, uri) orelse return null;
    if (entries.count() >= max_entries) {
        // drop one: nodes showing it keep their own reference
        var it = entries.iterator();
        if (it.next()) |kv| {
            kv.value_ptr.deinit();
            entries.removeByPtr(kv.key_ptr);
        }
    }
    entries.put(std.heap.c_allocator, key, img) catch {
        img.deinit();
        return null;
    };
    return img;
}
