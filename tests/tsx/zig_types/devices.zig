//! A native module for tests/tsx_zig_types_test.zig: Zig types and functions
//! used from TSX.
const std = @import("std");
const zant = @import("zant");

pub const Kind = enum { desktop, laptop, phone };

pub const Address = struct {
    host: []const u8,
    port: u16,
};

pub const Peer = struct {
    name: []const u8,
    kind: Kind,
    address: Address,
    online: bool,
    tags: []const []const u8,
    last_seen: ?i64,
    /// not representable in TSX: left out of the TSX type
    lock: zant.sync.Mutex = .{},
};

const all = [_]Peer{
    .{
        .name = "Mac",
        .kind = .laptop,
        .address = .{ .host = "10.0.0.2", .port = 7070 },
        .online = true,
        .tags = &.{"home"},
        .last_seen = 1,
    },
    .{
        .name = "PC",
        .kind = .desktop,
        .address = .{ .host = "10.0.0.3", .port = 7070 },
        .online = false,
        .tags = &.{},
        .last_seen = null,
    },
};

pub fn peers() []const Peer {
    return &all;
}

pub fn label(peer: Peer) []const u8 {
    return std.fmt.allocPrint(zant.ui.frameAllocator(), "{s}:{d}", .{ peer.address.host, peer.address.port }) catch "";
}

pub fn firstOnline(list: []const Peer) ?Peer {
    for (list) |p| if (p.online) return p;
    return null;
}
