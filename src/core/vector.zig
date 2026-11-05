//! Retained vector drawings: glyph runs, rules, lines and paths positioned
//! relative to a baseline origin. Used by `ui.vector` nodes (math blocks,
//! diagrams) and by rich-text inline embeds (inline math).
//!
//! Coordinates: x grows right from the drawing's left edge, y grows down
//! from the baseline (so glyphs above the baseline have negative y).
const std = @import("std");
const geometry = @import("geometry.zig");
const text_mod = @import("../renderer/text.zig");

pub const Point = [2]f32;

pub const Text = struct {
    x: f32,
    /// Baseline y.
    y: f32,
    text: []const u8,
    size: f32,
    family: text_mod.Family = .math,
    weight: u16 = 400,
    italic: bool = false,
    color: u32,
    /// Vertical stretch around the baseline (stretchy delimiters).
    scale_y: f32 = 1,
};

pub const Rect = struct { x: f32, y: f32, width: f32, height: f32, color: u32, radius: f32 = 0 };

pub const Line = struct { x0: f32, y0: f32, x1: f32, y1: f32, color: u32, width: f32 = 1 };

pub const Path = struct {
    points: []const Point,
    color: u32,
    /// 0 fills the polygon; > 0 strokes it.
    stroke_width: f32 = 0,
    closed: bool = false,
};

pub const Command = union(enum) {
    text: Text,
    rect: Rect,
    line: Line,
    path: Path,
};

pub const Drawing = struct {
    width: f32,
    /// Extent above the baseline.
    ascent: f32,
    /// Extent below the baseline.
    descent: f32,
    commands: []const Command,
    /// Content hash used to skip re-copying unchanged drawings; 0 = compute.
    hash: u64 = 0,

    pub fn height(self: Drawing) f32 {
        return self.ascent + self.descent;
    }

    pub fn contentHash(self: *const Drawing) u64 {
        if (self.hash != 0) return self.hash;
        var h = std.hash.Wyhash.init(0x5eed);
        h.update(std.mem.asBytes(&self.width));
        h.update(std.mem.asBytes(&self.ascent));
        h.update(std.mem.asBytes(&self.descent));
        for (self.commands) |cmd| hashCommand(&h, cmd);
        return h.final();
    }

    /// The drawing uniformly scaled by `factor` (positions, sizes, stroke
    /// widths); command text and point slices are allocated in `allocator`.
    pub fn scaled(self: *const Drawing, allocator: std.mem.Allocator, factor: f32) !Drawing {
        const commands = try allocator.alloc(Command, self.commands.len);
        for (self.commands, 0..) |cmd, i| {
            commands[i] = switch (cmd) {
                .text => |t| blk: {
                    var v = t;
                    v.x *= factor;
                    v.y *= factor;
                    v.size *= factor;
                    break :blk .{ .text = v };
                },
                .rect => |r| .{ .rect = .{ .x = r.x * factor, .y = r.y * factor, .width = r.width * factor, .height = r.height * factor, .color = r.color, .radius = r.radius * factor } },
                .line => |l| .{ .line = .{ .x0 = l.x0 * factor, .y0 = l.y0 * factor, .x1 = l.x1 * factor, .y1 = l.y1 * factor, .color = l.color, .width = @max(0.5, l.width * factor) } },
                .path => |pth| blk: {
                    const pts = try allocator.alloc(Point, pth.points.len);
                    for (pth.points, 0..) |p, k| pts[k] = .{ p[0] * factor, p[1] * factor };
                    var v = pth;
                    v.points = pts;
                    v.stroke_width = if (pth.stroke_width > 0) @max(0.5, pth.stroke_width * factor) else 0;
                    break :blk .{ .path = v };
                },
            };
        }
        return .{ .width = self.width * factor, .ascent = self.ascent * factor, .descent = self.descent * factor, .commands = commands };
    }

    /// Deep copy into `allocator`.
    pub fn clone(self: *const Drawing, allocator: std.mem.Allocator) !*Drawing {
        const out = try allocator.create(Drawing);
        const commands = try allocator.alloc(Command, self.commands.len);
        for (self.commands, 0..) |cmd, i| {
            commands[i] = switch (cmd) {
                .text => |t| blk: {
                    var copy = t;
                    copy.text = try allocator.dupe(u8, t.text);
                    break :blk .{ .text = copy };
                },
                .path => |p| blk: {
                    var copy = p;
                    copy.points = try allocator.dupe(Point, p.points);
                    break :blk .{ .path = copy };
                },
                else => cmd,
            };
        }
        out.* = .{
            .width = self.width,
            .ascent = self.ascent,
            .descent = self.descent,
            .commands = commands,
            .hash = self.contentHash(),
        };
        return out;
    }
};

fn hashCommand(h: *std.hash.Wyhash, cmd: Command) void {
    h.update(&.{@backingInt(std.meta.activeTag(cmd))});
    switch (cmd) {
        .text => |t| {
            h.update(std.mem.asBytes(&[_]f32{ t.x, t.y, t.size, t.scale_y }));
            h.update(std.mem.asBytes(&t.color));
            h.update(std.mem.asBytes(&t.weight));
            h.update(&.{ @as(u8, @intCast(@backingInt(t.family))), @intFromBool(t.italic) });
            h.update(t.text);
        },
        .rect => |r| h.update(std.mem.asBytes(&r)),
        .line => |l| h.update(std.mem.asBytes(&l)),
        .path => |p| {
            h.update(std.mem.asBytes(&p.color));
            h.update(std.mem.asBytes(&p.stroke_width));
            h.update(&.{@intFromBool(p.closed)});
            h.update(std.mem.sliceAsBytes(p.points));
        },
    }
}

/// Glyph-ink metrics of a single-line run (see `zant_text_measure_ink`).
pub const InkMetrics = extern struct {
    advance: f32,
    ascent: f32,
    descent: f32,
    font_ascent: f32,
    font_descent: f32,
    /// Signed ink bounds relative to the baseline (y grows down).
    ink_top: f32,
    ink_bottom: f32,
};

extern fn zant_text_measure_ink(utf8: [*]const u8, length: usize, size: f32, family: c_int, weight: c_int, italic: c_int) InkMetrics;
extern fn zant_canvas_draw_simple_text(canvas: ?*anyopaque, utf8: [*]const u8, length: usize, x: f32, y: f32, size: f32, family: c_int, weight: c_int, italic: c_int, color: u32, scale_y: f32) void;
extern fn zant_canvas_fill_path(canvas: ?*anyopaque, xy: [*]const f32, count: c_int, color: u32) void;
extern fn zant_canvas_stroke_path(canvas: ?*anyopaque, xy: [*]const f32, count: c_int, color: u32, width: f32, closed: c_int) void;
extern fn zant_canvas_fill_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, color: u32) void;
extern fn zant_canvas_fill_round_rect(canvas: ?*anyopaque, x: f32, y: f32, width: f32, height: f32, radius: f32, color: u32) void;
extern fn zant_canvas_draw_line(canvas: ?*anyopaque, x0: f32, y0: f32, x1: f32, y1: f32, color: u32, stroke_width: f32) void;

pub fn measure(text: []const u8, size: f32, family: text_mod.Family, weight: u16, italic: bool) InkMetrics {
    if (text.len == 0) return std.mem.zeroes(InkMetrics);
    return zant_text_measure_ink(text.ptr, text.len, size, @backingInt(family), weight, @intFromBool(italic));
}

/// Paints `drawing` with its baseline origin at (x, baseline).
pub fn paint(canvas: ?*anyopaque, drawing: *const Drawing, x: f32, baseline: f32) void {
    for (drawing.commands) |cmd| {
        switch (cmd) {
            .text => |t| zant_canvas_draw_simple_text(canvas, t.text.ptr, t.text.len, x + t.x, baseline + t.y, t.size, @backingInt(t.family), t.weight, @intFromBool(t.italic), t.color, t.scale_y),
            .rect => |r| {
                if (r.radius > 0) {
                    zant_canvas_fill_round_rect(canvas, x + r.x, baseline + r.y, r.width, r.height, r.radius, r.color);
                } else {
                    zant_canvas_fill_rect(canvas, x + r.x, baseline + r.y, r.width, r.height, r.color);
                }
            },
            .line => |l| zant_canvas_draw_line(canvas, x + l.x0, baseline + l.y0, x + l.x1, baseline + l.y1, l.color, l.width),
            .path => |p| {
                var buf: [256]f32 = undefined;
                const n = @min(p.points.len, buf.len / 2);
                for (p.points[0..n], 0..) |pt, i| {
                    buf[i * 2] = x + pt[0];
                    buf[i * 2 + 1] = baseline + pt[1];
                }
                if (p.stroke_width > 0) {
                    zant_canvas_stroke_path(canvas, &buf, @intCast(n), p.color, p.stroke_width, @intFromBool(p.closed));
                } else {
                    zant_canvas_fill_path(canvas, &buf, @intCast(n), p.color);
                }
            },
        }
    }
}

/// Builder that collects commands and tracks the drawing's extent.
pub const Builder = struct {
    allocator: std.mem.Allocator,
    commands: std.ArrayListUnmanaged(Command) = .empty,

    pub fn init(allocator: std.mem.Allocator) Builder {
        return .{ .allocator = allocator };
    }

    pub fn add(self: *Builder, cmd: Command) !void {
        try self.commands.append(self.allocator, cmd);
    }

    /// Appends `cmds` translated by (dx, dy).
    pub fn addTranslated(self: *Builder, cmds: []const Command, dx: f32, dy: f32) !void {
        for (cmds) |cmd| try self.add(try translate(self.allocator, cmd, dx, dy));
    }

    pub fn finish(self: *Builder, width: f32, ascent: f32, descent: f32) !Drawing {
        return .{ .width = width, .ascent = ascent, .descent = descent, .commands = try self.commands.toOwnedSlice(self.allocator) };
    }
};

pub fn translate(allocator: std.mem.Allocator, cmd: Command, dx: f32, dy: f32) !Command {
    return switch (cmd) {
        .text => |t| blk: {
            var c = t;
            c.x += dx;
            c.y += dy;
            break :blk .{ .text = c };
        },
        .rect => |r| blk: {
            var c = r;
            c.x += dx;
            c.y += dy;
            break :blk .{ .rect = c };
        },
        .line => |l| .{ .line = .{ .x0 = l.x0 + dx, .y0 = l.y0 + dy, .x1 = l.x1 + dx, .y1 = l.y1 + dy, .color = l.color, .width = l.width } },
        .path => |p| blk: {
            const pts = try allocator.alloc(Point, p.points.len);
            for (p.points, 0..) |pt, i| pts[i] = .{ pt[0] + dx, pt[1] + dy };
            var c = p;
            c.points = pts;
            break :blk .{ .path = c };
        },
    };
}
