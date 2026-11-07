//! Page text geometry: glyph runs, hit testing, selection rectangles, word /
//! line expansion, text extraction and search.
//!
//! Ported from EmbedPDF's selection plugin (`plugin-selection/src/lib/
//! utils.ts`, MIT), which adapts PDFium's `CPDF_TextPage::GetIndexAtPos` and
//! Chromium's PDF text selection (`pdf/pdfium/pdfium_range.cc`,
//! `PDFiumEngine::OnMultipleClick`, BSD-3-Clause). Coordinates are points
//! of the displayed page (top-left origin, y down).
const std = @import("std");
const native = @import("native.zig");

pub const Rect = struct {
    x: f32 = 0,
    y: f32 = 0,
    width: f32 = 0,
    height: f32 = 0,

    pub fn right(self: Rect) f32 {
        return self.x + self.width;
    }

    pub fn bottom(self: Rect) f32 {
        return self.y + self.height;
    }

    pub fn isEmpty(self: Rect) bool {
        return self.width <= 0 or self.height <= 0;
    }

    pub fn contains(self: Rect, x: f32, y: f32) bool {
        return x >= self.x and x <= self.right() and y >= self.y and y <= self.bottom();
    }

    pub fn unite(a: Rect, b: Rect) Rect {
        const l = @min(a.x, b.x);
        const t = @min(a.y, b.y);
        return .{ .x = l, .y = t, .width = @max(a.right(), b.right()) - l, .height = @max(a.bottom(), b.bottom()) - t };
    }

    pub fn intersect(a: Rect, b: Rect) Rect {
        const l = @max(a.x, b.x);
        const t = @max(a.y, b.y);
        return .{ .x = l, .y = t, .width = @max(0, @min(a.right(), b.right()) - l), .height = @max(0, @min(a.bottom(), b.bottom()) - t) };
    }
};

pub const GlyphKind = enum(u8) { text = 0, space = 1, empty = 2 };

pub const Glyph = struct {
    /// Loose box (line height): highlight rectangles.
    box: Rect,
    /// Tight (ink) box: hit testing.
    tight: Rect,
    codepoint: u21,
    kind: GlyphKind,
};

pub const Run = struct {
    rect: Rect,
    char_start: u32,
    len: u32,
    font_size: f32,
};

/// Inclusive glyph index range on one page.
pub const Slice = struct { from: usize, to: usize };

pub const PageText = struct {
    glyphs: []Glyph = &.{},
    runs: []Run = &.{},

    pub fn deinit(self: *PageText, allocator: std.mem.Allocator) void {
        allocator.free(self.glyphs);
        allocator.free(self.runs);
        self.* = .{};
    }

    /// Builds glyphs and visual runs from the native glyph stream
    /// (EmbedPDF `buildRunsFromGlyphs`). Backends that report text objects
    /// (`run >= 0`, PDFium) group by object; others group by geometry.
    pub fn build(allocator: std.mem.Allocator, raw: []const native.Glyph) !PageText {
        const glyphs = try allocator.alloc(Glyph, raw.len);
        errdefer allocator.free(glyphs);
        for (raw, glyphs) |g, *out| {
            const kind: GlyphKind = switch (g.flags) {
                1 => .space,
                2 => .empty,
                else => .text,
            };
            out.* = .{
                .box = .{ .x = g.x, .y = g.y, .width = g.width, .height = g.height },
                .tight = .{ .x = g.tight_x, .y = g.tight_y, .width = g.tight_width, .height = g.tight_height },
                .codepoint = if (g.codepoint <= 0x10FFFF) @intCast(g.codepoint) else 0xFFFD,
                .kind = kind,
            };
        }
        var runs: std.ArrayListUnmanaged(Run) = .empty;
        errdefer runs.deinit(allocator);
        var cur: ?usize = null;
        var cur_obj: c_int = std.math.minInt(c_int);
        var has_box = false;
        for (raw, glyphs, 0..) |g, glyph, i| {
            const start_new = blk: {
                const c = cur orelse break :blk true;
                if (g.run >= 0) break :blk g.run != cur_obj;
                // geometry: a new visual line or a jump back starts a run
                if (glyph.kind == .empty or !has_box) break :blk false;
                const r = runs.items[c].rect;
                if (verticalOverlap(r, glyph.box) < 0.5) break :blk true;
                const avg = r.width / @as(f32, @floatFromInt(@max(1, runs.items[c].len)));
                if (glyph.box.x + avg * 0.5 < r.x) break :blk true;
                if (g.font_size > 0 and runs.items[c].font_size > 0 and
                    @max(g.font_size, runs.items[c].font_size) / @min(g.font_size, runs.items[c].font_size) > 1.5) break :blk true;
                break :blk false;
            };
            if (start_new) {
                try runs.append(allocator, .{ .rect = .{}, .char_start = @intCast(i), .len = 0, .font_size = g.font_size });
                cur = runs.items.len - 1;
                cur_obj = g.run;
                has_box = false;
            }
            const run = &runs.items[cur.?];
            run.len += 1;
            if (glyph.kind == .empty) continue;
            run.rect = if (has_box) run.rect.unite(glyph.box) else glyph.box;
            if (run.font_size <= 0) run.font_size = g.font_size;
            has_box = true;
        }
        return .{ .glyphs = glyphs, .runs = try runs.toOwnedSlice(allocator) };
    }

    pub fn count(self: *const PageText) usize {
        return self.glyphs.len;
    }

    /// Glyph under a point, with PDFium's tolerance fallback (EmbedPDF
    /// `glyphAt`): exact tight-box match first, then the nearest glyph whose
    /// box expanded by `tolerance_factor` × average glyph height contains it.
    pub fn glyphAt(self: *const PageText, x: f32, y: f32, tolerance_factor: f32) ?usize {
        for (self.runs) |run| {
            if (!run.rect.contains(x, y)) continue;
            for (self.glyphs[run.char_start..][0..run.len], 0..) |g, k| {
                if (g.kind == .empty) continue;
                if (g.tight.contains(x, y)) return run.char_start + k;
            }
        }
        if (tolerance_factor <= 0) return null;
        const half = self.tolerance(tolerance_factor) / 2;
        var best: ?usize = null;
        var best_dist = std.math.inf(f32);
        for (self.runs) |run| {
            const r = run.rect;
            if (y < r.y - half or y > r.bottom() + half or x < r.x - half or x > r.right() + half) continue;
            for (self.glyphs[run.char_start..][0..run.len], 0..) |g, k| {
                if (g.kind == .empty) continue;
                const t = g.tight;
                if (x < t.x - half or x > t.right() + half or y < t.y - half or y > t.bottom() + half) continue;
                const dx = @min(@abs(x - t.x), @abs(x - t.right()));
                const dy = @min(@abs(y - t.y), @abs(y - t.bottom()));
                if (dx + dy < best_dist) {
                    best_dist = dx + dy;
                    best = run.char_start + k;
                }
            }
        }
        return best;
    }

    fn tolerance(self: *const PageText, factor: f32) f32 {
        var total: f32 = 0;
        var n: f32 = 0;
        for (self.glyphs) |g| {
            if (g.kind == .empty) continue;
            total += g.box.height;
            n += 1;
        }
        return if (n == 0) 0 else total / n * factor;
    }

    /// Nearest glyph boundary for a drag that is not over text: the glyph
    /// on the closest line, left or right of the point. Selections started
    /// or continued over margins snap to it (like Chromium / Preview).
    pub fn nearestGlyph(self: *const PageText, x: f32, y: f32) ?usize {
        var best: ?usize = null;
        var best_score = std.math.inf(f32);
        for (self.glyphs, 0..) |g, i| {
            if (g.kind == .empty) continue;
            const b = g.box;
            const dy = if (y < b.y) b.y - y else if (y > b.bottom()) y - b.bottom() else 0;
            const dx = if (x < b.x) b.x - x else if (x > b.right()) x - b.right() else 0;
            // lines matter more than columns
            const score = dy * 4 + dx;
            if (score < best_score) {
                best_score = score;
                best = i;
            }
        }
        return best;
    }

    /// Highlight rectangles for glyphs `from..to` (inclusive), split at big
    /// gaps and merged across runs on the same line (EmbedPDF
    /// `rectsWithinSlice` + Chromium `MergeAdjacentRects`).
    pub fn rects(self: *const PageText, allocator: std.mem.Allocator, from: usize, to: usize) ![]Rect {
        var parts: std.ArrayListUnmanaged(Part) = .empty;
        defer parts.deinit(allocator);
        const char_distance_factor: f32 = 2.5;
        for (self.runs) |run| {
            const run_end = run.char_start + run.len;
            if (run.len == 0 or run_end <= from or run.char_start > to) continue;
            const s = @max(from, run.char_start);
            const e = @min(to, run_end - 1);
            var acc: ?Rect = null;
            var n: u32 = 0;
            var width_sum: f32 = 0;
            var prev_right: f32 = -std.math.inf(f32);
            var i = s;
            while (i <= e) : (i += 1) {
                const g = self.glyphs[i];
                if (g.kind == .empty) continue;
                if (n > 0) {
                    const avg = width_sum / @as(f32, @floatFromInt(n));
                    if (avg > 0 and @abs(g.box.x - prev_right) > char_distance_factor * avg) {
                        try parts.append(allocator, .{ .rect = acc.?, .chars = n, .font_size = run.font_size });
                        acc = null;
                        n = 0;
                        width_sum = 0;
                    }
                }
                acc = if (acc) |a| a.unite(g.box) else g.box;
                n += 1;
                width_sum += g.box.width;
                prev_right = g.box.right();
            }
            if (acc) |a| try parts.append(allocator, .{ .rect = a, .chars = n, .font_size = run.font_size });
        }
        var out: std.ArrayListUnmanaged(Rect) = .empty;
        errdefer out.deinit(allocator);
        var current: ?Rect = null;
        var previous: ?Part = null;
        for (parts.items) |part| {
            if (previous != null and current != null) {
                if (shouldMerge(previous.?, part)) {
                    current = current.?.unite(part.rect);
                } else {
                    try out.append(allocator, current.?);
                    current = part.rect;
                }
            } else current = part.rect;
            previous = part;
        }
        if (current) |c| if (!c.isEmpty()) try out.append(allocator, c);
        return out.toOwnedSlice(allocator);
    }

    const Part = struct { rect: Rect, chars: u32, font_size: f32 };

    fn shouldMerge(a: Part, b: Part) bool {
        if (a.font_size > 0 and b.font_size > 0 and @max(a.font_size, b.font_size) / @min(a.font_size, b.font_size) > 1.5) return false;
        if (verticalOverlap(a.rect, b.rect) < 0.8) return false;
        const aw = a.rect.width / @as(f32, @floatFromInt(a.chars));
        const bw = b.rect.width / @as(f32, @floatFromInt(b.chars));
        return a.rect.x - aw < b.rect.right() + bw and a.rect.right() + aw > b.rect.x - bw;
    }

    fn isBoundary(self: *const PageText, i: usize) bool {
        const g = self.glyphs[i];
        return g.kind != .text or isPunctuationBoundary(g.codepoint);
    }

    /// Word around glyph `i` (double click; EmbedPDF `expandToWordBoundary`).
    pub fn wordAt(self: *const PageText, i: usize) ?Slice {
        if (i >= self.glyphs.len) return null;
        if (self.isBoundary(i)) return .{ .from = i, .to = i };
        var from = i;
        while (from > 0 and !self.isBoundary(from - 1)) from -= 1;
        var to = i;
        while (to + 1 < self.glyphs.len and !self.isBoundary(to + 1)) to += 1;
        return .{ .from = from, .to = to };
    }

    /// Visual line around glyph `i` (triple click; `expandToLineBoundary`).
    pub fn lineAt(self: *const PageText, i: usize) ?Slice {
        const r = self.runOf(i) orelse return null;
        const anchor = self.runs[r].rect;
        var from: usize = self.runs[r].char_start;
        var to: usize = self.runs[r].char_start + self.runs[r].len - 1;
        var k = r;
        while (k > 0) {
            k -= 1;
            const run = self.runs[k];
            if (run.rect.isEmpty()) continue;
            if (verticalOverlap(run.rect, anchor) < 0.5) break;
            from = run.char_start;
        }
        k = r + 1;
        while (k < self.runs.len) : (k += 1) {
            const run = self.runs[k];
            if (run.rect.isEmpty()) continue;
            if (verticalOverlap(run.rect, anchor) < 0.5) break;
            to = run.char_start + run.len - 1;
        }
        return .{ .from = from, .to = to };
    }

    fn runOf(self: *const PageText, i: usize) ?usize {
        for (self.runs, 0..) |run, k| {
            if (i >= run.char_start and i < run.char_start + run.len) return k;
        }
        return null;
    }

    /// UTF-8 text of glyphs `from..to` (inclusive). Generated line breaks
    /// become "\n" (CR dropped, as on macOS).
    pub fn appendText(self: *const PageText, allocator: std.mem.Allocator, out: *std.ArrayListUnmanaged(u8), from: usize, to: usize) !void {
        if (self.glyphs.len == 0 or from > to) return;
        var i = from;
        const end = @min(to, self.glyphs.len - 1);
        while (i <= end) : (i += 1) {
            const cp = self.glyphs[i].codepoint;
            if (cp == 0 or cp == '\r') continue;
            var buf: [4]u8 = undefined;
            const n = std.unicode.utf8Encode(cp, &buf) catch continue;
            try out.appendSlice(allocator, buf[0..n]);
        }
    }

    /// Case-insensitive matches of `query` (whitespace runs match any
    /// whitespace, including line breaks). Appends inclusive glyph ranges.
    pub fn search(self: *const PageText, allocator: std.mem.Allocator, query: []const u8, out: *std.ArrayListUnmanaged(Slice)) !void {
        var needle: std.ArrayListUnmanaged(u21) = .empty;
        defer needle.deinit(allocator);
        var it = (std.unicode.Utf8View.init(query) catch return).iterator();
        var last_space = true;
        while (it.nextCodepoint()) |cp| {
            const space = isSpace(cp);
            if (space and last_space) continue;
            try needle.append(allocator, if (space) ' ' else fold(cp));
            last_space = space;
        }
        while (needle.items.len > 0 and needle.items[needle.items.len - 1] == ' ') needle.items.len -= 1;
        if (needle.items.len == 0) return;
        var start: usize = 0;
        while (start < self.glyphs.len) {
            if (self.matchesAt(start, needle.items)) |end| {
                try out.append(allocator, .{ .from = start, .to = end });
                start = end + 1;
            } else start += 1;
        }
    }

    /// Last glyph of a match of `needle` starting at glyph `start`.
    fn matchesAt(self: *const PageText, start: usize, needle: []const u21) ?usize {
        const first = self.glyphs[start].codepoint;
        if (first == 0 or isSpace(first)) return null;
        var i = start;
        var k: usize = 0;
        while (k < needle.len) {
            if (i >= self.glyphs.len) return null;
            const cp = self.glyphs[i].codepoint;
            if (cp == 0) { // second half of a UTF-16 pair
                i += 1;
                continue;
            }
            if (needle[k] == ' ') {
                if (!isSpace(cp)) return null;
                while (i < self.glyphs.len and (self.glyphs[i].codepoint == 0 or isSpace(self.glyphs[i].codepoint))) i += 1;
                k += 1;
                continue;
            }
            if (fold(cp) != needle[k]) return null;
            i += 1;
            k += 1;
        }
        return i - 1;
    }
};

fn verticalOverlap(a: Rect, b: Rect) f32 {
    if (a.isEmpty() or b.isEmpty()) return 0;
    const u = a.unite(b);
    if (u.height == a.height or u.height == b.height) return 1;
    return a.intersect(b).height / u.height;
}

fn isSpace(cp: u21) bool {
    return cp == ' ' or cp == '\t' or cp == '\n' or cp == '\r' or cp == 0xA0 or cp == 0x3000;
}

/// Word boundaries besides spaces (PDFium `IsWordBoundary` style).
fn isPunctuationBoundary(cp: u21) bool {
    return switch (cp) {
        '.', ',', ';', ':', '!', '?', '(', ')', '[', ']', '{', '}', '"', '\'', '/', '\\', '<', '>', '|' => true,
        0x3001, 0x3002, 0xFF0C, 0xFF0E, 0xFF1A, 0xFF1B, 0xFF01, 0xFF1F => true, // CJK punctuation
        else => false,
    };
}

/// Simple case folding: ASCII, Latin-1, Greek and Cyrillic, full-width ASCII.
pub fn fold(cp: u21) u21 {
    return switch (cp) {
        'A'...'Z' => cp + 32,
        0xC0...0xD6, 0xD8...0xDE => cp + 32,
        0x391...0x3A1, 0x3A3...0x3AB => cp + 32,
        0x410...0x42F => cp + 32,
        0x400...0x40F => cp + 80,
        0xFF21...0xFF3A => cp - 0xFF21 + 'a',
        0xFF41...0xFF5A => cp - 0xFF41 + 'a',
        0xFF10...0xFF19 => cp - 0xFF10 + '0',
        else => cp,
    };
}
