const std = @import("std");

pub const Point = struct {
    x: f32 = 0,
    y: f32 = 0,
};

pub const Size = struct {
    width: f32 = 0,
    height: f32 = 0,

    pub fn zero() Size {
        return .{};
    }

    pub fn max(a: Size, b: Size) Size {
        return .{ .width = @max(a.width, b.width), .height = @max(a.height, b.height) };
    }
};

pub const Edges = struct {
    top: f32 = 0,
    right: f32 = 0,
    bottom: f32 = 0,
    left: f32 = 0,

    pub fn all(v: f32) Edges {
        return .{ .top = v, .right = v, .bottom = v, .left = v };
    }

    pub fn symmetric(h: f32, v: f32) Edges {
        return .{ .top = v, .right = h, .bottom = v, .left = h };
    }

    pub fn horizontal(self: Edges) f32 {
        return self.left + self.right;
    }

    pub fn vertical(self: Edges) f32 {
        return self.top + self.bottom;
    }
};

pub const Rect = struct {
    x: f32 = 0,
    y: f32 = 0,
    width: f32 = 0,
    height: f32 = 0,

    pub fn init(x: f32, y: f32, width: f32, height: f32) Rect {
        return .{ .x = x, .y = y, .width = width, .height = height };
    }

    pub fn right(self: Rect) f32 {
        return self.x + self.width;
    }

    pub fn bottom(self: Rect) f32 {
        return self.y + self.height;
    }

    pub fn size(self: Rect) Size {
        return .{ .width = self.width, .height = self.height };
    }

    pub fn contains(self: Rect, px: f32, py: f32) bool {
        return px >= self.x and py >= self.y and px < self.right() and py < self.bottom();
    }

    pub fn inset(self: Rect, edges: Edges) Rect {
        return .{
            .x = self.x + edges.left,
            .y = self.y + edges.top,
            .width = @max(0, self.width - edges.horizontal()),
            .height = @max(0, self.height - edges.vertical()),
        };
    }

    pub fn intersect(a: Rect, b: Rect) Rect {
        const x0 = @max(a.x, b.x);
        const y0 = @max(a.y, b.y);
        const x1 = @min(a.right(), b.right());
        const y1 = @min(a.bottom(), b.bottom());
        return .{
            .x = x0,
            .y = y0,
            .width = @max(0, x1 - x0),
            .height = @max(0, y1 - y0),
        };
    }
};

pub const Constraints = struct {
    min_width: f32 = 0,
    max_width: f32 = std.math.inf(f32),
    min_height: f32 = 0,
    max_height: f32 = std.math.inf(f32),

    pub fn tight(width: f32, height: f32) Constraints {
        return .{ .min_width = width, .max_width = width, .min_height = height, .max_height = height };
    }

    pub fn loose(width: f32, height: f32) Constraints {
        return .{ .max_width = width, .max_height = height };
    }
};
