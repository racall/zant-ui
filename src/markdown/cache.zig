//! Streamdown's processing pipeline with block-level memoisation:
//!
//!     source ─(streaming? remend)→ healed ─(parse-blocks)→ block strings
//!            ─(per block, cached by content)→ parsed documents
//!
//! A block whose text did not change keeps its parsed document (and its
//! hash, which lets the view skip rebuilding its UI via `ui.memo`). While a
//! response streams in, only the last block or two are parsed again per tick.
//! The cache is owned by the caller (no global state).
const std = @import("std");
const ast = @import("ast.zig");
const parser = @import("parser/mod.zig");
const blocks_mod = @import("blocks.zig");
const remend = @import("remend/mod.zig");
const hash_mod = @import("../core/hash.zig");

pub const Mode = enum {
    /// Complete documents (notes, files).
    static,
    /// Content arriving incrementally (AI responses): incomplete syntax is
    /// healed with remend before parsing.
    streaming,
};

pub const Options = struct {
    mode: Mode = .static,
    /// Streamdown `parseIncompleteMarkdown` (only used in streaming mode).
    parse_incomplete: bool = true,
    remend: remend.Options = .{},
    parse: parser.Options = .{},
};

pub const Entry = struct {
    /// Stable identity for the entry's lifetime (used for UI keys, so an
    /// unchanged block keeps its nodes when blocks are inserted before it).
    id: u64,
    raw: []u8,
    doc: ast.Document,
    /// Hash of `raw` and the parse options.
    hash: u64,
    /// Opaque per-entry slot for the view (stable while the entry lives).
    view_slot: ?*anyopaque = null,
    /// Scratch memory of the entry's last UI build; reset on every rebuild.
    view_arena: std.heap.ArenaAllocator,

    pub fn arena(self: *Entry) std.mem.Allocator {
        return self.doc.arena.allocator();
    }
};

pub const Cache = struct {
    allocator: std.mem.Allocator,
    splitter: blocks_mod.Splitter,
    entries: std.ArrayListUnmanaged(*Entry) = .empty,
    options_hash: u64 = 0,
    /// Statistics of the last update (tests / profiling).
    parsed_last: usize = 0,
    next_id: u64 = 1,

    pub fn init(allocator: std.mem.Allocator) Cache {
        return .{ .allocator = allocator, .splitter = blocks_mod.Splitter.init(allocator) };
    }

    pub fn deinit(self: *Cache) void {
        for (self.entries.items) |e| self.freeEntry(e);
        self.entries.deinit(self.allocator);
        self.splitter.deinit();
    }

    fn freeEntry(self: *Cache, e: *Entry) void {
        e.view_arena.deinit();
        e.doc.deinit();
        self.allocator.free(e.raw);
        self.allocator.destroy(e);
    }

    /// Brings the cache up to date with `source`; returns the block entries
    /// (valid until the next update / deinit).
    pub fn update(self: *Cache, source: []const u8, options: Options) ![]const *Entry {
        var scratch = std.heap.ArenaAllocator.init(self.allocator);
        defer scratch.deinit();
        const healed = if (options.mode == .streaming and options.parse_incomplete)
            try remend.remend(scratch.allocator(), source, options.remend)
        else
            source;

        const options_hash = hash_mod.hash(1, options.parse);
        if (options_hash != self.options_hash) {
            for (self.entries.items) |e| self.freeEntry(e);
            self.entries.clearRetainingCapacity();
            self.options_hash = options_hash;
        }

        self.splitter.setOptions(options.parse);
        const raw_blocks = try self.splitter.split(healed);
        const old = self.entries.items;
        // keep the common prefix and suffix; only the middle is re-parsed
        var p: usize = 0;
        while (p < old.len and p < raw_blocks.len and std.mem.eql(u8, old[p].raw, raw_blocks[p])) p += 1;
        var s: usize = 0;
        while (s < old.len - p and s < raw_blocks.len - p and
            std.mem.eql(u8, old[old.len - 1 - s].raw, raw_blocks[raw_blocks.len - 1 - s])) s += 1;

        const middle = raw_blocks[p .. raw_blocks.len - s];
        var fresh: std.ArrayListUnmanaged(*Entry) = .empty;
        defer fresh.deinit(self.allocator);
        errdefer for (fresh.items) |e| self.freeEntry(e);
        try fresh.ensureTotalCapacity(self.allocator, middle.len);
        for (middle) |raw| fresh.appendAssumeCapacity(try self.createEntry(raw, options_hash, options.parse));

        // changed blocks keep the identity (UI key) of the block they
        // replace, position by position, like Streamdown's index keys: the
        // block that grows while streaming keeps its nodes, so unchanged
        // lines / cells keep their shaped text instead of being rebuilt
        const removed = old.len - s - p;
        for (fresh.items[0..@min(removed, fresh.items.len)], old[p..][0..@min(removed, fresh.items.len)]) |e, prev| e.id = prev.id;
        // splice: old[p .. old.len - s] → fresh
        for (old[p .. old.len - s]) |e| self.freeEntry(e);
        try self.entries.replaceRange(self.allocator, p, removed, fresh.items);
        fresh.clearRetainingCapacity();
        self.parsed_last = middle.len;
        return self.entries.items;
    }

    fn createEntry(self: *Cache, raw: []const u8, options_hash: u64, parse: parser.Options) !*Entry {
        const entry = try self.allocator.create(Entry);
        errdefer self.allocator.destroy(entry);
        const owned = try self.allocator.dupe(u8, raw);
        errdefer self.allocator.free(owned);
        var h = std.hash.Wyhash.init(options_hash);
        h.update(raw);
        entry.* = .{
            .id = self.next_id,
            .raw = owned,
            .doc = try parser.parse(self.allocator, owned, parse),
            .hash = h.final(),
            .view_arena = std.heap.ArenaAllocator.init(self.allocator),
        };
        self.next_id += 1;
        return entry;
    }

    /// Index of the entry containing byte `offset` of the (healed) source.
    pub fn entryAtOffset(self: *const Cache, offset: usize) ?usize {
        var start: usize = 0;
        for (self.entries.items, 0..) |e, i| {
            const end = start + e.raw.len;
            if (offset < end or (i + 1 == self.entries.items.len and offset == end)) return i;
            start = end;
        }
        return null;
    }
};

test "streaming reparses only the tail" {
    var cache = Cache.init(std.testing.allocator);
    defer cache.deinit();
    _ = try cache.update("# Title\n\nFirst paragraph.\n\nSecond **bo", .{ .mode = .streaming });
    const entries = try cache.update("# Title\n\nFirst paragraph.\n\nSecond **bold** text\n\nThird", .{ .mode = .streaming });
    try std.testing.expectEqual(@as(usize, 4), entries.len);
    try std.testing.expect(cache.parsed_last <= 2);
    // healed incomplete bold rendered as bold
    var bold = false;
    for (entries[2].doc.blocks[0].paragraph) |s| if (s.style.bold) {
        bold = true;
    };
    try std.testing.expect(bold);
}

test "a block growing while streaming keeps its id" {
    var cache = Cache.init(std.testing.allocator);
    defer cache.deinit();
    const a = try cache.update("# T\n\nPara one", .{ .mode = .streaming });
    const id = a[1].id;
    const b = try cache.update("# T\n\nPara one grows", .{ .mode = .streaming });
    try std.testing.expectEqual(id, b[1].id);
    const c = try cache.update("# T\n\nPara one grows\n\nNext", .{ .mode = .streaming });
    try std.testing.expectEqual(id, c[1].id);
    try std.testing.expect(c[2].id != id);
}

test "inserting a block in the middle parses only that block" {
    var cache = Cache.init(std.testing.allocator);
    defer cache.deinit();
    const before = try cache.update("# A\n\nOne.\n\nTwo.\n\nThree.", .{});
    try std.testing.expectEqual(@as(usize, 4), before.len);
    const ids = [_]u64{ before[0].id, before[1].id, before[2].id, before[3].id };
    const after = try cache.update("# A\n\nOne.\n\nInserted.\n\nTwo.\n\nThree.", .{});
    try std.testing.expectEqual(@as(usize, 5), after.len);
    try std.testing.expectEqual(@as(usize, 1), cache.parsed_last);
    try std.testing.expectEqual(ids[0], after[0].id);
    try std.testing.expectEqual(ids[1], after[1].id);
    try std.testing.expectEqual(ids[2], after[3].id);
    try std.testing.expectEqual(ids[3], after[4].id);
    // deleting it again re-parses nothing
    _ = try cache.update("# A\n\nOne.\n\nTwo.\n\nThree.", .{});
    try std.testing.expectEqual(@as(usize, 0), cache.parsed_last);
}
