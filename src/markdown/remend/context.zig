//! Shared state for one `remend` call: the arena every handler allocates
//! from and the single-entry scan memo of `scan.ts` (`getScan`).
const std = @import("std");
const Scan = @import("scan.zig").Scan;

pub const Context = struct {
    arena: std.mem.Allocator,
    cached: ?Scan = null,

    /// Memoized scan of `text`. Handlers pass each other's output along, and
    /// most return their input unchanged, so pointer identity is a cheap and
    /// exact cache key.
    pub fn scan(self: *Context, text: []const u8) !*Scan {
        if (self.cached) |*cached| {
            if (cached.text.ptr == text.ptr and cached.text.len == text.len) return cached;
        }
        self.cached = try Scan.init(self.arena, text);
        return &self.cached.?;
    }

    pub fn isInsideCodeBlock(self: *Context, text: []const u8, position: usize) !bool {
        return (try self.scan(text)).isCodeAt(@intCast(position));
    }

    pub fn isWithinCompleteInlineCode(self: *Context, text: []const u8, position: usize) !bool {
        return (try self.scan(text)).isCompleteSpanAt(@intCast(position));
    }

    pub fn concat(self: *Context, parts: []const []const u8) ![]const u8 {
        return std.mem.concat(self.arena, u8, parts);
    }
};
