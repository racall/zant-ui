//! TeX box layout, ported from KaTeX's HTML builder (`buildHTML.js`,
//! `functions/{supsub,genfrac,sqrt,op,delimsizing,accent,…}.js`,
//! `environments/array.js`) and its font metrics (`fontMetrics.js`).
//!
//! Every box is measured in pixels with its origin on the baseline at the
//! left edge; y grows downwards. Glyphs are drawn with STIX Two Math.
const std = @import("std");
const vector = @import("../core/vector.zig");
const text_mod = @import("../renderer/text.zig");
const parser = @import("parser.zig");
const symbols = @import("symbols.zig");
const fonts = @import("fonts.zig");

const Node = parser.Node;
const Class = symbols.Class;
const Command = vector.Command;

// ── styles (KaTeX Style.js) ───────────────────────────────────────────────

pub const Size = enum(u2) { display, text, script, scriptscript };

pub const Style = struct {
    size: Size,
    cramped: bool = false,

    pub const display: Style = .{ .size = .display };
    pub const text: Style = .{ .size = .text };

    fn smaller(s: Size) Size {
        return switch (s) {
            .display, .text => .script,
            .script, .scriptscript => .scriptscript,
        };
    }

    pub fn sup(self: Style) Style {
        return .{ .size = smaller(self.size), .cramped = self.cramped };
    }
    pub fn sub(self: Style) Style {
        return .{ .size = smaller(self.size), .cramped = true };
    }
    pub fn fracNum(self: Style) Style {
        return .{ .size = switch (self.size) {
            .display => .text,
            .text => .script,
            else => .scriptscript,
        }, .cramped = self.cramped };
    }
    pub fn fracDen(self: Style) Style {
        var s = self.fracNum();
        s.cramped = true;
        return s;
    }
    pub fn cramp(self: Style) Style {
        return .{ .size = self.size, .cramped = true };
    }
    fn metricIndex(self: Style) usize {
        return switch (self.size) {
            .display, .text => 0,
            .script => 1,
            .scriptscript => 2,
        };
    }
    fn multiplier(self: Style) f32 {
        return switch (self.size) {
            .display, .text => 1.0,
            .script => 0.7,
            .scriptscript => 0.5,
        };
    }
    fn tight(self: Style) bool {
        return self.size == .script or self.size == .scriptscript;
    }
};

// ── font metrics (KaTeX sigmasAndXis, per text/script/scriptscript) ───────

const Metric = enum {
    x_height,
    quad,
    num1,
    num2,
    num3,
    denom1,
    denom2,
    sup1,
    sup2,
    sup3,
    sub1,
    sub2,
    sup_drop,
    sub_drop,
    delim1,
    delim2,
    axis_height,
    default_rule_thickness,
    big_op_spacing1,
    big_op_spacing2,
    big_op_spacing3,
    big_op_spacing4,
    big_op_spacing5,
};

const sigmas = std.enums.EnumArray(Metric, [3]f32).init(.{
    .x_height = .{ 0.431, 0.431, 0.431 },
    .quad = .{ 1.000, 1.171, 1.472 },
    .num1 = .{ 0.677, 0.732, 0.925 },
    .num2 = .{ 0.394, 0.384, 0.387 },
    .num3 = .{ 0.444, 0.471, 0.504 },
    .denom1 = .{ 0.686, 0.752, 1.025 },
    .denom2 = .{ 0.345, 0.344, 0.532 },
    .sup1 = .{ 0.413, 0.503, 0.504 },
    .sup2 = .{ 0.363, 0.431, 0.404 },
    .sup3 = .{ 0.289, 0.286, 0.294 },
    .sub1 = .{ 0.150, 0.143, 0.200 },
    .sub2 = .{ 0.247, 0.286, 0.400 },
    .sup_drop = .{ 0.386, 0.353, 0.494 },
    .sub_drop = .{ 0.050, 0.071, 0.100 },
    .delim1 = .{ 2.390, 1.700, 1.980 },
    .delim2 = .{ 1.010, 1.157, 1.420 },
    .axis_height = .{ 0.250, 0.250, 0.250 },
    .default_rule_thickness = .{ 0.04, 0.049, 0.049 },
    .big_op_spacing1 = .{ 0.111, 0.111, 0.111 },
    .big_op_spacing2 = .{ 0.166, 0.166, 0.166 },
    .big_op_spacing3 = .{ 0.2, 0.2, 0.2 },
    .big_op_spacing4 = .{ 0.6, 0.611, 0.611 },
    .big_op_spacing5 = .{ 0.1, 0.143, 0.143 },
});

// ── spacing (KaTeX spacingData.js, in mu) ─────────────────────────────────

fn spacingMu(left: Class, right: Class, tight: bool) f32 {
    if (tight) {
        return switch (left) {
            .ord, .close, .inner => if (right == .op) 3 else 0,
            .op => if (right == .ord or right == .op) 3 else 0,
            else => 0,
        };
    }
    const thin: f32 = 3;
    const med: f32 = 4;
    const thick: f32 = 5;
    return switch (left) {
        .ord => switch (right) {
            .op, .inner => thin,
            .bin => med,
            .rel => thick,
            else => 0,
        },
        .op => switch (right) {
            .ord, .op, .inner => thin,
            .rel => thick,
            else => 0,
        },
        .bin => switch (right) {
            .ord, .op, .open, .inner => med,
            else => 0,
        },
        .rel => switch (right) {
            .ord, .op, .open, .inner => thick,
            else => 0,
        },
        .open => 0,
        .close => switch (right) {
            .op, .inner => thin,
            .bin => med,
            .rel => thick,
            else => 0,
        },
        .punct => switch (right) {
            .rel => thick,
            .bin => 0,
            else => thin,
        },
        .inner => switch (right) {
            .ord, .op, .open, .punct, .inner => thin,
            .bin => med,
            .rel => thick,
            else => 0,
        },
    };
}

// ── boxes ─────────────────────────────────────────────────────────────────

pub const Box = struct {
    width: f32 = 0,
    height: f32 = 0,
    depth: f32 = 0,
    cmds: []const Command = &.{},
    class: Class = .ord,
    /// Single glyph (TeX "character box"): scripts attach without drops.
    is_char: bool = false,
    /// Italic correction (scripts after slanted glyphs move right).
    italic: f32 = 0,
    /// Spaces do not take part in inter-atom spacing.
    is_space: bool = false,
    /// `\tag` content, carried to the top level.
    is_tag: bool = false,
};

pub const Options = struct {
    /// Pixel size of 1em in text style.
    size: f32,
    color: u32,
    error_color: u32,
};

pub const Engine = struct {
    arena: std.mem.Allocator,
    options: Options,
    color: u32,
    tag: ?Box = null,
    /// Delimiter height for `\middle` inside the current `\left…\right`.
    middle_total: ?f32 = null,

    pub fn init(arena: std.mem.Allocator, options: Options) Engine {
        return .{ .arena = arena, .options = options, .color = options.color };
    }

    fn em(self: *const Engine, style: Style) f32 {
        return self.options.size * style.multiplier();
    }

    fn metric(self: *const Engine, style: Style, m: Metric) f32 {
        return sigmas.get(m)[style.metricIndex()] * self.em(style);
    }

    fn rule(self: *const Engine, style: Style) f32 {
        return @max(0.9, self.metric(style, .default_rule_thickness));
    }

    // ── primitives ──────────────────────────────────────────────────────

    fn glyph(self: *Engine, text: []const u8, size: f32, class: Class, italic: bool) !Box {
        const m = vector.measure(text, size, .math, 400, false);
        const cmds = try self.arena.alloc(Command, 1);
        cmds[0] = .{ .text = .{ .x = 0, .y = 0, .text = text, .size = size, .family = .math, .color = self.color } };
        return .{
            .width = m.advance,
            .height = m.ascent,
            .depth = m.descent,
            .cmds = cmds,
            .class = class,
            .is_char = true,
            .italic = if (italic) 0.04 * size else 0,
        };
    }

    fn textBox(self: *Engine, text: []const u8, size: f32, color: u32) !Box {
        const m = vector.measure(text, size, .math, 400, false);
        const cmds = try self.arena.alloc(Command, 1);
        cmds[0] = .{ .text = .{ .x = 0, .y = 0, .text = text, .size = size, .family = .math, .color = color } };
        // text keeps the font's line extent so mixed rows align
        return .{ .width = m.advance, .height = @max(m.ascent, 0.7 * size), .depth = @max(m.descent, 0.2 * size), .cmds = cmds };
    }

    fn kern(width: f32) Box {
        return .{ .width = width, .is_space = true };
    }

    fn translated(self: *Engine, box: Box, dx: f32, dy: f32, out: *std.ArrayListUnmanaged(Command)) !void {
        for (box.cmds) |cmd| try out.append(self.arena, try vector.translate(self.arena, cmd, dx, dy));
    }

    fn rect(self: *Engine, x: f32, y: f32, w: f32, h: f32, out: *std.ArrayListUnmanaged(Command)) !void {
        try out.append(self.arena, .{ .rect = .{ .x = x, .y = y, .width = w, .height = h, .color = self.color } });
    }

    fn stroke(self: *Engine, points: []const vector.Point, width: f32, out: *std.ArrayListUnmanaged(Command)) !void {
        try out.append(self.arena, .{ .path = .{ .points = try self.arena.dupe(vector.Point, points), .color = self.color, .stroke_width = width } });
    }

    fn fill(self: *Engine, points: []const vector.Point, out: *std.ArrayListUnmanaged(Command)) !void {
        try out.append(self.arena, .{ .path = .{ .points = try self.arena.dupe(vector.Point, points), .color = self.color } });
    }

    // ── math lists ──────────────────────────────────────────────────────

    pub fn list(self: *Engine, nodes: []const Node, style: Style) anyerror!Box {
        var boxes: std.ArrayListUnmanaged(Box) = .empty;
        var current = style;
        for (nodes) |node| {
            if (node == .style) {
                // \displaystyle etc. switch the rest of the group
                const sw = node.style;
                current = switch (sw.style) {
                    .display => Style.display,
                    .text => Style.text,
                    .script => .{ .size = .script },
                    .scriptscript => .{ .size = .scriptscript },
                };
                try boxes.append(self.arena, try self.list(sw.body, current));
                continue;
            }
            if (node == .tag) {
                self.tag = try self.textBox(try std.fmt.allocPrint(self.arena, "({s})", .{node.tag}), self.em(Style.text), self.color);
                continue;
            }
            try boxes.append(self.arena, try self.build(node, current));
        }
        return self.hlist(boxes.items, style);
    }

    /// Joins boxes horizontally with TeX inter-atom spacing.
    fn hlist(self: *Engine, boxes: []Box, style: Style) !Box {
        // bin → ord conversions (KaTeX buildExpression)
        var prev_class: ?Class = null;
        for (boxes, 0..) |*box, i| {
            if (box.is_space) continue;
            if (box.class == .bin) {
                const next_class: ?Class = blk: {
                    var j = i + 1;
                    while (j < boxes.len) : (j += 1) if (!boxes[j].is_space) break :blk boxes[j].class;
                    break :blk null;
                };
                const left_cancel = prev_class == null or prev_class.? == .bin or prev_class.? == .op or prev_class.? == .rel or prev_class.? == .open or prev_class.? == .punct;
                const right_cancel = next_class != null and (next_class.? == .rel or next_class.? == .close or next_class.? == .punct);
                if (left_cancel or right_cancel) box.class = .ord;
            }
            prev_class = box.class;
        }
        var out: std.ArrayListUnmanaged(Command) = .empty;
        var x: f32 = 0;
        var height: f32 = 0;
        var depth: f32 = 0;
        var last: ?Class = null;
        const mu = self.em(style) / 18.0;
        for (boxes) |box| {
            if (!box.is_space) {
                if (last) |l| x += spacingMu(l, box.class, style.tight()) * mu;
                last = box.class;
            }
            try self.translated(box, x, 0, &out);
            x += box.width;
            height = @max(height, box.height);
            depth = @max(depth, box.depth);
        }
        const single = boxes.len == 1;
        return .{
            .width = x,
            .height = height,
            .depth = depth,
            .cmds = try out.toOwnedSlice(self.arena),
            .class = .ord,
            .is_char = single and boxes[0].is_char,
            .italic = if (single) boxes[0].italic else 0,
        };
    }

    fn build(self: *Engine, node: Node, style: Style) anyerror!Box {
        return switch (node) {
            .atom => |a| self.atom(a, style),
            .group => |g| blk: {
                var b = try self.list(g, style);
                b.class = .ord;
                break :blk b;
            },
            .supsub => |s| self.supsub(s, style),
            .frac => |f| self.frac(f, style),
            .sqrt => |s| self.sqrt(s, style),
            .left_right => |lr| self.leftRight(lr, style),
            .sized_delim => |d| self.sizedDelim(d, style),
            .accent => |a| self.accent(a, style),
            .over_under => |o| self.overUnder(o, style),
            .op => |o| self.op(o, style),
            .text => |t| self.textNode(t, style),
            .space => |s| kern(s * self.em(style)),
            .array => |a| self.array(a, style),
            .color => |c| blk: {
                const saved = self.color;
                self.color = c.color;
                defer self.color = saved;
                break :blk try self.list(c.body, style);
            },
            .style => |s| self.list(s.body, style),
            .phantom => |p| blk: {
                const inner = try self.list(p.body, style);
                break :blk .{ .width = if (p.horizontal) inner.width else 0, .height = if (p.vertical) inner.height else 0, .depth = if (p.vertical) inner.depth else 0 };
            },
            .boxed => |b| self.boxed(b, style),
            .cancel => |c| self.cancel(c.body, c.kind, style),
            .stack => |s| self.stack(s, style),
            .tag => kern(0),
            .xarrow => |x| self.xArrow(x, style),
            .sized => |sz| blk: {
                const saved = self.options.size;
                self.options.size = saved * sz.factor;
                defer self.options.size = saved;
                break :blk try self.list(sz.body, style);
            },
            .colorbox => |cb| self.colorBox(cb, style),
            .lap => |l| blk: {
                const inner = try self.list(l.body, style);
                var out: std.ArrayListUnmanaged(Command) = .empty;
                const dx: f32 = switch (l.side) {
                    .right => 0,
                    .left => -inner.width,
                    .center => -inner.width / 2,
                };
                try self.translated(inner, dx, 0, &out);
                break :blk Box{ .width = 0, .height = inner.height, .depth = inner.depth, .cmds = try out.toOwnedSlice(self.arena) };
            },
            .raise => |r| blk: {
                const inner = try self.list(r.body, style);
                const dy = r.amount * self.em(style);
                var out: std.ArrayListUnmanaged(Command) = .empty;
                try self.translated(inner, 0, -dy, &out);
                break :blk Box{ .width = inner.width, .height = @max(0, inner.height + dy), .depth = @max(0, inner.depth - dy), .cmds = try out.toOwnedSlice(self.arena) };
            },
            .choice => |c| self.list(c[
                switch (style.size) {
                    .display => @as(usize, 0),
                    .text => 1,
                    .script => 2,
                    .scriptscript => 3,
                }
            ], style),
            .smash => |body| blk: {
                var inner = try self.list(body, style);
                inner.height = 0;
                inner.depth = 0;
                break :blk inner;
            },
            .rule_box => |r| blk: {
                var out: std.ArrayListUnmanaged(Command) = .empty;
                const w = r.width * self.em(style);
                const h = r.height * self.em(style);
                try self.rect(0, -h, w, h, &out);
                break :blk Box{ .width = w, .height = h, .depth = 0, .cmds = try out.toOwnedSlice(self.arena) };
            },
            .unknown => |name| blk: {
                const saved = self.color;
                self.color = self.options.error_color;
                defer self.color = saved;
                break :blk try self.textBox(try std.fmt.allocPrint(self.arena, "\\{s}", .{name}), self.em(style), self.options.error_color);
            },
        };
    }

    fn atom(self: *Engine, a: parser.Atom, style: Style) !Box {
        const mapped = try fonts.mapText(self.arena, a.variant, a.text);
        const italic = a.variant == .normal and mapped.ptr != a.text.ptr and !std.mem.eql(u8, mapped, a.text);
        return self.glyph(mapped, self.em(style), a.class, italic);
    }

    fn textNode(self: *Engine, t: parser.Text, style: Style) !Box {
        const variant: fonts.Variant = if (t.bold) .bold else if (t.variant == .roman) .roman else t.variant;
        const mapped = try fonts.mapText(self.arena, variant, t.text);
        var b = try self.textBox(mapped, self.em(style), self.color);
        b.class = .ord;
        return b;
    }

    // ── supsub (KaTeX functions/supsub.js) ──────────────────────────────

    fn supsub(self: *Engine, s: parser.SupSub, style: Style) !Box {
        // operators with limits and braces with labels stack their scripts
        if (s.base) |base_node| {
            switch (base_node.*) {
                .op => |o| {
                    const limits = o.forced orelse (o.limits and style.size == .display);
                    if (limits) return self.opLimits(o, s, style);
                },
                .over_under => |ou| if (ou.kind == .overbrace or ou.kind == .underbrace) return self.braceLabel(ou, s, style),
                else => {},
            }
        }
        const base: Box = if (s.base) |b| try self.build(b.*, style) else .{};
        const sup_style = style.sup();
        const sub_style = style.sub();
        var sup_box: ?Box = if (s.sup) |n| try self.build(n.*, sup_style) else null;
        var sub_box: ?Box = if (s.sub) |n| try self.build(n.*, sub_style) else null;

        var sup_shift: f32 = 0;
        var sub_shift: f32 = 0;
        if (!base.is_char) {
            sup_shift = base.height - self.metric(sup_style, .sup_drop);
            sub_shift = base.depth + self.metric(sub_style, .sub_drop);
        }
        const min_sup_shift = if (style.size == .display) self.metric(style, .sup1) else if (style.cramped) self.metric(style, .sup3) else self.metric(style, .sup2);
        const x_height = self.metric(style, .x_height);
        if (sup_box != null and sub_box != null) {
            const sp = sup_box.?;
            const sb = sub_box.?;
            sup_shift = @max(sup_shift, min_sup_shift, sp.depth + 0.25 * x_height);
            sub_shift = @max(sub_shift, self.metric(style, .sub2));
            const rule_width = self.metric(style, .default_rule_thickness);
            const max_width = 4 * rule_width;
            if ((sup_shift - sp.depth) - (sb.height - sub_shift) < max_width) {
                sub_shift = max_width - (sup_shift - sp.depth) + sb.height;
                const psi = 0.8 * x_height - (sup_shift - sp.depth);
                if (psi > 0) {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
        } else if (sub_box) |sb| {
            sub_shift = @max(sub_shift, self.metric(style, .sub1), sb.height - 0.8 * x_height);
        } else if (sup_box) |sp| {
            sup_shift = @max(sup_shift, min_sup_shift, sp.depth + 0.25 * x_height);
        }

        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(base, 0, 0, &out);
        const script_space = 0.05 * self.options.size;
        var width = base.width;
        var height = base.height;
        var depth = base.depth;
        var scripts_width: f32 = 0;
        if (sup_box) |*sp| {
            try self.translated(sp.*, base.width + base.italic, -sup_shift, &out);
            scripts_width = @max(scripts_width, sp.width + base.italic);
            height = @max(height, sup_shift + sp.height);
            depth = @max(depth, sp.depth - sup_shift);
        }
        if (sub_box) |*sb| {
            try self.translated(sb.*, base.width, sub_shift, &out);
            scripts_width = @max(scripts_width, sb.width);
            depth = @max(depth, sub_shift + sb.depth);
            height = @max(height, sb.height - sub_shift);
        }
        width += scripts_width + script_space;
        sup_box = null;
        sub_box = null;
        return .{ .width = width, .height = height, .depth = depth, .cmds = try out.toOwnedSlice(self.arena), .class = base.class };
    }

    // ── genfrac (KaTeX functions/genfrac.js) ────────────────────────────

    fn frac(self: *Engine, f: parser.Frac, outer: Style) !Box {
        const style: Style = if (f.style) |s| switch (s) {
            .display => Style.display,
            .text => Style.text,
            .script => .{ .size = .script },
            .scriptscript => .{ .size = .scriptscript },
        } else outer;
        const num_style = if (f.continued) Style.display else style.fracNum();
        const num = try self.list(f.num, num_style);
        const den = try self.list(f.den, style.fracDen());
        const rule_width: f32 = if (f.bar) self.rule(style) else 0;
        const rule_spacing = self.metric(style, .default_rule_thickness);
        var num_shift: f32 = undefined;
        var clearance: f32 = undefined;
        var den_shift: f32 = undefined;
        if (style.size == .display) {
            num_shift = self.metric(style, .num1);
            clearance = if (rule_width > 0) 3 * rule_spacing else 7 * rule_spacing;
            den_shift = self.metric(style, .denom1);
        } else {
            if (rule_width > 0) {
                num_shift = self.metric(style, .num2);
                clearance = rule_spacing;
            } else {
                num_shift = self.metric(style, .num3);
                clearance = 3 * rule_spacing;
            }
            den_shift = self.metric(style, .denom2);
        }
        const axis = self.metric(style, .axis_height);
        if (rule_width == 0) {
            const candidate = (num_shift - num.depth) - (den.height - den_shift);
            if (candidate < clearance) {
                num_shift += 0.5 * (clearance - candidate);
                den_shift += 0.5 * (clearance - candidate);
            }
        } else {
            const above = (num_shift - num.depth) - (axis + 0.5 * rule_width);
            if (above < clearance) num_shift += clearance - above;
            const below = (axis - 0.5 * rule_width) - (den.height - den_shift);
            if (below < clearance) den_shift += clearance - below;
        }
        const null_delim = 0.12 * self.em(style);
        const inner_w = @max(num.width, den.width);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(num, null_delim + (inner_w - num.width) / 2, -num_shift, &out);
        try self.translated(den, null_delim + (inner_w - den.width) / 2, den_shift, &out);
        if (rule_width > 0) try self.rect(null_delim, -axis - rule_width / 2, inner_w, rule_width, &out);
        var box: Box = .{
            .width = inner_w + 2 * null_delim,
            .height = num_shift + num.height,
            .depth = den_shift + den.depth,
            .cmds = try out.toOwnedSlice(self.arena),
            .class = .ord,
        };
        if (f.left.len > 0 or f.right.len > 0) {
            const delim_size = if (style.size == .display) self.metric(style, .delim1) else self.metric(style, .delim2);
            box = try self.wrapDelims(box, f.left, f.right, style, delim_size);
            box.class = .ord;
        }
        return box;
    }

    // ── delimiters (KaTeX delimiter.js) ─────────────────────────────────

    /// A delimiter glyph with total height `total`, centred on the axis.
    fn delimiter(self: *Engine, delim: []const u8, total: f32, style: Style, class: Class) !Box {
        const size = self.em(style);
        if (delim.len == 0) return .{ .width = 0.12 * size, .class = class, .is_space = false };
        const base = vector.measure(delim, size, .math, 400, false);
        const ink = @max(base.ascent + base.descent, 0.01);
        var font_size = size;
        var scale_y: f32 = 1;
        if (total > ink) {
            const ratio = total / ink;
            const grow = @min(ratio, 1.35);
            font_size = size * grow;
            scale_y = ratio / grow;
        }
        const m = vector.measure(delim, font_size, .math, 400, false);
        const asc = m.ascent * scale_y;
        const desc = m.descent * scale_y;
        const axis = self.metric(style, .axis_height);
        // ink centre on the math axis
        const baseline = -axis + (asc - desc) / 2;
        const cmds = try self.arena.alloc(Command, 1);
        cmds[0] = .{ .text = .{ .x = 0, .y = baseline, .text = delim, .size = font_size, .family = .math, .color = self.color, .scale_y = scale_y } };
        return .{ .width = m.advance, .height = asc - baseline, .depth = desc + baseline, .cmds = cmds, .class = class };
    }

    /// Size of `\left`/`\right` around content (KaTeX makeLeftRightDelim).
    fn leftRightTotal(self: *Engine, height: f32, depth: f32, style: Style) f32 {
        const axis = self.metric(style, .axis_height);
        const delimiter_factor: f32 = 901;
        const delimiter_extend = 0.5 * self.em(style);
        const max_dist = @max(height - axis, depth + axis);
        return @max(max_dist / 500.0 * delimiter_factor, 2 * max_dist - delimiter_extend);
    }

    fn wrapDelims(self: *Engine, inner: Box, left: []const u8, right: []const u8, style: Style, total: f32) !Box {
        var parts: [3]Box = undefined;
        parts[0] = try self.delimiter(left, total, style, .open);
        parts[1] = inner;
        parts[1].class = .inner;
        parts[2] = try self.delimiter(right, total, style, .close);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        var x: f32 = 0;
        var h: f32 = 0;
        var d: f32 = 0;
        for (parts) |p| {
            try self.translated(p, x, 0, &out);
            x += p.width;
            h = @max(h, p.height);
            d = @max(d, p.depth);
        }
        return .{ .width = x, .height = h, .depth = d, .cmds = try out.toOwnedSlice(self.arena), .class = .inner };
    }

    fn leftRight(self: *Engine, lr: parser.LeftRight, style: Style) !Box {
        // first pass: content size decides delimiter size (also for \middle)
        var inner = try self.list(lr.body, style);
        const total = self.leftRightTotal(inner.height, inner.depth, style);
        var has_middle = false;
        for (lr.body) |n| if (n == .sized_delim and n.sized_delim.size == 0) {
            has_middle = true;
        };
        if (has_middle) {
            self.middle_total = total;
            defer self.middle_total = null;
            inner = try self.list(lr.body, style);
        }
        return self.wrapDelims(inner, lr.left, lr.right, style, total);
    }

    fn sizedDelim(self: *Engine, d: parser.SizedDelim, style: Style) !Box {
        const heights = [_]f32{ 0, 1.2, 1.8, 2.4, 3.0 };
        const total = if (d.size == 0) (self.middle_total orelse self.em(style)) else heights[d.size] * self.options.size;
        return self.delimiter(d.delim, total, style, d.class);
    }

    // ── sqrt (KaTeX functions/sqrt.js + delimiter.makeSqrtImage) ────────

    fn sqrt(self: *Engine, s: parser.Sqrt, style: Style) !Box {
        var inner = try self.list(s.body, style.cramp());
        const em_size = self.em(style);
        if (inner.height == 0) inner.height = self.metric(style, .x_height);
        const theta = self.rule(style);
        const phi = if (style.size == .display) self.metric(style, .x_height) else theta;
        var line_clearance = theta + phi / 4;
        const min_height = inner.height + inner.depth + line_clearance + theta;
        const total = @max(min_height, 1.0 * em_size);
        const delim_depth = total - theta;
        if (delim_depth > inner.height + inner.depth + line_clearance) {
            line_clearance = (line_clearance + delim_depth - inner.height - inner.depth) / 2;
        }
        const top = -(inner.height + line_clearance + theta);
        const bottom = top + total;
        const surd_w = if (total > 1.8 * em_size) 1.0 * em_size else 0.833 * em_size;
        const small = @min(total, 1.0 * em_size);

        var out: std.ArrayListUnmanaged(Command) = .empty;
        var x0: f32 = 0;
        var index_box: ?Box = null;
        if (s.index) |idx| {
            const ib = try self.list(idx, .{ .size = .scriptscript });
            const anchor = 0.55 * surd_w;
            if (ib.width > anchor) x0 = ib.width - anchor;
            index_box = ib;
        }
        // surd: left tick, heavy down-stroke, thin up-stroke to the vinculum
        const ax = x0 + 0.06 * surd_w;
        const ay = bottom - 0.42 * small;
        const bx = x0 + 0.22 * surd_w;
        const by = bottom - 0.52 * small;
        const cx = x0 + 0.50 * surd_w;
        const cy = bottom;
        const dx = x0 + surd_w;
        const dy = top + theta / 2;
        try self.stroke(&.{ .{ ax, ay }, .{ bx, by } }, theta * 1.1, &out);
        try self.stroke(&.{ .{ bx, by }, .{ cx, cy } }, theta * 2.4, &out);
        try self.stroke(&.{ .{ cx, cy }, .{ dx, dy } }, theta * 1.1, &out);
        const body_x = dx + 0.06 * em_size;
        try self.rect(dx - theta * 0.3, top, body_x - dx + inner.width + 0.06 * em_size + theta * 0.3, theta, &out);
        try self.translated(inner, body_x, 0, &out);
        var height = -top;
        if (index_box) |ib| {
            const raise = bottom - 0.6 * total;
            try self.translated(ib, x0 + 0.55 * surd_w - ib.width, raise - ib.depth, &out);
            height = @max(height, -(raise - ib.depth) + ib.height);
        }
        return .{
            .width = body_x + inner.width + 0.06 * em_size,
            .height = height,
            .depth = @max(bottom, inner.depth),
            .cmds = try out.toOwnedSlice(self.arena),
        };
    }

    // ── big operators (KaTeX functions/op.js, assembleSupSub) ───────────

    fn opBox(self: *Engine, o: parser.Op, style: Style) !Box {
        const size = self.em(style);
        if (!o.symbol) {
            var b = try self.textBox(o.text, size, self.color);
            b.class = .op;
            return b;
        }
        // KaTeX draws display operators from the Size2 font (∑ ≈ 1.45×, ∫ ≈ 2×)
        const factor: f32 = if (style.size == .display) (if (o.integral) 2.0 else 1.45) else (if (o.integral) 1.15 else 1.0);
        const font_size = size * factor;
        const m = vector.measure(o.text, font_size, .math, 400, false);
        const axis = self.metric(style, .axis_height);
        const baseline = -axis + (m.ascent - m.descent) / 2;
        const cmds = try self.arena.alloc(Command, 1);
        cmds[0] = .{ .text = .{ .x = 0, .y = baseline, .text = o.text, .size = font_size, .family = .math, .color = self.color } };
        return .{
            .width = m.advance,
            .height = m.ascent - baseline,
            .depth = m.descent + baseline,
            .cmds = cmds,
            .class = .op,
            .italic = if (o.integral) 0.3 * font_size else 0,
        };
    }

    fn op(self: *Engine, o: parser.Op, style: Style) !Box {
        return self.opBox(o, style);
    }

    fn opLimits(self: *Engine, o: parser.Op, s: parser.SupSub, style: Style) !Box {
        const base = try self.opBox(o, style);
        const sup: ?Box = if (s.sup) |n| try self.build(n.*, style.sup()) else null;
        const sub: ?Box = if (s.sub) |n| try self.build(n.*, style.sub()) else null;
        return self.assembleLimits(base, sup, sub, style, base.italic);
    }

    fn assembleLimits(self: *Engine, base: Box, sup: ?Box, sub: ?Box, style: Style, slant: f32) !Box {
        var width = base.width;
        if (sup) |b| width = @max(width, b.width);
        if (sub) |b| width = @max(width, b.width);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(base, (width - base.width) / 2, 0, &out);
        var height = base.height;
        var depth = base.depth;
        const spacing5 = self.metric(style, .big_op_spacing5);
        if (sup) |b| {
            const kern_sup = @max(self.metric(style, .big_op_spacing1), self.metric(style, .big_op_spacing3) - b.depth);
            const baseline = -(base.height + kern_sup + b.depth);
            try self.translated(b, (width - b.width) / 2 + slant / 2, baseline, &out);
            height = base.height + kern_sup + b.depth + b.height + spacing5;
        }
        if (sub) |b| {
            const kern_sub = @max(self.metric(style, .big_op_spacing2), self.metric(style, .big_op_spacing4) - b.height);
            const baseline = base.depth + kern_sub + b.height;
            try self.translated(b, (width - b.width) / 2 - slant / 2, baseline, &out);
            depth = base.depth + kern_sub + b.height + b.depth + spacing5;
        }
        return .{ .width = width, .height = height, .depth = depth, .cmds = try out.toOwnedSlice(self.arena), .class = .op };
    }

    fn stack(self: *Engine, s: anytype, style: Style) !Box {
        var base = try self.build(s.base.*, style);
        base.class = .rel;
        const over: ?Box = if (s.over) |n| try self.build(n.*, style.sup()) else null;
        const under: ?Box = if (s.under) |n| try self.build(n.*, style.sub()) else null;
        var b = try self.assembleLimits(base, over, under, style, 0);
        // \overset/\stackrel: tighter than operator limits, keep base class
        b.class = if (base.is_char) Class.rel else .ord;
        if (s.base.* == .group and s.base.group.len == 1 and s.base.group[0] == .atom) b.class = s.base.group[0].atom.class;
        return b;
    }

    // ── accents and over/under decorations ──────────────────────────────

    fn accent(self: *Engine, a: parser.AccentNode, style: Style) !Box {
        const base = try self.list(a.base, style.cramp());
        const size = self.em(style);
        const theta = self.rule(style);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(base, 0, 0, &out);
        const clearance = @max(base.height, self.metric(style, .x_height));
        const skew: f32 = if (base.is_char) base.italic + 0.03 * size else 0;
        var height = base.height;
        if (a.wide or base.width > 1.2 * size and !std.mem.eql(u8, a.name, "vec")) {
            // stretchy accents drawn as paths over the whole base
            const w = base.width;
            const y0 = -(clearance + 0.08 * size);
            const h = 0.2 * size;
            if (std.mem.eql(u8, a.name, "widehat") or std.mem.eql(u8, a.name, "hat")) {
                try self.stroke(&.{ .{ 0.02 * w, y0 }, .{ w / 2, y0 - h }, .{ 0.98 * w, y0 } }, theta * 1.2, &out);
            } else if (std.mem.eql(u8, a.name, "widecheck") or std.mem.eql(u8, a.name, "check")) {
                try self.stroke(&.{ .{ 0.02 * w, y0 - h }, .{ w / 2, y0 }, .{ 0.98 * w, y0 - h } }, theta * 1.2, &out);
            } else if (std.mem.eql(u8, a.name, "bar")) {
                try self.rect(0, y0 - theta, w, theta, &out);
            } else {
                // tilde wave
                var pts: [17]vector.Point = undefined;
                for (&pts, 0..) |*p, i| {
                    const t = @as(f32, @floatFromInt(i)) / 16.0;
                    p.* = .{ t * w, y0 - h / 2 - std.math.sin(t * std.math.tau) * h / 2.5 };
                }
                try self.stroke(&pts, theta * 1.2, &out);
            }
            height = clearance + 0.08 * size + h + theta;
        } else if (std.mem.eql(u8, a.name, "vec")) {
            const w = @max(base.width, 0.5 * size);
            const y = -(clearance + 0.16 * size);
            const head = 0.1 * size;
            const x0 = (base.width - w) / 2 + skew;
            try self.stroke(&.{ .{ x0, y }, .{ x0 + w, y } }, theta, &out);
            try self.stroke(&.{ .{ x0 + w - head, y - head * 0.7 }, .{ x0 + w, y }, .{ x0 + w - head, y + head * 0.7 } }, theta, &out);
            height = clearance + 0.16 * size + head;
        } else {
            const glyph_text = accentGlyph(a.name);
            const m = vector.measure(glyph_text, size, .math, 400, false);
            const gap = 0.06 * size;
            // the accent's ink bottom sits `gap` above the base
            const baseline = -(clearance + gap) - m.ink_bottom;
            const x = (base.width - m.advance) / 2 + skew;
            try out.append(self.arena, .{ .text = .{ .x = x, .y = baseline, .text = glyph_text, .size = size, .family = .math, .color = self.color } });
            height = -(baseline + m.ink_top);
        }
        return .{ .width = base.width, .height = @max(height, base.height), .depth = base.depth, .cmds = try out.toOwnedSlice(self.arena), .class = .ord, .is_char = base.is_char, .italic = base.italic };
    }

    fn accentGlyph(name: []const u8) []const u8 {
        const map = .{
            .{ "hat", "ˆ" },
            .{ "check", "ˇ" },
            .{ "tilde", "˜" },
            .{ "acute", "ˊ" },
            .{ "grave", "ˋ" },
            .{ "dot", "˙" },
            .{ "ddot", "¨" },
            .{ "breve", "˘" },
            .{ "bar", "ˉ" },
            .{ "mathring", "˚" },
        };
        inline for (map) |entry| if (std.mem.eql(u8, name, entry[0])) return entry[1];
        return "ˆ";
    }

    fn brace(self: *Engine, w: f32, y_edge: f32, depth_dir: f32, theta: f32, size: f32, out: *std.ArrayListUnmanaged(Command)) !void {
        // depth_dir = -1: opening downward (overbrace), +1: upward (underbrace)
        const h = 0.32 * size;
        const r = @min(0.22 * size, w / 4);
        const mid = w / 2;
        const ym = y_edge + depth_dir * h * 0.5;
        const tip = y_edge + depth_dir * h;
        try self.stroke(&.{
            .{ 0, y_edge },                                 .{ r * 0.35, ym - depth_dir * h * 0.12 },     .{ r, ym },                                     .{ mid - r, ym },
            .{ mid - r * 0.35, ym + depth_dir * h * 0.12 }, .{ mid, tip },                                .{ mid + r * 0.35, ym + depth_dir * h * 0.12 }, .{ mid + r, ym },
            .{ w - r, ym },                                 .{ w - r * 0.35, ym - depth_dir * h * 0.12 }, .{ w, y_edge },
        }, theta * 1.4, out);
    }

    fn overUnder(self: *Engine, o: parser.OverUnder, style: Style) !Box {
        const body_style = if (o.kind == .overline) style.cramp() else style;
        const base = try self.list(o.body, body_style);
        const theta = self.rule(style);
        const size = self.em(style);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(base, 0, 0, &out);
        var height = base.height;
        var depth = base.depth;
        const w = base.width;
        switch (o.kind) {
            .overline => {
                try self.rect(0, -(base.height + 3 * theta + theta), w, theta, &out);
                height = base.height + 5 * theta;
            },
            .underline => {
                try self.rect(0, base.depth + 3 * theta, w, theta, &out);
                depth = base.depth + 5 * theta;
            },
            .overbrace => {
                const y = -(base.height + 0.1 * size);
                try self.brace(w, y, -1, theta, size, &out);
                height = base.height + 0.1 * size + 0.32 * size + theta;
            },
            .underbrace => {
                const y = base.depth + 0.1 * size;
                try self.brace(w, y, 1, theta, size, &out);
                depth = base.depth + 0.1 * size + 0.32 * size + theta;
            },
            .overrightarrow, .overleftarrow, .overleftrightarrow, .underrightarrow, .underleftarrow => {
                const over = o.kind == .overrightarrow or o.kind == .overleftarrow or o.kind == .overleftrightarrow;
                const y = if (over) -(base.height + 0.18 * size) else base.depth + 0.18 * size;
                const head = 0.12 * size;
                try self.stroke(&.{ .{ 0, y }, .{ w, y } }, theta, &out);
                if (o.kind != .overleftarrow and o.kind != .underleftarrow)
                    try self.stroke(&.{ .{ w - head, y - head * 0.7 }, .{ w, y }, .{ w - head, y + head * 0.7 } }, theta, &out);
                if (o.kind == .overleftarrow or o.kind == .underleftarrow or o.kind == .overleftrightarrow)
                    try self.stroke(&.{ .{ head, y - head * 0.7 }, .{ 0, y }, .{ head, y + head * 0.7 } }, theta, &out);
                if (over) height = base.height + 0.18 * size + head else depth = base.depth + 0.18 * size + head;
            },
        }
        return .{ .width = w, .height = height, .depth = depth, .cmds = try out.toOwnedSlice(self.arena), .class = .ord };
    }

    /// `\xrightarrow[below]{above}` (KaTeX functions/arrow.js + stretchy.js):
    /// labels in script style padded 0.5em each side, the arrow at least
    /// its minimum width and centred on the axis, labels 0.111em from it.
    fn xArrow(self: *Engine, x: parser.XArrow, style: Style) !Box {
        const size = self.em(style);
        const theta = self.rule(style);
        const above = try self.list(x.above, style.sup());
        const below: ?Box = if (x.below) |b| try self.list(b, style.sub()) else null;
        const pad_above = 0.5 * self.em(style.sup());
        const pad_below = 0.5 * self.em(style.sub());
        const min_w: f32 = switch (x.kind) {
            .right, .left => 1.469,
            .double_right, .double_left => 1.526,
            .left_right, .double_left_right, .harpoons_rl, .harpoons_lr, .arrows_rl => 1.75,
            .hook_right, .hook_left => 1.08,
            .maps_to => 1.5,
            .long_equal, .two_head_right, .two_head_left => 0.888,
        };
        var w = @max(min_w * size, above.width + 2 * pad_above);
        if (below) |b| w = @max(w, b.width + 2 * pad_below);
        // arrow body heights in em (stretchy.js): single 0.522, double 0.56,
        // long equal 0.334, harpoon pairs 0.716
        const body_h: f32 = size * switch (x.kind) {
            .double_right, .double_left, .double_left_right => @as(f32, 0.56),
            .long_equal, .two_head_right, .two_head_left => 0.334,
            .harpoons_rl, .harpoons_lr, .arrows_rl => 0.716,
            else => 0.522,
        };
        const axis = self.metric(style, .axis_height);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.drawXArrow(x.kind, w, -axis, theta, size, &out);
        // labels: baseline shifts as in KaTeX's vlist (negative = up)
        var upper_shift = -axis - 0.5 * body_h - 0.111 * size;
        if (above.depth > 0.25 * size) upper_shift -= above.depth;
        try self.translated(above, (w - above.width) / 2, upper_shift, &out);
        var height = @max(-upper_shift + above.height, axis + body_h / 2);
        var depth = @max(0, -axis + body_h / 2);
        if (below) |b| {
            const lower_shift = -axis + b.height + 0.5 * body_h + 0.111 * size;
            try self.translated(b, (w - b.width) / 2, lower_shift, &out);
            depth = @max(depth, lower_shift + b.depth);
        }
        height = @max(height, 0);
        return .{ .width = w, .height = height, .depth = depth, .cmds = try out.toOwnedSlice(self.arena), .class = .rel };
    }

    fn drawXArrow(self: *Engine, kind: parser.XArrowKind, w: f32, y: f32, theta: f32, size: f32, out: *std.ArrayListUnmanaged(Command)) !void {
        const head = 0.14 * size;
        const hy = head * 0.65;
        const gap = 0.09 * size; // between the lines of a double arrow
        switch (kind) {
            .right, .left, .left_right, .maps_to, .hook_right, .hook_left, .two_head_right, .two_head_left => {
                try self.stroke(&.{ .{ 0, y }, .{ w, y } }, theta, out);
                const right_head = kind == .right or kind == .left_right or kind == .maps_to or kind == .hook_right or kind == .two_head_right;
                const left_head = kind == .left or kind == .left_right or kind == .hook_left or kind == .two_head_left;
                if (right_head) try self.stroke(&.{ .{ w - head, y - hy }, .{ w, y }, .{ w - head, y + hy } }, theta, out);
                if (left_head) try self.stroke(&.{ .{ head, y - hy }, .{ 0, y }, .{ head, y + hy } }, theta, out);
                if (kind == .two_head_right) try self.stroke(&.{ .{ w - 2 * head, y - hy }, .{ w - head, y }, .{ w - 2 * head, y + hy } }, theta, out);
                if (kind == .two_head_left) try self.stroke(&.{ .{ 2 * head, y - hy }, .{ head, y }, .{ 2 * head, y + hy } }, theta, out);
                if (kind == .maps_to) try self.stroke(&.{ .{ 0, y - hy }, .{ 0, y + hy } }, theta, out);
                if (kind == .hook_right or kind == .hook_left) {
                    // ↪ / ↩: a half circle curling up from the tail
                    const r = hy;
                    var pts: [9]vector.Point = undefined;
                    for (&pts, 0..) |*p, i| {
                        const t = std.math.pi * @as(f32, @floatFromInt(i)) / 8.0;
                        p.* = if (kind == .hook_right)
                            .{ r - r * @sin(t), y - r + r * @cos(t) }
                        else
                            .{ w - r + r * @sin(t), y - r + r * @cos(t) };
                    }
                    try self.stroke(&pts, theta, out);
                }
            },
            .double_right, .double_left, .double_left_right => {
                const right = kind != .double_left;
                const left = kind != .double_right;
                const x0: f32 = if (left) head * 0.6 else 0;
                const x1: f32 = if (right) w - head * 0.6 else w;
                try self.stroke(&.{ .{ x0, y - gap / 2 }, .{ x1, y - gap / 2 } }, theta, out);
                try self.stroke(&.{ .{ x0, y + gap / 2 }, .{ x1, y + gap / 2 } }, theta, out);
                if (right) try self.stroke(&.{ .{ w - head * 1.1, y - hy * 1.4 }, .{ w, y }, .{ w - head * 1.1, y + hy * 1.4 } }, theta, out);
                if (left) try self.stroke(&.{ .{ head * 1.1, y - hy * 1.4 }, .{ 0, y }, .{ head * 1.1, y + hy * 1.4 } }, theta, out);
            },
            .long_equal => {
                try self.stroke(&.{ .{ 0, y - gap / 2 }, .{ w, y - gap / 2 } }, theta, out);
                try self.stroke(&.{ .{ 0, y + gap / 2 }, .{ w, y + gap / 2 } }, theta, out);
            },
            .harpoons_rl, .harpoons_lr, .arrows_rl => {
                // upper line points right, lower points left (⇌ / ⇄)
                const top = y - gap * 1.1;
                const bottom = y + gap * 1.1;
                const upper_right = kind != .harpoons_lr;
                try self.stroke(&.{ .{ 0, top }, .{ w, top } }, theta, out);
                try self.stroke(&.{ .{ 0, bottom }, .{ w, bottom } }, theta, out);
                if (kind == .arrows_rl) {
                    try self.stroke(&.{ .{ w - head, top - hy }, .{ w, top }, .{ w - head, top + hy } }, theta, out);
                    try self.stroke(&.{ .{ head, bottom - hy }, .{ 0, bottom }, .{ head, bottom + hy } }, theta, out);
                } else if (upper_right) {
                    try self.stroke(&.{ .{ w - head, top - hy * 1.2 }, .{ w, top } }, theta, out);
                    try self.stroke(&.{ .{ head, bottom + hy * 1.2 }, .{ 0, bottom } }, theta, out);
                } else {
                    try self.stroke(&.{ .{ head, top - hy * 1.2 }, .{ 0, top } }, theta, out);
                    try self.stroke(&.{ .{ w - head, bottom + hy * 1.2 }, .{ w, bottom } }, theta, out);
                }
            },
        }
    }

    fn colorBox(self: *Engine, cb: anytype, style: Style) !Box {
        const inner = try self.list(cb.body, style);
        const pad = 0.3 * self.em(style);
        const theta = self.rule(style);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        const w = inner.width + 2 * pad;
        const top = -(inner.height + pad);
        const bottom = inner.depth + pad;
        try out.append(self.arena, .{ .rect = .{ .x = 0, .y = top, .width = w, .height = bottom - top, .color = cb.background } });
        if (cb.frame) |frame| {
            const saved = self.color;
            self.color = frame;
            defer self.color = saved;
            try self.stroke(&.{ .{ 0, top }, .{ w, top }, .{ w, bottom }, .{ 0, bottom }, .{ 0, top } }, theta, &out);
        }
        try self.translated(inner, pad, 0, &out);
        return .{ .width = w, .height = -top, .depth = bottom, .cmds = try out.toOwnedSlice(self.arena) };
    }

    fn braceLabel(self: *Engine, ou: parser.OverUnder, s: parser.SupSub, style: Style) !Box {
        const base = try self.overUnder(ou, style);
        const label_node = if (ou.kind == .overbrace) s.sup orelse s.sub else s.sub orelse s.sup;
        const label = if (label_node) |n| try self.build(n.*, style.sup()) else null;
        if (ou.kind == .overbrace) return self.assembleLimits(base, label, null, style, 0);
        return self.assembleLimits(base, null, label, style, 0);
    }

    // ── boxes and cancels ───────────────────────────────────────────────

    fn boxed(self: *Engine, body: []const Node, style: Style) !Box {
        const inner = try self.list(body, style);
        const pad = 0.3 * self.em(style);
        const theta = self.rule(style);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(inner, pad, 0, &out);
        const w = inner.width + 2 * pad;
        const top = -(inner.height + pad);
        const bottom = inner.depth + pad;
        try self.stroke(&.{ .{ 0, top }, .{ w, top }, .{ w, bottom }, .{ 0, bottom } }, theta, &out);
        // close the rectangle
        try self.stroke(&.{ .{ 0, bottom }, .{ 0, top } }, theta, &out);
        return .{ .width = w, .height = -top + theta, .depth = bottom + theta, .cmds = try out.toOwnedSlice(self.arena) };
    }

    fn cancel(self: *Engine, body: []const Node, kind: parser.CancelKind, style: Style) !Box {
        const inner = try self.list(body, style);
        const theta = self.rule(style);
        var out: std.ArrayListUnmanaged(Command) = .empty;
        try self.translated(inner, 0, 0, &out);
        const w = inner.width;
        const top = -inner.height - 0.05 * self.em(style);
        const bottom = inner.depth + 0.05 * self.em(style);
        switch (kind) {
            .cancel => try self.stroke(&.{ .{ 0, bottom }, .{ w, top } }, theta, &out),
            .bcancel => try self.stroke(&.{ .{ 0, top }, .{ w, bottom } }, theta, &out),
            .xcancel => {
                try self.stroke(&.{ .{ 0, bottom }, .{ w, top } }, theta, &out);
                try self.stroke(&.{ .{ 0, top }, .{ w, bottom } }, theta, &out);
            },
            .sout => try self.rect(0, -self.metric(style, .axis_height) - theta / 2, w, theta, &out),
        }
        return .{ .width = w, .height = inner.height, .depth = inner.depth, .cmds = try out.toOwnedSlice(self.arena), .class = inner.class };
    }

    // ── arrays (KaTeX environments/array.js) ────────────────────────────

    fn array(self: *Engine, a: parser.Array, outer: Style) !Box {
        const cell_style: Style = if (a.small) .{ .size = .script } else if (a.display) Style.display else Style.text;
        const em_size = self.em(outer);
        const arraystretch: f32 = if (a.small) 0.5 else 1.0;
        const baselineskip = 1.2 * em_size;
        const arrayskip = arraystretch * baselineskip;
        const strut_h = 0.7 * arrayskip;
        const strut_d = 0.3 * arrayskip;
        const jot: f32 = if (a.aligned or a.display) 0.3 * em_size else 0;
        const ncols = a.cols.len;

        const cells = try self.arena.alloc([]Box, a.rows.len);
        const col_widths = try self.arena.alloc(f32, ncols);
        @memset(col_widths, 0);
        const row_h = try self.arena.alloc(f32, a.rows.len);
        const row_d = try self.arena.alloc(f32, a.rows.len);
        for (a.rows, 0..) |row, r| {
            cells[r] = try self.arena.alloc(Box, ncols);
            var h = strut_h;
            var d = strut_d;
            for (0..ncols) |c| {
                // aligned: KaTeX starts every right-hand cell with an empty
                // ord so a leading relation (`&=`) keeps its spacing
                const box = if (c >= row.len) Box{} else if (a.aligned and c % 2 == 1)
                    try self.list(try std.mem.concat(self.arena, Node, &.{ &.{Node{ .group = &.{} }}, row[c] }), cell_style)
                else
                    try self.list(row[c], cell_style);
                cells[r][c] = box;
                col_widths[c] = @max(col_widths[c], box.width);
                h = @max(h, box.height);
                d = @max(d, box.depth);
            }
            row_h[r] = h;
            row_d[r] = d + jot;
        }
        // vertical positions
        const row_y = try self.arena.alloc(f32, a.rows.len);
        var total: f32 = 0;
        for (a.rows, 0..) |_, r| {
            total += row_h[r];
            row_y[r] = total;
            total += row_d[r];
            if (r < a.row_gaps.len) total += a.row_gaps[r] * em_size;
        }
        const offset = total / 2 + self.metric(outer, .axis_height);

        // horizontal positions
        const col_x = try self.arena.alloc(f32, ncols);
        var x: f32 = 0;
        const sep = a.col_sep * em_size;
        for (0..ncols) |c| {
            if (c > 0) {
                if (a.aligned) {
                    x += if (c % 2 == 0) 1.0 * em_size else 0;
                } else x += 2 * sep;
            }
            col_x[c] = x;
            x += col_widths[c];
        }
        const width = x;
        var out: std.ArrayListUnmanaged(Command) = .empty;
        for (a.rows, 0..) |_, r| {
            const baseline = row_y[r] - offset;
            for (0..ncols) |c| {
                const box = cells[r][c];
                const slack = col_widths[c] - box.width;
                const dx: f32 = switch (a.cols[c]) {
                    'l' => 0,
                    'r' => slack,
                    else => slack / 2,
                };
                try self.translated(box, col_x[c] + dx, baseline, &out);
            }
        }
        const theta = self.metric(outer, .default_rule_thickness);
        for (a.col_rules) |before| {
            const rx = if (before == 0) -sep else if (before >= ncols) width + sep else col_x[before] - sep;
            try self.rect(rx - theta / 2, -offset, theta, total, &out);
        }
        for (a.row_rules) |before| {
            var ry: f32 = 0;
            for (0..@min(before, a.rows.len)) |r| ry += row_h[r] + row_d[r];
            try self.rect(0, ry - offset - theta / 2, width, theta, &out);
        }
        var box: Box = .{ .width = width, .height = offset, .depth = total - offset, .cmds = try out.toOwnedSlice(self.arena), .class = .ord };
        if (a.left.len > 0 or a.right.len > 0) {
            const total_h = self.leftRightTotal(box.height, box.depth, outer);
            box = try self.wrapDelimsGapped(box, a.left, a.right, outer, total_h, if (a.left.len > 0 and a.right.len == 0) 0.2 * em_size else 0);
        }
        return box;
    }

    fn wrapDelimsGapped(self: *Engine, inner: Box, left: []const u8, right: []const u8, style: Style, total: f32, gap: f32) !Box {
        var padded = inner;
        if (gap > 0) {
            var out: std.ArrayListUnmanaged(Command) = .empty;
            try self.translated(inner, gap, 0, &out);
            padded.cmds = try out.toOwnedSlice(self.arena);
            padded.width += gap;
        }
        return self.wrapDelims(padded, left, right, style, total);
    }
};
