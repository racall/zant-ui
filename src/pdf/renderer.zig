//! Background page rendering with a tile cache (EmbedPDF's render plugin
//! and tiling layer: a low-resolution base image per page, sharp tiles for
//! the visible area at the current zoom).
//!
//! The UI thread asks for tiles every build (`tile`); missing tiles are
//! queued for one worker thread, which renders them, stores the image and
//! calls `ui.postWake` so the next build shows them. Requests not repeated
//! by later builds are dropped (scrolled away), and the cache evicts the
//! least recently used tiles over its memory budget.
const std = @import("std");
const Document = @import("document.zig").Document;
const image = @import("../image.zig");
const ui = @import("../ui.zig");
const sync = @import("../core/sync.zig");

/// Device pixels per tile edge.
pub const tile_size: u32 = 1024;
/// Width in device pixels of the base image of a page.
pub const base_width: u32 = 360;

pub const Key = struct {
    page: u32,
    /// Scaled page width in device pixels (identifies the zoom level).
    page_w: u32,
    tx: u16,
    ty: u16,
};

const Entry = struct {
    image: ?image.Image = null,
    /// Frame that last asked for it (LRU and request freshness).
    used: u64 = 0,
    queued: bool = false,
    failed: bool = false,
    bytes: usize = 0,
};

pub const Renderer = struct {
    allocator: std.mem.Allocator,
    doc: *Document,
    mutex: sync.Mutex = .{},
    cond: sync.Condition = .{},
    thread: ?std.Thread = null,
    stop: bool = false,
    entries: std.AutoHashMapUnmanaged(Key, Entry) = .empty,
    /// Pending keys, newest last (the worker takes the newest first).
    queue: std.ArrayListUnmanaged(Key) = .empty,
    frame: u64 = 1,
    bytes: usize = 0,
    budget: usize = 384 * 1024 * 1024,

    pub fn create(allocator: std.mem.Allocator, doc: *Document) !*Renderer {
        const self = try allocator.create(Renderer);
        self.* = .{ .allocator = allocator, .doc = doc };
        self.thread = std.Thread.spawn(.{}, worker, .{self}) catch null;
        return self;
    }

    pub fn destroy(self: *Renderer) void {
        self.mutex.lock();
        self.stop = true;
        self.cond.signal();
        self.mutex.unlock();
        if (self.thread) |t| t.join();
        var it = self.entries.valueIterator();
        while (it.next()) |e| if (e.image) |img| img.deinit();
        self.entries.deinit(self.allocator);
        self.queue.deinit(self.allocator);
        self.allocator.destroy(self);
    }

    /// Starts a build: tiles not asked for from now on are stale.
    pub fn beginFrame(self: *Renderer) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.frame += 1;
        // drop queued work nobody asked for in the previous build
        var i: usize = 0;
        while (i < self.queue.items.len) {
            const key = self.queue.items[i];
            const e = self.entries.getPtr(key) orelse {
                _ = self.queue.orderedRemove(i);
                continue;
            };
            if (e.used + 1 < self.frame) {
                e.queued = false;
                if (e.image == null) _ = self.entries.remove(key);
                _ = self.queue.orderedRemove(i);
            } else i += 1;
        }
    }

    /// The tile's image, or null while it renders (it is queued).
    pub fn tile(self: *Renderer, key: Key) ?image.Image {
        self.mutex.lock();
        defer self.mutex.unlock();
        const gop = self.entries.getOrPut(self.allocator, key) catch return null;
        if (!gop.found_existing) gop.value_ptr.* = .{};
        const e = gop.value_ptr;
        e.used = self.frame;
        if (e.image) |img| return img;
        if (!e.queued and !e.failed) {
            e.queued = true;
            self.queue.append(self.allocator, key) catch {
                e.queued = false;
                return null;
            };
            self.cond.signal();
        }
        return null;
    }

    /// A ready image of the page at any zoom (shown, scaled, while the
    /// sharp tiles render): the whole-page image closest to `page_w`.
    pub fn anyPageImage(self: *Renderer, page: u32, page_w: u32) ?image.Image {
        self.mutex.lock();
        defer self.mutex.unlock();
        var best: ?image.Image = null;
        var best_diff: u32 = std.math.maxInt(u32);
        var it = self.entries.iterator();
        while (it.next()) |kv| {
            const k = kv.key_ptr.*;
            if (k.page != page or k.tx != 0 or k.ty != 0) continue;
            const img = kv.value_ptr.image orelse continue;
            // whole-page images only (single tile pages)
            if (k.page_w > tile_size) continue;
            const diff = if (k.page_w > page_w) k.page_w - page_w else page_w - k.page_w;
            if (diff < best_diff) {
                best_diff = diff;
                best = img;
                kv.value_ptr.used = self.frame;
            }
        }
        return best;
    }

    fn pageHeightPx(self: *Renderer, page: u32, page_w: u32) u32 {
        const size = self.doc.pageSize(page);
        return @max(1, @as(u32, @intFromFloat(@round(@as(f32, @floatFromInt(page_w)) * size.height / size.width))));
    }

    /// Scaled page height in device pixels for a page width (tiles grid).
    pub fn pageHeightFor(self: *Renderer, page: u32, page_w: u32) u32 {
        return self.pageHeightPx(page, page_w);
    }

    fn worker(self: *Renderer) void {
        while (true) {
            self.mutex.lock();
            while (self.queue.items.len == 0 and !self.stop) self.cond.wait(&self.mutex);
            if (self.stop) {
                self.mutex.unlock();
                return;
            }
            const key = self.queue.pop().?;
            self.mutex.unlock();

            const page_h = self.pageHeightPx(key.page, key.page_w);
            const x = @as(u32, key.tx) * tile_size;
            const y = @as(u32, key.ty) * tile_size;
            const w = @min(tile_size, key.page_w -| x);
            const h = @min(tile_size, page_h -| y);
            const rendered = if (w > 0 and h > 0)
                self.doc.renderTile(std.heap.c_allocator, key.page, key.page_w, page_h, x, y, w, h)
            else
                null;

            self.mutex.lock();
            if (self.entries.getPtr(key)) |e| {
                e.queued = false;
                if (rendered) |img| {
                    e.image = img;
                    e.bytes = @as(usize, img.width) * img.height * 4;
                    self.bytes += e.bytes;
                } else e.failed = true;
            } else if (rendered) |img| img.deinit();
            self.evict();
            self.mutex.unlock();
            ui.postWake();
        }
    }

    /// Frees least recently used tiles while over budget (keeps tiles of
    /// the current frame). Called with the mutex held.
    fn evict(self: *Renderer) void {
        while (self.bytes > self.budget) {
            var oldest: ?Key = null;
            var oldest_used: u64 = std.math.maxInt(u64);
            var it = self.entries.iterator();
            while (it.next()) |kv| {
                const e = kv.value_ptr;
                if (e.image == null or e.used + 1 >= self.frame) continue;
                if (e.used < oldest_used) {
                    oldest_used = e.used;
                    oldest = kv.key_ptr.*;
                }
            }
            const key = oldest orelse return;
            const e = self.entries.fetchRemove(key).?.value;
            if (e.image) |img| img.deinit();
            self.bytes -= e.bytes;
        }
    }
};
