//! Native Mermaid renderer for the diagram types AI answers and notes use
//! most: `flowchart`/`graph`, `sequenceDiagram` and `pie`. Produces a
//! `vector.Drawing`; unsupported diagrams return null and are shown as code.
const std = @import("std");
const vector = @import("../core/vector.zig");

pub const Colors = struct {
    text: u32,
    node_fill: u32,
    node_border: u32,
    edge: u32,
    label_bg: u32,
    muted: u32,
};

pub const Options = struct {
    font_size: f32 = 14,
    colors: Colors,
    max_width: f32 = 640,
};

const palette = [_]u32{ 0xff4e79a7, 0xfff28e2b, 0xffe15759, 0xff76b7b2, 0xff59a14f, 0xffedc948, 0xffb07aa1, 0xffff9da7, 0xff9c755f, 0xffbab0ac };

pub const Kind = enum { flowchart, sequence, pie };

pub fn detect(source: []const u8) ?Kind {
    var lines = std.mem.splitScalar(u8, source, '\n');
    var in_front_matter = false;
    while (lines.next()) |raw| {
        const line = std.mem.trim(u8, raw, " \t\r");
        if (line.len == 0 or std.mem.startsWith(u8, line, "%%")) continue;
        if (std.mem.eql(u8, line, "---")) {
            in_front_matter = !in_front_matter;
            continue;
        }
        if (in_front_matter) continue;
        const word_end = std.mem.indexOfAny(u8, line, " \t") orelse line.len;
        const word = line[0..word_end];
        if (std.mem.eql(u8, word, "graph") or std.mem.eql(u8, word, "flowchart")) return .flowchart;
        if (std.mem.eql(u8, word, "sequenceDiagram")) return .sequence;
        if (std.mem.eql(u8, word, "pie")) return .pie;
        return null;
    }
    return null;
}

pub fn render(arena: std.mem.Allocator, source: []const u8, options: Options) !?vector.Drawing {
    const kind = detect(source) orelse return null;
    var drawing = switch (kind) {
        .flowchart => try flowchart(arena, source, options),
        .sequence => try sequence(arena, source, options),
        .pie => try pie(arena, source, options),
    } orelse return null;
    // wide diagrams shrink to the column (like display math)
    if (options.max_width > 0 and drawing.width > options.max_width) {
        drawing = try drawing.scaled(arena, options.max_width / drawing.width);
    }
    var h = std.hash.Wyhash.init(0x6d6d64);
    h.update(source);
    h.update(std.mem.asBytes(&options));
    drawing.hash = h.final();
    return drawing;
}

// ── shared drawing helpers ────────────────────────────────────────────────

const Canvas = struct {
    arena: std.mem.Allocator,
    cmds: std.ArrayListUnmanaged(vector.Command) = .empty,
    o: Options,

    fn text(self: *Canvas, x: f32, baseline: f32, t: []const u8, size: f32, color: u32) !void {
        try self.cmds.append(self.arena, .{ .text = .{ .x = x, .y = baseline, .text = t, .size = size, .family = .sans, .color = color } });
    }

    fn centeredText(self: *Canvas, cx: f32, cy: f32, t: []const u8, size: f32, color: u32) !void {
        const m = vector.measure(t, size, .sans, 400, false);
        try self.text(cx - m.advance / 2, cy + (m.font_ascent - m.font_descent) / 2, t, size, color);
    }

    fn rect(self: *Canvas, x: f32, y: f32, w: f32, h: f32, color: u32, radius: f32) !void {
        try self.cmds.append(self.arena, .{ .rect = .{ .x = x, .y = y, .width = w, .height = h, .color = color, .radius = radius } });
    }

    fn poly(self: *Canvas, pts: []const vector.Point, color: u32, stroke_width: f32, closed: bool) !void {
        try self.cmds.append(self.arena, .{ .path = .{ .points = try self.arena.dupe(vector.Point, pts), .color = color, .stroke_width = stroke_width, .closed = closed } });
    }

    fn line(self: *Canvas, x0: f32, y0: f32, x1: f32, y1: f32, color: u32, width: f32, dashed: bool) !void {
        if (!dashed) return self.poly(&.{ .{ x0, y0 }, .{ x1, y1 } }, color, width, false);
        const len = @sqrt((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0));
        if (len < 0.1) return;
        const dash: f32 = 5;
        var t: f32 = 0;
        while (t < len) : (t += dash * 2) {
            const e = @min(t + dash, len);
            try self.poly(&.{ .{ x0 + (x1 - x0) * t / len, y0 + (y1 - y0) * t / len }, .{ x0 + (x1 - x0) * e / len, y0 + (y1 - y0) * e / len } }, color, width, false);
        }
    }

    fn arrowHead(self: *Canvas, x: f32, y: f32, dx: f32, dy: f32, color: u32) !void {
        const len = @max(0.001, @sqrt(dx * dx + dy * dy));
        const ux = dx / len;
        const uy = dy / len;
        const s: f32 = 8;
        try self.poly(&.{ .{ x, y }, .{ x - ux * s - uy * s * 0.5, y - uy * s + ux * s * 0.5 }, .{ x - ux * s + uy * s * 0.5, y - uy * s - ux * s * 0.5 } }, color, 0, true);
    }

    fn roundedBorder(self: *Canvas, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) !void {
        // approximate rounded rectangle outline
        var pts: std.ArrayListUnmanaged(vector.Point) = .empty;
        const corners = [_][3]f32{ .{ x + w - r, y + r, -std.math.pi / 2.0 }, .{ x + w - r, y + h - r, 0 }, .{ x + r, y + h - r, std.math.pi / 2.0 }, .{ x + r, y + r, std.math.pi } };
        for (corners) |c| {
            var k: usize = 0;
            while (k <= 6) : (k += 1) {
                const a = c[2] + @as(f32, @floatFromInt(k)) / 6.0 * std.math.pi / 2.0;
                try pts.append(self.arena, .{ c[0] + @cos(a) * r, c[1] + @sin(a) * r });
            }
        }
        try self.poly(pts.items, color, 1.2, true);
    }

    /// Finishes with the top-left at (0,0); `ascent` = full height.
    fn finish(self: *Canvas, width: f32, height: f32) vector.Drawing {
        return .{ .width = width, .ascent = height, .descent = 0, .commands = self.cmds.items };
    }
};

/// Shifts every command up by `height` so the drawing's top is y = -ascent
/// (drawings are baseline-anchored; diagrams sit entirely above it).
fn anchorTop(arena: std.mem.Allocator, d: vector.Drawing) !vector.Drawing {
    var out = d;
    const cmds = try arena.alloc(vector.Command, d.commands.len);
    for (d.commands, 0..) |cmd, i| cmds[i] = try vector.translate(arena, cmd, 0, -d.ascent);
    out.commands = cmds;
    return out;
}

fn stripQuotes(t: []const u8) []const u8 {
    const s = std.mem.trim(u8, t, " \t");
    if (s.len >= 2 and (s[0] == '"' or s[0] == '\'') and s[s.len - 1] == s[0]) return s[1 .. s.len - 1];
    return s;
}

// ── flowchart ─────────────────────────────────────────────────────────────

const Shape = enum { rect, round, stadium, circle, diamond, hexagon, subroutine, cylinder, asymmetric };

const FNode = struct {
    id: []const u8,
    label: []const u8,
    shape: Shape = .rect,
    layer: usize = 0,
    order: f32 = 0,
    w: f32 = 0,
    h: f32 = 0,
    x: f32 = 0,
    y: f32 = 0,
    defined: bool = false,
};

const EdgeStyle = enum { solid, dotted, thick };

const FEdge = struct {
    from: usize,
    to: usize,
    label: []const u8 = "",
    arrow: bool = true,
    style: EdgeStyle = .solid,
};

const Direction = enum { td, bt, lr, rl };

const Flow = struct {
    arena: std.mem.Allocator,
    nodes: std.ArrayListUnmanaged(FNode) = .empty,
    edges: std.ArrayListUnmanaged(FEdge) = .empty,

    fn nodeIndex(self: *Flow, id: []const u8) !usize {
        for (self.nodes.items, 0..) |n, i| if (std.mem.eql(u8, n.id, id)) return i;
        try self.nodes.append(self.arena, .{ .id = id, .label = id });
        return self.nodes.items.len - 1;
    }

    /// Parses `id`, `id[label]`, `id(label)`, ... Returns node index and rest.
    fn parseNode(self: *Flow, text: []const u8) !?struct { index: usize, rest: []const u8 } {
        const t = std.mem.trimStart(u8, text, " \t");
        var i: usize = 0;
        while (i < t.len and (std.ascii.isAlphanumeric(t[i]) or t[i] == '_' or t[i] >= 0x80 or (t[i] == '-' and i + 1 < t.len and std.ascii.isAlphanumeric(t[i + 1]) and !(i + 1 < t.len and t[i + 1] == '-')))) i += 1;
        if (i == 0) return null;
        const id = t[0..i];
        const index = try self.nodeIndex(id);
        var rest = t[i..];
        const opens = [_]struct { open: []const u8, close: []const u8, shape: Shape }{
            .{ .open = "(((", .close = ")))", .shape = .circle }, .{ .open = "((", .close = "))", .shape = .circle },
            .{ .open = "([", .close = "])", .shape = .stadium },  .{ .open = "[[", .close = "]]", .shape = .subroutine },
            .{ .open = "[(", .close = ")]", .shape = .cylinder }, .{ .open = "{{", .close = "}}", .shape = .hexagon },
            .{ .open = "[/", .close = "/]", .shape = .rect },     .{ .open = "[\\", .close = "\\]", .shape = .rect },
            .{ .open = "(", .close = ")", .shape = .round },      .{ .open = "[", .close = "]", .shape = .rect },
            .{ .open = "{", .close = "}", .shape = .diamond },    .{ .open = ">", .close = "]", .shape = .asymmetric },
        };
        for (opens) |o| {
            if (std.mem.startsWith(u8, rest, o.open)) {
                const close = std.mem.indexOfPos(u8, rest, o.open.len, o.close) orelse continue;
                const node = &self.nodes.items[index];
                node.label = try cleanLabel(self.arena, rest[o.open.len..close]);
                node.shape = o.shape;
                node.defined = true;
                rest = rest[close + o.close.len ..];
                break;
            }
        }
        // :::class suffix
        if (std.mem.startsWith(u8, rest, ":::")) {
            var j: usize = 3;
            while (j < rest.len and (std.ascii.isAlphanumeric(rest[j]) or rest[j] == '_' or rest[j] == '-')) j += 1;
            rest = rest[j..];
        }
        return .{ .index = index, .rest = rest };
    }

    const Link = struct { arrow: bool, style: EdgeStyle, label: []const u8, rest: []const u8 };

    fn parseLink(self: *Flow, text: []const u8) !?Link {
        const t = std.mem.trimStart(u8, text, " \t");
        if (t.len < 2) return null;
        var label: []const u8 = "";
        var i: usize = 0;
        // inline text form: -- text --> / == text ==> / -. text .->
        if (std.mem.startsWith(u8, t, "-- ") or std.mem.startsWith(u8, t, "== ") or std.mem.startsWith(u8, t, "-. ")) {
            const closers = [_][]const u8{ "-->", "---", "==>", "===", ".->", ".-" };
            var best: ?usize = null;
            var best_len: usize = 0;
            for (closers) |c| if (std.mem.indexOfPos(u8, t, 3, c)) |p| {
                if (best == null or p < best.?) {
                    best = p;
                    best_len = c.len;
                }
            };
            if (best) |p| {
                label = std.mem.trim(u8, t[3..p], " ");
                const style: EdgeStyle = if (t[0] == '=') .thick else if (t[1] == '.') .dotted else .solid;
                const arrow = t[p + best_len - 1] == '>';
                return .{ .arrow = arrow, .style = style, .label = try cleanLabel(self.arena, label), .rest = t[p + best_len ..] };
            }
        }
        while (i < t.len and std.mem.indexOfScalar(u8, "-=.<>ox", t[i]) != null) : (i += 1) {
            // stop "o"/"x" heads that start an identifier
            if ((t[i] == 'o' or t[i] == 'x') and i + 1 < t.len and std.ascii.isAlphanumeric(t[i + 1])) break;
        }
        const op = t[0..i];
        if (op.len < 2 or (std.mem.indexOfScalar(u8, op, '-') == null and std.mem.indexOfScalar(u8, op, '=') == null)) return null;
        var rest = t[i..];
        if (rest.len > 0 and rest[0] == '|') {
            const close = std.mem.indexOfScalarPos(u8, rest, 1, '|') orelse return null;
            label = try cleanLabel(self.arena, rest[1..close]);
            rest = rest[close + 1 ..];
        }
        return .{
            .arrow = std.mem.indexOfAny(u8, op, ">ox") != null,
            .style = if (std.mem.indexOfScalar(u8, op, '=') != null) .thick else if (std.mem.indexOfScalar(u8, op, '.') != null) .dotted else .solid,
            .label = label,
            .rest = rest,
        };
    }

    /// `A & B --> C --> D`
    fn parseStatement(self: *Flow, stmt: []const u8) !void {
        var rest = stmt;
        var current: std.ArrayListUnmanaged(usize) = .empty;
        while (true) {
            const n = (try self.parseNode(rest)) orelse return;
            try current.append(self.arena, n.index);
            rest = std.mem.trimStart(u8, n.rest, " \t");
            if (rest.len > 0 and rest[0] == '&') {
                rest = rest[1..];
                continue;
            }
            break;
        }
        while (true) {
            const link = (try self.parseLink(rest)) orelse return;
            rest = link.rest;
            var targets: std.ArrayListUnmanaged(usize) = .empty;
            while (true) {
                const n = (try self.parseNode(rest)) orelse return;
                try targets.append(self.arena, n.index);
                rest = std.mem.trimStart(u8, n.rest, " \t");
                if (rest.len > 0 and rest[0] == '&') {
                    rest = rest[1..];
                    continue;
                }
                break;
            }
            for (current.items) |from| for (targets.items) |to| {
                try self.edges.append(self.arena, .{ .from = from, .to = to, .label = link.label, .arrow = link.arrow, .style = link.style });
            };
            current = targets;
        }
    }
};

fn cleanLabel(arena: std.mem.Allocator, raw: []const u8) ![]const u8 {
    var t = stripQuotes(raw);
    // markdown strings "`**x**`" and <br> line breaks: keep text simple
    if (t.len >= 2 and t[0] == '`' and t[t.len - 1] == '`') t = t[1 .. t.len - 1];
    if (std.mem.indexOf(u8, t, "<br") == null and std.mem.indexOfScalar(u8, t, '*') == null) return t;
    var out: std.ArrayListUnmanaged(u8) = .empty;
    var i: usize = 0;
    while (i < t.len) {
        if (t[i] == '<') {
            if (std.mem.indexOfScalarPos(u8, t, i, '>')) |e| {
                try out.append(arena, ' ');
                i = e + 1;
                continue;
            }
        }
        if (t[i] != '*') try out.append(arena, t[i]);
        i += 1;
    }
    return out.toOwnedSlice(arena);
}

fn flowchart(arena: std.mem.Allocator, source: []const u8, o: Options) !?vector.Drawing {
    var flow: Flow = .{ .arena = arena };
    var direction: Direction = .td;
    var lines = std.mem.splitAny(u8, source, "\n;");
    var header_seen = false;
    while (lines.next()) |raw| {
        const line = std.mem.trim(u8, raw, " \t\r");
        if (line.len == 0 or std.mem.startsWith(u8, line, "%%")) continue;
        if (!header_seen) {
            header_seen = true;
            var words = std.mem.tokenizeScalar(u8, line, ' ');
            _ = words.next();
            if (words.next()) |dir| {
                if (std.ascii.eqlIgnoreCase(dir, "LR")) direction = .lr else if (std.ascii.eqlIgnoreCase(dir, "RL")) direction = .rl else if (std.ascii.eqlIgnoreCase(dir, "BT")) direction = .bt;
            }
            continue;
        }
        const skip = [_][]const u8{ "classDef", "class ", "style ", "linkStyle", "click ", "subgraph", "end", "direction" };
        var skipped = false;
        for (skip) |s| if (std.mem.startsWith(u8, line, s)) {
            skipped = true;
        };
        if (skipped) continue;
        try flow.parseStatement(line);
    }
    const nodes = flow.nodes.items;
    if (nodes.len == 0) return null;
    const fs = o.font_size;

    // measure nodes
    for (nodes) |*n| {
        const m = vector.measure(n.label, fs, .sans, 400, false);
        n.w = @max(m.advance + 28, 56);
        n.h = fs * 2.6;
        switch (n.shape) {
            .diamond => {
                n.w = @max(n.w * 1.35, n.h * 1.6);
                n.h *= 1.45;
            },
            .circle => {
                n.w = @max(n.w, n.h);
                n.h = n.w;
            },
            .hexagon => n.w += 16,
            else => {},
        }
    }

    // layering: longest path, ignoring back edges found by DFS
    const back = try arena.alloc(bool, flow.edges.items.len);
    @memset(back, false);
    {
        const state = try arena.alloc(u8, nodes.len); // 0 new, 1 active, 2 done
        @memset(state, 0);
        const Dfs = struct {
            fn visit(f: *Flow, st: []u8, bk: []bool, v: usize) void {
                st[v] = 1;
                for (f.edges.items, 0..) |e, k| {
                    if (e.from != v) continue;
                    if (st[e.to] == 1) {
                        bk[k] = true;
                    } else if (st[e.to] == 0) visit(f, st, bk, e.to);
                }
                st[v] = 2;
            }
        };
        for (0..nodes.len) |v| if (state[v] == 0) Dfs.visit(&flow, state, back, v);
    }
    var changed = true;
    var guard: usize = 0;
    while (changed and guard < nodes.len + 2) : (guard += 1) {
        changed = false;
        for (flow.edges.items, 0..) |e, k| {
            if (back[k] or e.from == e.to) continue;
            if (nodes[e.to].layer < nodes[e.from].layer + 1) {
                nodes[e.to].layer = nodes[e.from].layer + 1;
                changed = true;
            }
        }
    }
    var layer_count: usize = 0;
    for (nodes) |n| layer_count = @max(layer_count, n.layer + 1);

    // ordering: appearance order, then two barycenter sweeps
    for (nodes, 0..) |*n, i| n.order = @floatFromInt(i);
    for (0..2) |_| {
        for (nodes, 0..) |*n, v| {
            var sum: f32 = 0;
            var count: f32 = 0;
            for (flow.edges.items) |e| {
                if (e.to == v and nodes[e.from].layer < n.layer) {
                    sum += nodes[e.from].order;
                    count += 1;
                }
            }
            if (count > 0) n.order = sum / count + @as(f32, @floatFromInt(v)) * 0.001;
        }
    }

    const horizontal = direction == .lr or direction == .rl;
    const rank_gap: f32 = if (horizontal) 56 else 48;
    const node_gap: f32 = 28;
    // per layer: sorted members, extent along the cross axis
    const layer_main = try arena.alloc(f32, layer_count); // thickness of layer along main axis
    @memset(layer_main, 0);
    for (nodes) |n| layer_main[n.layer] = @max(layer_main[n.layer], if (horizontal) n.w else n.h);
    var cross_max: f32 = 0;
    const members = try arena.alloc(std.ArrayListUnmanaged(usize), layer_count);
    for (members) |*m| m.* = .empty;
    for (nodes, 0..) |n, i| try members[n.layer].append(arena, i);
    for (members) |*m| {
        std.mem.sort(usize, m.items, nodes, struct {
            fn lt(ns: []FNode, a: usize, b: usize) bool {
                return ns[a].order < ns[b].order;
            }
        }.lt);
        var extent: f32 = 0;
        for (m.items, 0..) |idx, k| {
            if (k > 0) extent += node_gap;
            extent += if (horizontal) nodes[idx].h else nodes[idx].w;
        }
        cross_max = @max(cross_max, extent);
    }
    var main_pos: f32 = 8;
    for (members, 0..) |m, layer| {
        var extent: f32 = 0;
        for (m.items, 0..) |idx, k| {
            if (k > 0) extent += node_gap;
            extent += if (horizontal) nodes[idx].h else nodes[idx].w;
        }
        var cross = 8 + (cross_max - extent) / 2;
        for (m.items) |idx| {
            const n = &nodes[idx];
            if (horizontal) {
                n.x = main_pos + (layer_main[layer] - n.w) / 2;
                n.y = cross;
                cross += n.h + node_gap;
            } else {
                n.y = main_pos + (layer_main[layer] - n.h) / 2;
                n.x = cross;
                cross += n.w + node_gap;
            }
        }
        main_pos += layer_main[layer] + rank_gap;
    }
    var width: f32 = 0;
    var height: f32 = 0;
    for (nodes) |n| {
        width = @max(width, n.x + n.w + 8);
        height = @max(height, n.y + n.h + 8);
    }
    // flip for BT / RL
    if (direction == .bt or direction == .rl) for (nodes) |*n| {
        if (direction == .bt) n.y = height - n.y - n.h else n.x = width - n.x - n.w;
    };

    var c: Canvas = .{ .arena = arena, .o = o };
    const colors = o.colors;
    var back_lane: f32 = 0;
    // edges first (under nodes)
    for (flow.edges.items, 0..) |e, k| {
        const a = nodes[e.from];
        const b = nodes[e.to];
        const lw: f32 = if (e.style == .thick) 2.6 else 1.3;
        const dashed = e.style == .dotted;
        var label_x: f32 = 0;
        var label_y: f32 = 0;
        if (back[k] or e.from == e.to) {
            // route around the side of the diagram
            back_lane += 14;
            if (horizontal) {
                const y = height + back_lane - 6;
                const ax = a.x + a.w / 2;
                const bx = b.x + b.w / 2;
                try c.line(ax, a.y + a.h, ax, y, colors.edge, lw, dashed);
                try c.line(ax, y, bx, y, colors.edge, lw, dashed);
                try c.line(bx, y, bx, b.y + b.h, colors.edge, lw, dashed);
                if (e.arrow) try c.arrowHead(bx, b.y + b.h, 0, -1, colors.edge);
                label_x = (ax + bx) / 2;
                label_y = y;
            } else {
                const x = width + back_lane - 6;
                const ay = a.y + a.h / 2;
                const by = b.y + b.h / 2;
                try c.line(a.x + a.w, ay, x, ay, colors.edge, lw, dashed);
                try c.line(x, ay, x, by, colors.edge, lw, dashed);
                try c.line(x, by, b.x + b.w, by, colors.edge, lw, dashed);
                if (e.arrow) try c.arrowHead(b.x + b.w, by, -1, 0, colors.edge);
                label_x = x;
                label_y = (ay + by) / 2;
            }
        } else if (horizontal) {
            const forward = b.x >= a.x;
            const x0 = if (forward) a.x + a.w else a.x;
            const x1 = if (forward) b.x else b.x + b.w;
            const y0 = a.y + a.h / 2;
            const y1 = b.y + b.h / 2;
            const mid = (x0 + x1) / 2;
            if (@abs(y0 - y1) < 1) {
                try c.line(x0, y0, x1, y1, colors.edge, lw, dashed);
            } else {
                try c.poly(&.{ .{ x0, y0 }, .{ mid, y0 }, .{ mid, y1 }, .{ x1, y1 } }, colors.edge, lw, false);
            }
            if (e.arrow) try c.arrowHead(x1, y1, if (forward) 1 else -1, 0, colors.edge);
            label_x = mid;
            label_y = (y0 + y1) / 2;
        } else {
            const down = b.y >= a.y;
            const x0 = a.x + a.w / 2;
            const x1 = b.x + b.w / 2;
            const y0 = if (down) a.y + a.h else a.y;
            const y1 = if (down) b.y else b.y + b.h;
            const mid = y1 - (if (down) rank_gap / 2 else -rank_gap / 2);
            if (@abs(x0 - x1) < 1) {
                try c.line(x0, y0, x1, y1, colors.edge, lw, dashed);
            } else {
                try c.poly(&.{ .{ x0, y0 }, .{ x0, mid }, .{ x1, mid }, .{ x1, y1 } }, colors.edge, lw, false);
            }
            if (e.arrow) try c.arrowHead(x1, y1, 0, if (down) 1 else -1, colors.edge);
            label_x = (x0 + x1) / 2;
            label_y = if (@abs(x0 - x1) < 1) (y0 + y1) / 2 else mid;
        }
        if (e.label.len > 0) {
            const m = vector.measure(e.label, fs * 0.86, .sans, 400, false);
            try c.rect(label_x - m.advance / 2 - 4, label_y - fs * 0.62, m.advance + 8, fs * 1.24, colors.label_bg, 3);
            try c.centeredText(label_x, label_y, e.label, fs * 0.86, colors.muted);
        }
    }
    // nodes
    for (nodes) |n| {
        switch (n.shape) {
            .diamond => {
                const pts = [_]vector.Point{ .{ n.x + n.w / 2, n.y }, .{ n.x + n.w, n.y + n.h / 2 }, .{ n.x + n.w / 2, n.y + n.h }, .{ n.x, n.y + n.h / 2 } };
                try c.poly(&pts, colors.node_fill, 0, true);
                try c.poly(&pts, colors.node_border, 1.2, true);
            },
            .hexagon => {
                const d: f32 = 12;
                const pts = [_]vector.Point{ .{ n.x + d, n.y }, .{ n.x + n.w - d, n.y }, .{ n.x + n.w, n.y + n.h / 2 }, .{ n.x + n.w - d, n.y + n.h }, .{ n.x + d, n.y + n.h }, .{ n.x, n.y + n.h / 2 } };
                try c.poly(&pts, colors.node_fill, 0, true);
                try c.poly(&pts, colors.node_border, 1.2, true);
            },
            .asymmetric => {
                const d: f32 = 12;
                const pts = [_]vector.Point{ .{ n.x, n.y }, .{ n.x + n.w, n.y }, .{ n.x + n.w, n.y + n.h }, .{ n.x, n.y + n.h }, .{ n.x + d, n.y + n.h / 2 } };
                try c.poly(&pts, colors.node_fill, 0, true);
                try c.poly(&pts, colors.node_border, 1.2, true);
            },
            else => {
                const r: f32 = switch (n.shape) {
                    .round => 8,
                    .stadium => n.h / 2,
                    .circle => n.w / 2,
                    .cylinder => 10,
                    else => 3,
                };
                try c.rect(n.x, n.y, n.w, n.h, colors.node_fill, r);
                try c.roundedBorder(n.x, n.y, n.w, n.h, @min(r, @min(n.w, n.h) / 2), colors.node_border);
                if (n.shape == .subroutine) {
                    try c.line(n.x + 7, n.y, n.x + 7, n.y + n.h, colors.node_border, 1.2, false);
                    try c.line(n.x + n.w - 7, n.y, n.x + n.w - 7, n.y + n.h, colors.node_border, 1.2, false);
                }
            },
        }
        try c.centeredText(n.x + n.w / 2, n.y + n.h / 2, n.label, fs, colors.text);
    }
    if (horizontal) height += back_lane else width += back_lane;
    return try anchorTop(arena, c.finish(width, height));
}

// ── sequence diagrams ─────────────────────────────────────────────────────

const Message = struct {
    from: usize,
    to: usize,
    text: []const u8,
    dashed: bool,
    arrow: bool,
    note: bool = false,
    /// note spans from..to (inclusive) or sits right/left of `from`
    note_side: enum { over, right, left } = .over,
};

fn sequence(arena: std.mem.Allocator, source: []const u8, o: Options) !?vector.Drawing {
    var names: std.ArrayListUnmanaged([]const u8) = .empty;
    var labels: std.ArrayListUnmanaged([]const u8) = .empty;
    var messages: std.ArrayListUnmanaged(Message) = .empty;
    const Find = struct {
        fn index(a: std.mem.Allocator, n: *std.ArrayListUnmanaged([]const u8), l: *std.ArrayListUnmanaged([]const u8), name: []const u8) !usize {
            const t = std.mem.trim(u8, name, " \t");
            for (n.items, 0..) |x, i| if (std.mem.eql(u8, x, t)) return i;
            try n.append(a, t);
            try l.append(a, t);
            return n.items.len - 1;
        }
    };
    var lines = std.mem.splitScalar(u8, source, '\n');
    _ = lines.next();
    while (lines.next()) |raw| {
        const line = std.mem.trim(u8, raw, " \t\r");
        if (line.len == 0 or std.mem.startsWith(u8, line, "%%")) continue;
        if (std.mem.startsWith(u8, line, "participant ") or std.mem.startsWith(u8, line, "actor ")) {
            const rest = line[std.mem.indexOfScalar(u8, line, ' ').? + 1 ..];
            if (std.mem.indexOf(u8, rest, " as ")) |p| {
                const idx = try Find.index(arena, &names, &labels, rest[0..p]);
                labels.items[idx] = std.mem.trim(u8, rest[p + 4 ..], " ");
            } else _ = try Find.index(arena, &names, &labels, rest);
            continue;
        }
        if (std.mem.startsWith(u8, line, "Note ") or std.mem.startsWith(u8, line, "note ")) {
            const colon = std.mem.indexOfScalar(u8, line, ':') orelse continue;
            const head = line[5..colon];
            const text = std.mem.trim(u8, line[colon + 1 ..], " ");
            var side: @TypeOf(@as(Message, undefined).note_side) = .over;
            var who = head;
            if (std.mem.startsWith(u8, head, "right of ")) {
                side = .right;
                who = head[9..];
            } else if (std.mem.startsWith(u8, head, "left of ")) {
                side = .left;
                who = head[8..];
            } else if (std.mem.startsWith(u8, head, "over ")) who = head[5..];
            var parts = std.mem.splitScalar(u8, who, ',');
            const a = try Find.index(arena, &names, &labels, parts.next().?);
            const b = if (parts.next()) |p| try Find.index(arena, &names, &labels, p) else a;
            try messages.append(arena, .{ .from = a, .to = b, .text = text, .dashed = false, .arrow = false, .note = true, .note_side = side });
            continue;
        }
        const ops = [_][]const u8{ "-->>", "->>", "--)", "-)", "--x", "-x", "-->", "->" };
        for (ops) |op| {
            if (std.mem.indexOf(u8, line, op)) |p| {
                const colon = std.mem.indexOfScalarPos(u8, line, p, ':') orelse line.len;
                const lhs = line[0..p];
                var rhs = line[p + op.len .. colon];
                rhs = std.mem.trim(u8, rhs, " +-");
                const a = try Find.index(arena, &names, &labels, lhs);
                const b = try Find.index(arena, &names, &labels, rhs);
                const text = if (colon < line.len) std.mem.trim(u8, line[colon + 1 ..], " ") else "";
                try messages.append(arena, .{ .from = a, .to = b, .text = text, .dashed = std.mem.startsWith(u8, op, "--"), .arrow = !std.mem.eql(u8, op, "->") and !std.mem.eql(u8, op, "-->") });
                break;
            }
        }
    }
    if (names.items.len == 0) return null;
    const fs = o.font_size;
    const colors = o.colors;
    const n = names.items.len;
    const box_w = try arena.alloc(f32, n);
    for (labels.items, 0..) |l, i| box_w[i] = vector.measure(l, fs, .sans, 400, false).advance + 28;
    // spacing between neighbours: fits the widest message between them
    const gaps = try arena.alloc(f32, n);
    @memset(gaps, 0);
    for (messages.items) |m| {
        if (m.note) continue;
        const lo = @min(m.from, m.to);
        const hi = @max(m.from, m.to);
        const w = vector.measure(m.text, fs * 0.9, .sans, 400, false).advance + 30;
        if (hi > lo) {
            const per = w / @as(f32, @floatFromInt(hi - lo));
            for (lo..hi) |k| gaps[k] = @max(gaps[k], per);
        }
    }
    const xs = try arena.alloc(f32, n);
    var x: f32 = 8 + box_w[0] / 2;
    for (0..n) |i| {
        xs[i] = x;
        if (i + 1 < n) x += @max((box_w[i] + box_w[i + 1]) / 2 + 30, gaps[i]);
    }
    const box_h = fs * 2.4;
    const row_h = fs * 2.6;
    var c: Canvas = .{ .arena = arena, .o = o };
    const rows_start = 8 + box_h + 20;
    var bottom = rows_start;
    for (messages.items) |m| bottom += if (!m.note and m.from == m.to) row_h + 6 else row_h;
    // lifelines and participant boxes (top and bottom) under the messages
    for (0..n) |i| {
        try c.line(xs[i], 8 + box_h, xs[i], bottom, colors.muted, 1, true);
        for ([_]f32{ 8, bottom }) |top| {
            try c.rect(xs[i] - box_w[i] / 2, top, box_w[i], box_h, colors.node_fill, 4);
            try c.roundedBorder(xs[i] - box_w[i] / 2, top, box_w[i], box_h, 4, colors.node_border);
            try c.centeredText(xs[i], top + box_h / 2, labels.items[i], fs, colors.text);
        }
    }
    var y: f32 = rows_start;
    for (messages.items) |m| {
        if (m.note) {
            const nw = vector.measure(m.text, fs * 0.88, .sans, 400, false).advance + 20;
            const spans = m.note_side == .over and m.from != m.to;
            const lo = @min(xs[m.from], xs[m.to]);
            const hi = @max(xs[m.from], xs[m.to]);
            const w = if (spans) @max(nw, hi - lo + 40) else nw;
            const lx = switch (m.note_side) {
                .right => xs[m.from] + 10,
                .left => xs[m.from] - 10 - nw,
                .over => (lo + hi) / 2 - w / 2,
            };
            try c.rect(lx, y - fs * 0.9, w, fs * 1.8, 0xfffff5ad, 3);
            try c.centeredText(lx + w / 2, y, m.text, fs * 0.88, 0xff333333);
            y += row_h;
            continue;
        }
        const x0 = xs[m.from];
        const x1 = xs[m.to];
        if (m.from == m.to) {
            const lw: f32 = 34;
            try c.poly(&.{ .{ x0, y - 4 }, .{ x0 + lw, y - 4 }, .{ x0 + lw, y + 10 }, .{ x0 + 4, y + 10 } }, colors.edge, 1.3, false);
            try c.arrowHead(x0, y + 10, -1, 0, colors.edge);
            try c.text(x0 + lw + 6, y + fs * 0.35, m.text, fs * 0.9, colors.text);
            y += row_h + 6;
            continue;
        }
        const dir: f32 = if (x1 > x0) 1 else -1;
        try c.line(x0, y, x1, y, colors.edge, 1.3, m.dashed);
        if (m.arrow) {
            try c.arrowHead(x1, y, dir, 0, colors.edge);
        } else {
            try c.poly(&.{ .{ x1 - dir * 8, y - 4 }, .{ x1, y }, .{ x1 - dir * 8, y + 4 } }, colors.edge, 1.3, false);
        }
        try c.centeredText((x0 + x1) / 2, y - fs * 0.75, m.text, fs * 0.9, colors.text);
        y += row_h;
    }
    const width = xs[n - 1] + box_w[n - 1] / 2 + 8 + 60;
    return try anchorTop(arena, c.finish(width, bottom + box_h + 8));
}

// ── pie charts ────────────────────────────────────────────────────────────

fn pie(arena: std.mem.Allocator, source: []const u8, o: Options) !?vector.Drawing {
    var title: []const u8 = "";
    var labels: std.ArrayListUnmanaged([]const u8) = .empty;
    var values: std.ArrayListUnmanaged(f32) = .empty;
    var lines = std.mem.splitScalar(u8, source, '\n');
    while (lines.next()) |raw| {
        const line = std.mem.trim(u8, raw, " \t\r");
        if (std.mem.startsWith(u8, line, "pie")) {
            if (std.mem.indexOf(u8, line, "title ")) |t| title = std.mem.trim(u8, line[t + 6 ..], " ");
            continue;
        }
        if (std.mem.startsWith(u8, line, "title ")) {
            title = std.mem.trim(u8, line[6..], " ");
            continue;
        }
        const colon = std.mem.lastIndexOfScalar(u8, line, ':') orelse continue;
        const value = std.fmt.parseFloat(f32, std.mem.trim(u8, line[colon + 1 ..], " ")) catch continue;
        if (value <= 0) continue;
        try labels.append(arena, stripQuotes(line[0..colon]));
        try values.append(arena, value);
    }
    if (values.items.len == 0) return null;
    var total: f32 = 0;
    for (values.items) |v| total += v;
    const fs = o.font_size;
    const r: f32 = 90;
    const cx: f32 = 8 + r;
    var top: f32 = 8;
    var c: Canvas = .{ .arena = arena, .o = o };
    if (title.len > 0) {
        try c.text(8, top + fs, title, fs * 1.1, o.colors.text);
        top += fs * 2;
    }
    const cy = top + r;
    var angle: f32 = -std.math.pi / 2.0;
    var legend_w: f32 = 0;
    for (values.items, 0..) |v, i| {
        const sweep = v / total * std.math.tau;
        const steps: usize = @max(2, @as(usize, @intFromFloat(sweep / std.math.tau * 72)));
        var pts: std.ArrayListUnmanaged(vector.Point) = .empty;
        try pts.append(arena, .{ cx, cy });
        for (0..steps + 1) |k| {
            const a = angle + sweep * @as(f32, @floatFromInt(k)) / @as(f32, @floatFromInt(steps));
            try pts.append(arena, .{ cx + @cos(a) * r, cy + @sin(a) * r });
        }
        const color = palette[i % palette.len];
        try c.poly(pts.items, color, 0, true);
        try c.poly(pts.items, o.colors.label_bg, 1.5, true);
        angle += sweep;
        // legend
        const ly = top + 6 + @as(f32, @floatFromInt(i)) * fs * 1.8;
        try c.rect(cx + r + 28, ly, fs * 0.9, fs * 0.9, color, 2);
        const pct = try std.fmt.allocPrint(arena, "{s}  {d:.1}%", .{ labels.items[i], v / total * 100 });
        try c.text(cx + r + 28 + fs * 1.4, ly + fs * 0.8, pct, fs * 0.92, o.colors.text);
        legend_w = @max(legend_w, vector.measure(pct, fs * 0.92, .sans, 400, false).advance);
    }
    const legend_h = @as(f32, @floatFromInt(values.items.len)) * fs * 1.8;
    return try anchorTop(arena, c.finish(cx + r + 28 + fs * 1.4 + legend_w + 8, @max(cy + r, top + legend_h) + 8));
}

test "flowchart parsing" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    var flow: Flow = .{ .arena = arena.allocator() };
    try flow.parseStatement("A[Start] --> B{Is it?} -->|Yes| C(Done)");
    try flow.parseStatement("B -- No --> D[[Retry]]");
    try std.testing.expectEqual(@as(usize, 4), flow.nodes.items.len);
    try std.testing.expectEqual(@as(usize, 3), flow.edges.items.len);
    try std.testing.expectEqualStrings("Yes", flow.edges.items[1].label);
    try std.testing.expectEqualStrings("No", flow.edges.items[2].label);
    try std.testing.expectEqual(Shape.diamond, flow.nodes.items[1].shape);
    try std.testing.expectEqual(Kind.flowchart, detect("%% c\ngraph TD\nA-->B").?);
}

test "wide flowcharts shrink to max_width" {
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const src = "flowchart LR\n  A[A very long first node] --> B[Another long node]\n  B --> C[Third long node] --> D[Fourth long node] --> E[Fifth long node]";
    const colors: Colors = .{ .text = 0xff000000, .node_fill = 0xffeeeeee, .node_border = 0xff999999, .edge = 0xff666666, .label_bg = 0xffffffff, .muted = 0xff888888 };
    const wide = (try render(arena.allocator(), src, .{ .max_width = 0, .colors = colors })).?;
    try std.testing.expect(wide.width > 300);
    const fitted = (try render(arena.allocator(), src, .{ .max_width = 300, .colors = colors })).?;
    try std.testing.expectApproxEqAbs(@as(f32, 300), fitted.width, 0.5);
}
