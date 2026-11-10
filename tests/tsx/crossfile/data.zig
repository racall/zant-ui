//! Native data for tests/tsx/crossfile/.
pub const Kind = enum { text, image };
pub const Data = struct { row: i64, kind: Kind, label: []const u8 };
pub fn sample() []const Data {
    return &.{ .{ .row = 1, .kind = .text, .label = "one" }, .{ .row = 2, .kind = .image, .label = "two" } };
}
