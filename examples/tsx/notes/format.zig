//! Formatting helpers shared with the TSX notes app.
const std = @import("std");
const zant = @import("zant");

/// Return a count and its English label in memory valid until the frame ends.
pub fn plural(n: usize, word: []const u8) []const u8 {
    return std.fmt.allocPrint(zant.ui.frameAllocator(), "{d} {s}{s}", .{
        n, word, if (n == 1) "" else "s",
    }) catch @panic("out of memory");
}
