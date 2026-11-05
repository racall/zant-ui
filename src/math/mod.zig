//! Native LaTeX math (the `zant.math` component).
//!
//! Mirrors what Streamdown does with KaTeX: the TeX subset KaTeX supports is
//! parsed (`parser.zig`) and laid out with KaTeX's algorithms and font
//! metrics (`layout.zig`), then drawn with STIX Two Math as a retained vector
//! drawing. Like KaTeX with `throwOnError: false`, a formula that fails to
//! parse is shown as its source in `error_color`.
const std = @import("std");
const hash_mod = @import("../core/hash.zig");
const ui = @import("../ui.zig");
const vector = @import("../core/vector.zig");
pub const parser = @import("parser.zig");
pub const layout_engine = @import("layout.zig");
pub const symbols = @import("symbols.zig");
pub const fonts = @import("fonts.zig");

pub const Drawing = vector.Drawing;

pub const Options = struct {
    /// Font size of the surrounding text (px).
    size: f32 = 16,
    /// Math is drawn `scale` times larger than the surrounding text
    /// (KaTeX: 1.21 with Computer Modern; STIX Two has a larger x-height).
    scale: f32 = 1.1,
    color: u32 = 0xff292d2a,
    /// Streamdown's default `errorColor` is the muted foreground.
    error_color: u32 = 0xff8b918c,
    /// Display (block) style; false = inline/text style.
    display: bool = true,
    /// Shrinks the formula to fit this width (display math).
    max_width: ?f32 = null,
};

/// Lays out `source` into a drawing allocated in `arena`.
pub fn layout(arena: std.mem.Allocator, source: []const u8, options: Options) !Drawing {
    var drawing = try layoutOnce(arena, source, options, options.size * options.scale);
    if (options.max_width) |max| {
        if (drawing.width > max and drawing.width > 0) {
            const factor = @max(0.5, max / drawing.width);
            drawing = try layoutOnce(arena, source, options, options.size * options.scale * factor);
        }
    }
    var h = std.hash.Wyhash.init(0x6d617468);
    h.update(source);
    hash_mod.hashValue(&h, options);
    drawing.hash = h.final();
    return drawing;
}

fn layoutOnce(arena: std.mem.Allocator, source: []const u8, options: Options, size: f32) !Drawing {
    const nodes = parser.parse(arena, source) catch |err| switch (err) {
        error.ParseError => return errorDrawing(arena, source, options, size),
        else => |e| return e,
    };
    var engine = layout_engine.Engine.init(arena, .{ .size = size, .color = options.color, .error_color = options.error_color });
    const style = if (options.display) layout_engine.Style.display else layout_engine.Style.text;
    var box = try engine.list(nodes, style);
    if (engine.tag) |tag| {
        // \tag{n}: placed after a quad
        var cmds: std.ArrayListUnmanaged(vector.Command) = .empty;
        try cmds.appendSlice(arena, box.cmds);
        const x = box.width + 2 * size;
        for (tag.cmds) |cmd| try cmds.append(arena, try vector.translate(arena, cmd, x, 0));
        box.cmds = try cmds.toOwnedSlice(arena);
        box.width = x + tag.width;
        box.height = @max(box.height, tag.height);
        box.depth = @max(box.depth, tag.depth);
    }
    return .{ .width = box.width, .ascent = box.height, .descent = box.depth, .commands = box.cmds };
}

fn errorDrawing(arena: std.mem.Allocator, source: []const u8, options: Options, size: f32) !Drawing {
    const text = std.mem.trim(u8, source, " \t\n");
    const m = vector.measure(text, size * 0.9, .math, 400, false);
    const cmds = try arena.alloc(vector.Command, 1);
    cmds[0] = .{ .text = .{ .x = 0, .y = 0, .text = text, .size = size * 0.9, .family = .math, .color = options.error_color } };
    return .{ .width = m.advance, .ascent = @max(m.ascent, m.font_ascent), .descent = @max(m.descent, m.font_descent), .commands = cmds };
}

/// A centred display formula (KaTeX `.katex-display`: 1em vertical margin).
pub fn block(source: []const u8, options: Options) ui.Node {
    var opts = options;
    opts.display = true;
    const arena = ui.frameAllocator();
    const drawing = arena.create(Drawing) catch @panic("Zant UI: out of memory");
    drawing.* = layout(arena, source, opts) catch errorDrawing(arena, source, opts, opts.size) catch @panic("Zant UI: out of memory");
    return ui.row(.{
        .width_percent = 100,
        .justify = .center,
        .margin = .{ .top = 0.5 * opts.size, .bottom = 0.5 * opts.size },
        .children = &.{ui.vector(.{ .drawing = drawing })},
    });
}

/// Inline (text-style) formula for embedding in rich text.
pub fn inlineDrawing(arena: std.mem.Allocator, source: []const u8, options: Options) !*const Drawing {
    var opts = options;
    opts.display = false;
    const drawing = try arena.create(Drawing);
    drawing.* = try layout(arena, source, opts);
    // keep line metrics stable: never shorter than the surrounding text
    drawing.ascent = @max(drawing.ascent, 0.75 * opts.size);
    drawing.descent = @max(drawing.descent, 0.22 * opts.size);
    return drawing;
}

/// Backwards-compatible: display formula with default options.
pub fn render(allocator: std.mem.Allocator, source: []const u8) ui.Node {
    _ = allocator;
    return block(source, .{});
}

/// Plain-text rendering of a formula (copy, word counts, accessibility):
/// `\frac{a}{b}` → `a/b`, `x^2` → `x²`, `\alpha` → `α`.
pub fn inlineText(allocator: std.mem.Allocator, source: []const u8) ![]const u8 {
    var arena = std.heap.ArenaAllocator.init(allocator);
    defer arena.deinit();
    const nodes = parser.parse(arena.allocator(), source) catch return allocator.dupe(u8, source);
    var out: std.ArrayListUnmanaged(u8) = .empty;
    errdefer out.deinit(allocator);
    try flatten(allocator, &out, nodes);
    return out.toOwnedSlice(allocator);
}

const superscripts = [_][2][]const u8{
    .{ "0", "⁰" },
    .{ "1", "¹" },
    .{ "2", "²" },
    .{ "3", "³" },
    .{ "4", "⁴" },
    .{ "5", "⁵" },
    .{ "6", "⁶" },
    .{ "7", "⁷" },
    .{ "8", "⁸" },
    .{ "9", "⁹" },
    .{ "+", "⁺" },
    .{ "−", "⁻" },
    .{ "n", "ⁿ" },
    .{ "i", "ⁱ" },
};
const subscripts = [_][2][]const u8{
    .{ "0", "₀" },
    .{ "1", "₁" },
    .{ "2", "₂" },
    .{ "3", "₃" },
    .{ "4", "₄" },
    .{ "5", "₅" },
    .{ "6", "₆" },
    .{ "7", "₇" },
    .{ "8", "₈" },
    .{ "9", "₉" },
    .{ "+", "₊" },
    .{ "−", "₋" },
    .{ "i", "ᵢ" },
    .{ "n", "ₙ" },
};

fn scriptText(allocator: std.mem.Allocator, out: *std.ArrayListUnmanaged(u8), node: *const parser.Node, sup: bool) !void {
    var tmp: std.ArrayListUnmanaged(u8) = .empty;
    defer tmp.deinit(allocator);
    try flatten(allocator, &tmp, &.{node.*});
    const table = if (sup) &superscripts else &subscripts;
    var mapped: std.ArrayListUnmanaged(u8) = .empty;
    defer mapped.deinit(allocator);
    var it = (std.unicode.Utf8View.init(tmp.items) catch {
        try out.appendSlice(allocator, if (sup) "^" else "_");
        try out.appendSlice(allocator, tmp.items);
        return;
    }).iterator();
    while (it.nextCodepointSlice()) |cp| {
        var found: ?[]const u8 = null;
        for (table) |entry| if (std.mem.eql(u8, entry[0], cp)) {
            found = entry[1];
        };
        if (found) |f| try mapped.appendSlice(allocator, f) else {
            try out.appendSlice(allocator, if (sup) "^(" else "_(");
            try out.appendSlice(allocator, tmp.items);
            try out.appendSlice(allocator, ")");
            return;
        }
    }
    try out.appendSlice(allocator, mapped.items);
}

fn flatten(allocator: std.mem.Allocator, out: *std.ArrayListUnmanaged(u8), nodes: []const parser.Node) anyerror!void {
    for (nodes) |node| switch (node) {
        .atom => |a| try out.appendSlice(allocator, a.text),
        .group => |g| try flatten(allocator, out, g),
        .supsub => |s| {
            if (s.base) |b| try flatten(allocator, out, &.{b.*});
            if (s.sub) |n| try scriptText(allocator, out, n, false);
            if (s.sup) |n| try scriptText(allocator, out, n, true);
        },
        .frac => |f| {
            try out.appendSlice(allocator, "(");
            try flatten(allocator, out, f.num);
            try out.appendSlice(allocator, ")/(");
            try flatten(allocator, out, f.den);
            try out.appendSlice(allocator, ")");
        },
        .sqrt => |s| {
            try out.appendSlice(allocator, "√(");
            try flatten(allocator, out, s.body);
            try out.appendSlice(allocator, ")");
        },
        .left_right => |lr| {
            try out.appendSlice(allocator, lr.left);
            try flatten(allocator, out, lr.body);
            try out.appendSlice(allocator, lr.right);
        },
        .sized_delim => |d| try out.appendSlice(allocator, d.delim),
        .accent => |a| try flatten(allocator, out, a.base),
        .over_under => |o| try flatten(allocator, out, o.body),
        .op => |o| {
            try out.appendSlice(allocator, o.text);
            if (!o.symbol) try out.append(allocator, ' ');
        },
        .text => |t| try out.appendSlice(allocator, t.text),
        .space => try out.append(allocator, ' '),
        .array => |a| for (a.rows, 0..) |row, r| {
            if (r > 0) try out.appendSlice(allocator, "; ");
            for (row, 0..) |cell, c| {
                if (c > 0) try out.appendSlice(allocator, ", ");
                try flatten(allocator, out, cell);
            }
        },
        .color => |c| try flatten(allocator, out, c.body),
        .style => |s| try flatten(allocator, out, s.body),
        .phantom => {},
        .boxed => |b| try flatten(allocator, out, b),
        .cancel => |c| try flatten(allocator, out, c.body),
        .stack => |s| try flatten(allocator, out, &.{s.base.*}),
        .tag => |t| {
            try out.appendSlice(allocator, " (");
            try out.appendSlice(allocator, t);
            try out.appendSlice(allocator, ")");
        },
        .unknown => |name| {
            try out.append(allocator, '\\');
            try out.appendSlice(allocator, name);
        },
    };
}

test {
    _ = parser;
    _ = fonts;
}
