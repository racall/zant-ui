//! PDF annotations made in the viewer (EmbedPDF annotation plugin model):
//! text markup (highlight, underline, strikeout, squiggly) on selected
//! text, ink strokes, and sticky notes. The viewer keeps the list (with
//! undo / redo); apps read it to persist it and write it into a copy of the
//! PDF with `Viewer.saveAnnotated` (standard PDF annotations).
//!
//! Coordinates are points of the displayed page (top-left origin, y down).
const std = @import("std");
const text = @import("text.zig");

pub const Rect = text.Rect;
pub const Point = [2]f32;

pub const Kind = enum(u8) {
    highlight = 0,
    underline = 1,
    strikeout = 2,
    squiggly = 3,
    ink = 4,
    note = 5,

    pub fn isMarkup(self: Kind) bool {
        return @backingInt(self) <= @backingInt(Kind.squiggly);
    }
};

pub const Annotation = struct {
    id: u64,
    page: usize,
    kind: Kind,
    /// 0xAARRGGBB; alpha is the opacity.
    color: u32,
    /// Markup: one box per line. Note: the icon box.
    rects: []const Rect = &.{},
    /// Ink strokes.
    strokes: []const []const Point = &.{},
    /// Ink stroke width (points).
    width: f32 = 2,
    /// Comment / note text.
    contents: []const u8 = "",
    /// Text under a markup annotation (for lists and search).
    text: []const u8 = "",

    /// Bounding box on the page.
    pub fn bounds(self: *const Annotation) Rect {
        var out: ?Rect = null;
        for (self.rects) |r| out = if (out) |o| o.unite(r) else r;
        for (self.strokes) |stroke| for (stroke) |p| {
            const dot = Rect{ .x = p[0] - self.width / 2, .y = p[1] - self.width / 2, .width = self.width, .height = self.width };
            out = if (out) |o| o.unite(dot) else dot;
        };
        return out orelse .{};
    }

    /// True when (x, y) (page points) is on the annotation.
    pub fn hit(self: *const Annotation, x: f32, y: f32) bool {
        if (self.kind == .ink) {
            const tolerance = @max(4, self.width * 2);
            for (self.strokes) |stroke| {
                var i: usize = 0;
                while (i + 1 < stroke.len) : (i += 1) {
                    if (segmentDistance(x, y, stroke[i], stroke[i + 1]) <= tolerance) return true;
                }
                if (stroke.len == 1 and @abs(stroke[0][0] - x) + @abs(stroke[0][1] - y) <= tolerance) return true;
            }
            return false;
        }
        for (self.rects) |r| if (r.contains(x, y)) return true;
        return false;
    }

    fn clone(self: Annotation, allocator: std.mem.Allocator) !Annotation {
        var copy = self;
        copy.rects = try allocator.dupe(Rect, self.rects);
        const strokes = try allocator.alloc([]const Point, self.strokes.len);
        for (self.strokes, 0..) |s, i| strokes[i] = try allocator.dupe(Point, s);
        copy.strokes = strokes;
        copy.contents = try allocator.dupe(u8, self.contents);
        copy.text = try allocator.dupe(u8, self.text);
        return copy;
    }

    fn free(self: Annotation, allocator: std.mem.Allocator) void {
        allocator.free(self.rects);
        for (self.strokes) |s| allocator.free(s);
        allocator.free(self.strokes);
        allocator.free(self.contents);
        allocator.free(self.text);
    }
};

fn segmentDistance(x: f32, y: f32, a: Point, b: Point) f32 {
    const dx = b[0] - a[0];
    const dy = b[1] - a[1];
    const len2 = dx * dx + dy * dy;
    const t = if (len2 == 0) 0 else std.math.clamp(((x - a[0]) * dx + (y - a[1]) * dy) / len2, 0, 1);
    const px = a[0] + t * dx - x;
    const py = a[1] + t * dy - y;
    return @sqrt(px * px + py * py);
}

/// Owned annotation list with snapshot undo / redo (EmbedPDF history
/// plugin: every change is one step).
pub const Store = struct {
    allocator: std.mem.Allocator,
    items: std.ArrayListUnmanaged(Annotation) = .empty,
    undo_stack: std.ArrayListUnmanaged([]Annotation) = .empty,
    redo_stack: std.ArrayListUnmanaged([]Annotation) = .empty,
    next_id: u64 = 1,
    /// Bumped on every change (apps compare it to know when to save).
    revision: u64 = 0,

    pub fn init(allocator: std.mem.Allocator) Store {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Store) void {
        self.clearItems();
        self.items.deinit(self.allocator);
        self.dropStack(&self.undo_stack);
        self.dropStack(&self.redo_stack);
        self.undo_stack.deinit(self.allocator);
        self.redo_stack.deinit(self.allocator);
    }

    fn clearItems(self: *Store) void {
        for (self.items.items) |a| a.free(self.allocator);
        self.items.clearRetainingCapacity();
    }

    fn dropStack(self: *Store, stack: *std.ArrayListUnmanaged([]Annotation)) void {
        for (stack.items) |snap| self.freeSnapshot(snap);
        stack.clearRetainingCapacity();
    }

    fn freeSnapshot(self: *Store, snap: []Annotation) void {
        for (snap) |a| a.free(self.allocator);
        self.allocator.free(snap);
    }

    fn snapshot(self: *Store) ![]Annotation {
        const snap = try self.allocator.alloc(Annotation, self.items.items.len);
        for (self.items.items, 0..) |a, i| snap[i] = try a.clone(self.allocator);
        return snap;
    }

    fn restore(self: *Store, snap: []Annotation) !void {
        self.clearItems();
        for (snap) |a| try self.items.append(self.allocator, try a.clone(self.allocator));
        self.revision += 1;
    }

    /// Records the current list as an undo step (before a change).
    fn checkpoint(self: *Store) !void {
        try self.undo_stack.append(self.allocator, try self.snapshot());
        if (self.undo_stack.items.len > 100) self.freeSnapshot(self.undo_stack.orderedRemove(0));
        self.dropStack(&self.redo_stack);
    }

    pub fn all(self: *const Store) []const Annotation {
        return self.items.items;
    }

    pub fn get(self: *const Store, id: u64) ?*const Annotation {
        for (self.items.items) |*a| if (a.id == id) return a;
        return null;
    }

    /// Adds a copy of `annotation` (its id is assigned); returns the id.
    pub fn add(self: *Store, annotation: Annotation) !u64 {
        try self.checkpoint();
        var copy = try annotation.clone(self.allocator);
        copy.id = self.next_id;
        self.next_id += 1;
        try self.items.append(self.allocator, copy);
        self.revision += 1;
        return copy.id;
    }

    pub fn remove(self: *Store, id: u64) bool {
        for (self.items.items, 0..) |a, i| if (a.id == id) {
            self.checkpoint() catch return false;
            a.free(self.allocator);
            _ = self.items.orderedRemove(i);
            self.revision += 1;
            return true;
        };
        return false;
    }

    pub fn setContents(self: *Store, id: u64, contents: []const u8) !void {
        for (self.items.items) |*a| if (a.id == id) {
            if (std.mem.eql(u8, a.contents, contents)) return;
            try self.checkpoint();
            const owned = try self.allocator.dupe(u8, contents);
            self.allocator.free(a.contents);
            a.contents = owned;
            self.revision += 1;
            return;
        };
    }

    pub fn setColor(self: *Store, id: u64, color: u32) !void {
        for (self.items.items) |*a| if (a.id == id) {
            if (a.color == color) return;
            try self.checkpoint();
            a.color = color;
            self.revision += 1;
            return;
        };
    }

    /// Replaces the whole list (loading saved annotations); clears history.
    pub fn load(self: *Store, list: []const Annotation) !void {
        self.clearItems();
        self.dropStack(&self.undo_stack);
        self.dropStack(&self.redo_stack);
        for (list) |a| {
            const copy = try a.clone(self.allocator);
            try self.items.append(self.allocator, copy);
            self.next_id = @max(self.next_id, a.id + 1);
        }
        self.revision += 1;
    }

    pub fn canUndo(self: *const Store) bool {
        return self.undo_stack.items.len > 0;
    }

    pub fn canRedo(self: *const Store) bool {
        return self.redo_stack.items.len > 0;
    }

    pub fn undo(self: *Store) bool {
        const snap = self.undo_stack.pop() orelse return false;
        defer self.freeSnapshot(snap);
        const now = self.snapshot() catch return false;
        self.redo_stack.append(self.allocator, now) catch {
            self.freeSnapshot(now);
            return false;
        };
        self.restore(snap) catch return false;
        return true;
    }

    pub fn redo(self: *Store) bool {
        const snap = self.redo_stack.pop() orelse return false;
        defer self.freeSnapshot(snap);
        const now = self.snapshot() catch return false;
        self.undo_stack.append(self.allocator, now) catch {
            self.freeSnapshot(now);
            return false;
        };
        self.restore(snap) catch return false;
        return true;
    }
};

/// Default colours (EmbedPDF / Acrobat-like palette).
pub const colors = struct {
    pub const yellow: u32 = 0x80ffcd45;
    pub const green: u32 = 0x8057d975;
    pub const blue: u32 = 0x8052a8ff;
    pub const pink: u32 = 0x80ff6b9e;
    pub const red: u32 = 0xffe5484d;
    pub const ink: u32 = 0xffe5484d;
    pub const note: u32 = 0xffffc53d;
};
