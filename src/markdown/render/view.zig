//! The Streamdown component: renders Markdown with block-level memoisation.
const std = @import("std");
const ui = @import("../../ui.zig");
const ast = @import("../ast.zig");
const cache_mod = @import("../cache.zig");
const Options = @import("options.zig").Options;
const Ctx = @import("context.zig").Ctx;
const blocks = @import("blocks.zig");

/// Renders a parsed document without caching (everything in the frame).
pub fn document(doc: *const ast.Document, options: Options) ui.Node {
    const arena = ui.frameAllocator();
    const opts = arena.create(Options) catch @panic("Zant UI: out of memory");
    opts.* = options;
    return buildDocument(arena, doc, opts, options.caret != .none and options.is_animating, options.key_prefix) catch
        ui.text("Markdown render failed", .{ .color = options.theme.muted_foreground });
}

fn buildDocument(arena: std.mem.Allocator, doc: *const ast.Document, options: *const Options, caret: bool, key: []const u8) !ui.Node {
    var ctx = try Ctx.init(arena, options);
    ctx.caret = caret;
    ctx.key = key;
    const gap = options.theme.block_gap * options.font_size;
    const body = try blocks.sequence(ctx, doc.blocks, gap);
    if (doc.footnotes.len == 0) return body;
    return ui.column(.{ .key = key, .width_percent = 100, .gap = gap, .align_items = .stretch, .children = &.{ body, try blocks.footnotes(ctx, doc.footnotes) } });
}

pub const Props = struct {
    render: Options = .{},
    mode: cache_mod.Mode = .static,
    parse_incomplete: bool = true,
    parse: @import("../parser/mod.zig").Options = .{},
    remend: @import("../remend/mod.zig").Options = .{},
};

/// Per-block render slot kept in the cache entry (stable for `ui.memo`).
const Slot = struct {
    entry: *cache_mod.Entry,
    options: Options,
    caret: bool,
    key: []const u8,

    fn build(self: *Slot) ui.Node {
        // the previous build of this block is no longer referenced: the
        // retained tree deep-copies everything it keeps
        _ = self.entry.view_arena.reset(.retain_capacity);
        return buildDocument(self.entry.view_arena.allocator(), &self.entry.doc, &self.options, self.caret, self.key) catch
            ui.column(.{ .key = self.key });
    }
};

/// UI key of a block (stable while its cache entry lives).
pub fn blockKey(buf: []u8, key_prefix: []const u8, entry: *const cache_mod.Entry) ?[]const u8 {
    return std.fmt.bufPrint(buf, "{s}-e{d}", .{ key_prefix, entry.id }) catch null;
}

/// Key of the rendered block containing byte `offset` of the source, for
/// `ui.scrollIntoView` (e.g. jumping to a heading in the preview). The
/// cache must be the one passed to `streamdown` with the same `key_prefix`.
/// Key of the document's `n`-th top-level heading (0-based, in source
/// order) as rendered by `streamdown`, for `ui.scrollIntoView` — e.g. an
/// outline jump. Works even when the document is one block (footnotes).
pub fn headingKey(cache: *const cache_mod.Cache, key_prefix: []const u8, n: usize, buf: []u8) ?[]const u8 {
    var seen: usize = 0;
    for (cache.entries.items) |entry| {
        for (entry.doc.blocks, 0..) |block, i| {
            if (block != .heading) continue;
            if (seen == n) return std.fmt.bufPrint(buf, "{s}-e{d}-{d}", .{ key_prefix, entry.id, i }) catch null;
            seen += 1;
        }
    }
    return null;
}

pub fn blockKeyAtOffset(cache: *const cache_mod.Cache, key_prefix: []const u8, offset: usize, buf: []u8) ?[]const u8 {
    const index = cache.entryAtOffset(offset) orelse return null;
    return blockKey(buf, key_prefix, cache.entries.items[index]);
}

/// Streamdown: heals (streaming), splits, parses changed blocks only and
/// rebuilds the UI of changed blocks only. `cache` is owned by the caller.
pub fn streamdown(cache: *cache_mod.Cache, source: []const u8, props: Props) ui.Node {
    const entries = cache.update(source, .{
        .mode = props.mode,
        .parse_incomplete = props.parse_incomplete,
        .parse = props.parse,
        .remend = props.remend,
    }) catch return ui.text("Markdown parse failed", .{ .color = props.render.theme.muted_foreground });
    const opts = props.render;
    const options_hash = opts.hash();
    const animating = opts.is_animating and opts.caret != .none and props.mode == .streaming;
    const arena = ui.frameAllocator();
    const children = arena.alloc(ui.Node, entries.len) catch @panic("Zant UI: out of memory");
    for (entries, 0..) |entry, i| {
        const caret = animating and i + 1 == entries.len;
        const slot: *Slot = if (entry.view_slot) |raw| @ptrCast(@alignCast(raw)) else blk: {
            const s = entry.arena().create(Slot) catch @panic("Zant UI: out of memory");
            entry.view_slot = s;
            break :blk s;
        };
        const key = std.fmt.allocPrint(arena, "{s}-e{d}", .{ opts.key_prefix, entry.id }) catch opts.key_prefix;
        slot.* = .{ .entry = entry, .options = opts, .caret = caret, .key = key };
        var h = std.hash.Wyhash.init(entry.hash);
        h.update(std.mem.asBytes(&options_hash));
        h.update(&.{@intFromBool(caret)});
        children[i] = ui.memo(.{ .key = key, .hash = h.final() }, slot, Slot.build);
    }
    if (entries.len == 0 and animating) {
        const empty = arena.create(ast.Document) catch @panic("Zant UI: out of memory");
        empty.* = .{ .arena = std.heap.ArenaAllocator.init(std.heap.page_allocator), .blocks = &.{} };
        const only = arena.alloc(ui.Node, 1) catch @panic("Zant UI: out of memory");
        only[0] = document(empty, opts);
        return ui.column(.{ .key = opts.key_prefix, .width_percent = 100, .align_items = .stretch, .children = only });
    }
    return ui.column(.{
        .key = opts.key_prefix,
        .width_percent = 100,
        .gap = opts.theme.block_gap * opts.font_size,
        .align_items = .stretch,
        .children = children,
    });
}
