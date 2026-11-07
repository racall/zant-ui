//! An open PDF document: page sizes, text geometry, links and outline,
//! backed by PDFKit (macOS) or PDFium (Windows).
//!
//! Text, links and the outline are loaded lazily and cached; they are for
//! the UI thread. `renderTile` is thread-safe (the viewer's render worker
//! calls it).
const std = @import("std");
const native = @import("native.zig");
const text = @import("text.zig");
const image = @import("../image.zig");

pub const Size = struct { width: f32, height: f32 };

pub const Link = struct {
    /// Area on the page (points, top-left origin).
    rect: text.Rect,
    /// Internal target page (null for external links).
    dest_page: ?usize = null,
    /// Target y on `dest_page` in points from the top (null: page top).
    dest_y: ?f32 = null,
    /// External URL ("" for internal links).
    uri: []const u8 = "",
};

pub const OutlineEntry = struct {
    depth: u32,
    page: ?usize,
    title: []const u8,
};

pub const OpenError = error{ FileNotFound, PasswordRequired, OutOfMemory };

pub const Document = struct {
    allocator: std.mem.Allocator,
    handle: *native.Doc,
    sizes: []Size,
    texts: []?text.PageText,
    links: []?[]Link,
    outline_items: ?[]OutlineEntry = null,
    /// Owns link URIs and outline titles.
    strings: std.heap.ArenaAllocator,

    /// Opens a PDF file. `password` unlocks encrypted documents.
    pub fn open(allocator: std.mem.Allocator, path: []const u8, password: ?[]const u8) OpenError!*Document {
        const path_z = try allocator.dupeSentinel(u8, path, 0);
        defer allocator.free(path_z);
        const pw_z: ?[:0]u8 = if (password) |pw| try allocator.dupeSentinel(u8, pw, 0) else null;
        defer if (pw_z) |pw| allocator.free(pw);
        var locked: c_int = 0;
        const handle = native.zant_pdf_open(path_z.ptr, if (pw_z) |pw| pw.ptr else null, &locked) orelse
            return if (locked != 0) error.PasswordRequired else error.FileNotFound;
        errdefer native.zant_pdf_close(handle);
        const n: usize = @intCast(@max(0, native.zant_pdf_count(handle)));
        const doc = try allocator.create(Document);
        errdefer allocator.destroy(doc);
        const sizes = try allocator.alloc(Size, n);
        errdefer allocator.free(sizes);
        for (sizes, 0..) |*size, i| {
            var w: f32 = 612;
            var h: f32 = 792;
            if (native.zant_pdf_size(handle, @intCast(i), &w, &h) != 0 or w <= 0 or h <= 0) {
                w = 612;
                h = 792;
            }
            size.* = .{ .width = w, .height = h };
        }
        const texts = try allocator.alloc(?text.PageText, n);
        errdefer allocator.free(texts);
        @memset(texts, null);
        const links = try allocator.alloc(?[]Link, n);
        @memset(links, null);
        doc.* = .{
            .allocator = allocator,
            .handle = handle,
            .sizes = sizes,
            .texts = texts,
            .links = links,
            .strings = std.heap.ArenaAllocator.init(allocator),
        };
        return doc;
    }

    /// Closes the document. Stop render workers using it first.
    pub fn close(self: *Document) void {
        for (self.texts) |*t| if (t.*) |*pt| pt.deinit(self.allocator);
        for (self.links) |l| if (l) |items| self.allocator.free(items);
        if (self.outline_items) |items| self.allocator.free(items);
        self.allocator.free(self.texts);
        self.allocator.free(self.links);
        self.allocator.free(self.sizes);
        self.strings.deinit();
        native.zant_pdf_close(self.handle);
        const allocator = self.allocator;
        allocator.destroy(self);
    }

    pub fn pageCount(self: *const Document) usize {
        return self.sizes.len;
    }

    /// Displayed page size in points (rotation applied).
    pub fn pageSize(self: *const Document, page: usize) Size {
        return self.sizes[page];
    }

    /// Text geometry of a page (loaded on first use; null when the page has
    /// no text layer or cannot be read).
    pub fn pageText(self: *Document, page: usize) ?*const text.PageText {
        if (page >= self.texts.len) return null;
        if (self.texts[page] == null) {
            self.texts[page] = self.loadText(page) catch text.PageText{};
        }
        return &self.texts[page].?;
    }

    fn loadText(self: *Document, page: usize) !text.PageText {
        const n = native.zant_pdf_glyphs(self.handle, @intCast(page), null, 0);
        if (n <= 0) return .{};
        const raw = try self.allocator.alloc(native.Glyph, @intCast(n));
        defer self.allocator.free(raw);
        const got = native.zant_pdf_glyphs(self.handle, @intCast(page), raw.ptr, n);
        if (got <= 0) return .{};
        return text.PageText.build(self.allocator, raw[0..@intCast(@min(got, n))]);
    }

    /// Links of a page (loaded on first use).
    pub fn pageLinks(self: *Document, page: usize) []const Link {
        if (page >= self.links.len) return &.{};
        if (self.links[page] == null) self.links[page] = self.loadLinks(page) catch &.{};
        return self.links[page].?;
    }

    fn loadLinks(self: *Document, page: usize) ![]Link {
        const n = native.zant_pdf_links(self.handle, @intCast(page), null, 0);
        if (n <= 0) return &.{};
        const raw = try self.allocator.alloc(native.Link, @intCast(n));
        defer self.allocator.free(raw);
        const got: usize = @intCast(@max(0, @min(n, native.zant_pdf_links(self.handle, @intCast(page), raw.ptr, n))));
        const out = try self.allocator.alloc(Link, got);
        for (raw[0..got], out) |l, *o| {
            const uri = std.mem.sliceTo(&l.uri, 0);
            o.* = .{
                .rect = .{ .x = l.x, .y = l.y, .width = l.width, .height = l.height },
                .dest_page = if (l.dest_page >= 0 and @as(usize, @intCast(l.dest_page)) < self.sizes.len) @intCast(l.dest_page) else null,
                .dest_y = if (l.dest_y >= 0) l.dest_y else null,
                .uri = if (uri.len > 0) try self.strings.allocator().dupe(u8, uri) else "",
            };
        }
        return out;
    }

    /// Bookmarks, flattened depth-first (loaded on first use).
    pub fn outline(self: *Document) []const OutlineEntry {
        if (self.outline_items == null) self.outline_items = self.loadOutline() catch &.{};
        return self.outline_items.?;
    }

    fn loadOutline(self: *Document) ![]OutlineEntry {
        const n = native.zant_pdf_outline(self.handle, null, 0);
        if (n <= 0) return &.{};
        const raw = try self.allocator.alloc(native.OutlineItem, @intCast(n));
        defer self.allocator.free(raw);
        const got: usize = @intCast(@max(0, @min(n, native.zant_pdf_outline(self.handle, raw.ptr, n))));
        const out = try self.allocator.alloc(OutlineEntry, got);
        for (raw[0..got], out) |item, *o| o.* = .{
            .depth = @intCast(@max(0, item.depth)),
            .page = if (item.page >= 0 and @as(usize, @intCast(item.page)) < self.sizes.len) @intCast(item.page) else null,
            .title = try self.strings.allocator().dupe(u8, std.mem.sliceTo(&item.title, 0)),
        };
        return out;
    }

    /// UTF-8 text of a page.
    pub fn pageTextString(self: *Document, allocator: std.mem.Allocator, page: usize) ![]u8 {
        var out: std.ArrayListUnmanaged(u8) = .empty;
        errdefer out.deinit(allocator);
        if (self.pageText(page)) |t| if (t.count() > 0) try t.appendText(allocator, &out, 0, t.count() - 1);
        return out.toOwnedSlice(allocator);
    }

    /// Renders region (x, y, w, h) of the page scaled to `page_w` × `page_h`
    /// pixels. Thread-safe.
    pub fn renderTile(self: *Document, allocator: std.mem.Allocator, page: usize, page_w: u32, page_h: u32, x: u32, y: u32, w: u32, h: u32) ?image.Image {
        if (w == 0 or h == 0) return null;
        const stride = w * 4;
        const pixels = allocator.alloc(u8, @as(usize, stride) * h) catch return null;
        defer allocator.free(pixels);
        if (native.zant_pdf_render(self.handle, @intCast(page), @intCast(page_w), @intCast(page_h), @intCast(x), @intCast(y), @intCast(w), @intCast(h), pixels.ptr, @intCast(stride)) != 0) return null;
        return image.Image.fromPixels(pixels, w, h, stride);
    }
};
