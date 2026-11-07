//! Block-level components (Streamdown `lib/components.tsx`).
const std = @import("std");
const ui = @import("../../ui.zig");
const text_mod = @import("../../renderer/text.zig");
const ast = @import("../ast.zig");
const safety = @import("../safety.zig");
const remend = @import("../remend/mod.zig");
const math = @import("../../math/mod.zig");
const inline_parser = @import("../parser/inline.zig");
const Ctx = @import("context.zig").Ctx;
const spans = @import("spans.zig");
const code_block = @import("code_block.zig");
const table = @import("table.zig");
const diagram = @import("diagram.zig");
const data_images = @import("data_images.zig");
const image_mod = @import("../../image.zig");

/// True when the block ends with inline text the caret can follow.
fn takesCaret(block: ast.Block) bool {
    return switch (block) {
        .paragraph, .heading, .code => true,
        .list => |l| l.items.len > 0 and l.items[l.items.len - 1].blocks.len > 0 and takesCaret(l.items[l.items.len - 1].blocks[l.items[l.items.len - 1].blocks.len - 1]),
        .quote => |q| q.len > 0 and takesCaret(q[q.len - 1]),
        else => false,
    };
}

/// Renders a sequence of blocks; the caret goes after the last one.
pub fn sequence(ctx: Ctx, blocks: []const ast.Block, gap: f32) anyerror!ui.Node {
    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    for (blocks, 0..) |block, i| {
        var c = ctx;
        const last = i + 1 == blocks.len;
        c.caret = ctx.caret and last and takesCaret(block);
        c.key = ctx.subKey("{d}", .{i});
        try children.append(ctx.arena, try render(c, block));
    }
    if (ctx.caret and (blocks.len == 0 or !takesCaret(blocks[blocks.len - 1]))) {
        try children.append(ctx.arena, try spans.body(ctx, &.{}, true));
    }
    return ui.column(.{ .key = ctx.key, .width_percent = 100, .gap = gap, .align_items = .stretch, .children = children.items });
}

pub fn render(ctx: Ctx, block: ast.Block) anyerror!ui.Node {
    const theme = &ctx.options.theme;
    return switch (block) {
        .heading => |h| heading(ctx, h),
        .paragraph => |p| spans.body(ctx, p, ctx.caret),
        .code => |code| customBlock(ctx, code) orelse if (std.ascii.eqlIgnoreCase(code.lang, "mermaid")) diagram.render(ctx, code, ctx.caret) else code_block.render(ctx, code, ctx.caret),
        .quote => |q| quote(ctx, q),
        .callout => |c| callout(ctx, c),
        .list => |l| list(ctx, l, 0),
        .table => |t| table.render(ctx, t),
        .math => |m| mathBlock(ctx, m),
        .image => |img| image(ctx, img),
        .html => |raw| html(ctx, raw),
        .rule => ui.divider(.{ .key = ctx.key, .color = theme.border, .margin = .{ .top = ctx.size * 0.5, .bottom = ctx.size * 0.5 } }),
    };
}

/// A fenced block handed to an app renderer (`Options.block_renderers`).
fn customBlock(ctx: Ctx, code: ast.Code) ?ui.Node {
    for (ctx.options.block_renderers) |r| {
        if (!std.ascii.eqlIgnoreCase(r.lang, code.lang)) continue;
        const node = r.func(r.ctx, .{
            .lang = code.lang,
            .meta = code.meta,
            .code = code.text,
            .complete = code.closed,
            .key = ctx.key,
            .arena = ctx.arena,
        }) orelse return null;
        return ui.column(.{ .key = ctx.key, .width_percent = 100, .align_items = .stretch, .children = &.{node} });
    }
    return null;
}

/// Decoded image for a URL: `data:` URIs, or the app's loader for remote
/// and custom URLs.
pub fn loadImage(ctx: Ctx, url: []const u8) ?image_mod.Image {
    if (std.mem.startsWith(u8, url, "data:")) return data_images.get(url);
    const loader = ctx.options.image_loader orelse return null;
    return loader.get(url);
}

fn heading(ctx: Ctx, h: ast.Heading) !ui.Node {
    const theme = &ctx.options.theme;
    const scale = theme.heading_scale[@min(h.level, 6) - 1];
    const node = try spans.richNode(ctx, h.spans, .{
        .size = ctx.size * scale,
        .color = theme.heading,
        .weight = theme.heading_weight,
        .family = theme.heading_family,
    }, ctx.caret);
    // headings get extra space above (Streamdown: mt-6 mb-2)
    return ui.column(.{
        .key = ctx.key,
        .width_percent = 100,
        .padding = .{ .top = if (h.level <= 2) ctx.size * 0.6 else ctx.size * 0.3 },
        .children = &.{node},
    });
}

fn quote(ctx: Ctx, blocks: []const ast.Block) !ui.Node {
    const theme = &ctx.options.theme;
    const inner = ctx.with(theme.quote_text, ctx.size);
    return ui.row(.{
        .key = ctx.key,
        .width_percent = 100,
        .align_items = .stretch,
        .children = &.{
            ui.styled(ui.space(.{ .width = 3 }), .{ .background = theme.quote_bar, .radius = 1.5 }),
            ui.column(.{
                .grow = 1,
                .shrink = 1,
                .basis = 0,
                .padding = .{ .left = 16 },
                .align_items = .stretch,
                .children = &.{try sequence(inner, blocks, ctx.size * 0.75)},
            }),
        },
    });
}

fn callout(ctx: Ctx, c: ast.Callout) !ui.Node {
    const theme = &ctx.options.theme;
    const color = switch (c.kind) {
        .note => theme.note,
        .tip => theme.tip,
        .important => theme.important,
        .warning => theme.warning,
        .caution => theme.caution,
    };
    const icon = switch (c.kind) {
        .note => "ⓘ",
        .tip => "💡",
        .important => "❢",
        .warning => "⚠",
        .caution => "⛔",
    };
    const title = ui.row(.{ .gap = 6, .align_items = .center, .children = &.{
        ui.text(icon, .{ .size = ctx.size * 0.9, .color = color }),
        ui.text(c.kind.label(), .{ .size = ctx.size * 0.9, .weight = .semibold, .color = color }),
    } });
    return ui.row(.{
        .key = ctx.key,
        .width_percent = 100,
        .align_items = .stretch,
        .children = &.{
            ui.styled(ui.space(.{ .width = 3 }), .{ .background = color, .radius = 1.5 }),
            ui.column(.{
                .grow = 1,
                .shrink = 1,
                .basis = 0,
                .padding = .{ .left = 16, .top = 2, .bottom = 2 },
                .gap = ctx.size * 0.5,
                .align_items = .stretch,
                .children = &.{ title, try sequence(ctx, c.blocks, ctx.size * 0.75) },
            }),
        },
    });
}

fn list(ctx: Ctx, l: ast.List, depth: usize) anyerror!ui.Node {
    const theme = &ctx.options.theme;
    var items: std.ArrayListUnmanaged(ui.Node) = .empty;
    // third level: a small square. "▪" has an emoji form that font fallback
    // picks (and that ignores the colour), so a half-size "■" is used.
    const bullets = [_][]const u8{ "•", "◦", "■" };
    var marker_width: f32 = ctx.size * 1.1;
    if (l.ordered) {
        const last_number = l.start + l.items.len;
        marker_width = @as(f32, @floatFromInt(std.fmt.count("{d}", .{last_number}) + 1)) * ctx.size * 0.6 + 6;
    }
    for (l.items, 0..) |item, i| {
        var c = ctx;
        c.caret = ctx.caret and i + 1 == l.items.len;
        c.key = ctx.subKey("i{d}", .{i});
        const marker_text: []const u8 = if (item.checked) |checked|
            (if (checked) "☑" else "☐")
        else if (l.ordered)
            try std.fmt.allocPrint(ctx.arena, "{d}.", .{l.start + i})
        else
            bullets[depth % bullets.len];
        const square = item.checked == null and !l.ordered and depth % bullets.len == 2;
        const marker_label = ui.text(marker_text, .{
            .size = if (square) ctx.size * 0.5 else ctx.size,
            .color = if (item.checked != null and item.checked.?) theme.link else theme.marker,
            // same line box as the item text, so the marker stays centred on it
            .line_height = if (square) theme.line_height * 2 else theme.line_height,
        });
        // markers are not copied (like list markers in a browser)
        const marker = ui.row(.{ .justify = if (l.ordered) .end else .start, .unselectable = true, .children = &.{marker_label} });
        // nested lists render one level deeper
        var children: std.ArrayListUnmanaged(ui.Node) = .empty;
        for (item.blocks, 0..) |block, b| {
            var bc = c;
            bc.caret = c.caret and b + 1 == item.blocks.len;
            bc.key = c.subKey("{d}", .{b});
            const node = switch (block) {
                .list => |nested| try list(bc, nested, depth + 1),
                else => try render(bc, block),
            };
            try children.append(ctx.arena, node);
        }
        if (children.items.len == 0) try children.append(ctx.arena, try spans.body(c, &.{}, c.caret));
        try items.append(ctx.arena, ui.row(.{
            .key = c.key,
            .width_percent = 100,
            .align_items = .start,
            .children = &.{
                ui.styled(marker, .{ .width = marker_width }),
                ui.column(.{
                    .grow = 1,
                    .shrink = 1,
                    .basis = 0,
                    .padding = .{ .left = 6 },
                    .gap = if (l.tight) ctx.size * 0.25 else ctx.size * 0.75,
                    .align_items = .stretch,
                    .children = children.items,
                }),
            },
        }));
    }
    return ui.column(.{
        .key = ctx.key,
        .width_percent = 100,
        .gap = if (l.tight) ctx.size * 0.25 else ctx.size * 0.6,
        .align_items = .stretch,
        .children = items.items,
    });
}

fn mathBlock(ctx: Ctx, m: ast.Math) !ui.Node {
    const theme = &ctx.options.theme;
    const drawing = try ctx.arena.create(math.Drawing);
    drawing.* = try math.layout(ctx.arena, m.source, .{
        .size = ctx.size,
        .color = theme.math,
        .error_color = theme.math_error,
        .display = true,
        .max_width = ctx.options.content_width,
    });
    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    try children.append(ctx.arena, ui.vector(.{ .drawing = drawing, .copy = try std.mem.concat(ctx.arena, u8, &.{ "$$\n", m.source, "\n$$" }) }));
    if (ctx.caret) try children.append(ctx.arena, ui.text(ctx.options.caret.glyph(), .{ .size = ctx.size * 0.9, .color = theme.caret }));
    return ui.row(.{
        .key = ctx.key,
        .width_percent = 100,
        .justify = .center,
        .align_items = .center,
        .padding = .{ .top = ctx.size * 0.25, .bottom = ctx.size * 0.25 },
        .children = children.items,
    });
}

fn resolveImagePath(ctx: Ctx, url: []const u8) ![]const u8 {
    if (std.mem.startsWith(u8, url, "/") or ctx.options.base_dir.len == 0) return url;
    var rel = url;
    if (std.mem.startsWith(u8, rel, "./")) rel = rel[2..];
    return std.fs.path.join(ctx.arena, &.{ ctx.options.base_dir, rel });
}

fn image(ctx: Ctx, img: ast.Image) !ui.Node {
    const theme = &ctx.options.theme;
    const placeholder = std.mem.eql(u8, img.url, remend.incomplete_image);
    const has_scheme = std.mem.indexOf(u8, img.url, "://") != null or std.mem.startsWith(u8, img.url, "data:");
    const allowed = !placeholder and safety.check(ctx.options.policy, .image, img.url);
    const loaded: ?image_mod.Image = if (allowed and has_scheme) loadImage(ctx, img.url) else null;
    if (allowed and (!has_scheme or loaded != null)) {
        var children: std.ArrayListUnmanaged(ui.Node) = .empty;
        const node = if (loaded) |decoded|
            ui.image(.{ .key = ctx.subKey("img", .{}), .image = decoded, .fit = .contain, .radius = 8 })
        else
            ui.image(.{ .key = ctx.subKey("img", .{}), .src = try resolveImagePath(ctx, img.url), .fit = .contain, .radius = 8 });
        try children.append(ctx.arena, ui.styled(node, .{ .max_width = ctx.options.content_width }));
        if (img.alt.len > 0) try children.append(ctx.arena, ui.text(img.alt, .{ .size = ctx.size * 0.8, .color = theme.muted_foreground }));
        return ui.column(.{ .key = ctx.key, .width_percent = 100, .gap = 6, .align_items = .center, .children = children.items });
    }
    // remote / blocked / still streaming: a labelled placeholder card
    const label = if (placeholder) ctx.options.labels.loading_image else if (img.alt.len > 0) img.alt else ctx.options.labels.image;
    var lines: std.ArrayListUnmanaged(ui.Node) = .empty;
    try lines.append(ctx.arena, ui.text(try std.mem.concat(ctx.arena, u8, &.{ "🖼  ", label }), .{ .size = ctx.size * 0.875, .color = theme.muted_foreground }));
    if (!placeholder and img.url.len > 0) try lines.append(ctx.arena, ui.text(img.url, .{ .size = ctx.size * 0.75, .color = theme.muted_foreground, .max_lines = 1 }));
    return ui.column(.{
        .key = ctx.key,
        .width_percent = 100,
        .padding = ui.inset(16),
        .gap = 4,
        .radius = 8,
        .background = theme.muted,
        .align_items = .center,
        .children = lines.items,
    });
}

fn html(ctx: Ctx, raw: []const u8) !ui.Node {
    // Streamdown sanitises HTML; natively we keep the text and inline
    // formatting (<b>, <em>, <code>, <a>, <br>) and drop block tags.
    var refs: inline_parser.RefMap = .empty;
    var order: std.ArrayListUnmanaged([]const u8) = .empty;
    var ictx: inline_parser.Context = .{ .arena = ctx.arena, .refs = &refs, .footnote_order = &order };
    var joined: std.ArrayListUnmanaged(u8) = .empty;
    var it = std.mem.splitScalar(u8, raw, '\n');
    while (it.next()) |line| {
        const t = std.mem.trim(u8, line, " \t");
        if (t.len == 0) continue;
        if (joined.items.len > 0) try joined.append(ctx.arena, '\n');
        try joined.appendSlice(ctx.arena, t);
    }
    const parsed = try inline_parser.parse(&ictx, joined.items);
    // drop the whitespace-only runs left by stripped block tags
    var kept: std.ArrayListUnmanaged(ast.Span) = .empty;
    for (parsed) |s| {
        if (std.mem.trim(u8, s.text, " \n\t").len == 0 and kept.items.len == 0) continue;
        try kept.append(ctx.arena, s);
    }
    if (kept.items.len == 0 and !ctx.caret) return ui.space(.{ .key = ctx.key, .height = 0 });
    return spans.body(ctx, kept.items, ctx.caret);
}

pub fn footnotes(ctx: Ctx, notes: []const ast.Footnote) !ui.Node {
    const theme = &ctx.options.theme;
    const small = ctx.with(theme.muted_foreground, ctx.size * 0.875);
    var rows: std.ArrayListUnmanaged(ui.Node) = .empty;
    try rows.append(ctx.arena, ui.divider(.{ .color = theme.border, .margin = .{ .top = ctx.size * 0.5, .bottom = ctx.size * 0.25 } }));
    try rows.append(ctx.arena, ui.text(ctx.options.labels.footnotes, .{ .size = small.size, .weight = .semibold, .color = theme.muted_foreground }));
    for (notes, 0..) |note, i| {
        var c = small;
        c.key = ctx.subKey("fn{d}", .{i});
        try rows.append(ctx.arena, ui.row(.{
            .key = c.key,
            .width_percent = 100,
            .align_items = .start,
            .children = &.{
                ui.styled(ui.text(try std.fmt.allocPrint(ctx.arena, "{d}.", .{note.number}), .{ .size = small.size, .color = theme.muted_foreground, .line_height = theme.line_height }), .{ .width = small.size * 2 }),
                ui.column(.{ .grow = 1, .shrink = 1, .basis = 0, .align_items = .stretch, .children = &.{try sequence(c, note.blocks, small.size * 0.5)} }),
            },
        }));
    }
    return ui.column(.{ .key = ctx.subKey("footnotes", .{}), .width_percent = 100, .gap = small.size * 0.4, .align_items = .stretch, .children = rows.items });
}
