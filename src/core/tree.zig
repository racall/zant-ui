const std = @import("std");
const node_mod = @import("node.zig");
const geometry = @import("geometry.zig");
const layout_mod = @import("layout.zig");
const tokens = @import("../theme/tokens.zig");
const Canvas = @import("../renderer/canvas.zig").Canvas;
const text = @import("../renderer/text.zig");
const button = @import("../components/button.zig");
const input = @import("../components/input.zig");
const toggle = @import("../components/toggle.zig");
const image_mod = @import("../components/image.zig");
const select_mod = @import("../components/select.zig");
const motion = @import("../motion/motion.zig");

pub const Node = node_mod.Node;
pub const NodeId = node_mod.NodeId;
pub const Data = node_mod.Data;
pub const FlexStyle = node_mod.FlexStyle;
pub const Visual = node_mod.Visual;
pub const Callback = node_mod.Callback;
pub const Theme = tokens.Theme;

pub const Tree = struct {
    allocator: std.mem.Allocator,
    /// Node slots. A `*Node` is never destroyed before `deinit`, so pointers
    /// stay valid; freed slots become tombstones (`alive == false`) and their
    /// ids are reused through `free_ids`.
    nodes: std.ArrayListUnmanaged(*Node) = .empty,
    free_ids: std.ArrayListUnmanaged(NodeId) = .empty,
    /// Nodes with a running transition (see `poke`); the runtime advances
    /// only these each frame instead of scanning the whole tree.
    animating: std.ArrayListUnmanaged(NodeId) = .empty,
    /// Frame timestamp (ms) sampled by the runtime.
    now_ms: i64 = 0,
    /// Layout transitions may start (off for the frame of a window resize).
    flip_enabled: bool = true,
    root: NodeId = 0,
    focused: ?NodeId = null,
    theme: Theme,
    /// Node by key (the most recently keyed node for a key); lookups fall
    /// back to a scan when the indexed node is not usable.
    key_index: std.StringHashMapUnmanaged(NodeId) = .empty,
    /// Fill behind selected text (`Node.text_sel`).
    selection_color: u32 = 0x401677ff,

    pub fn init(allocator: std.mem.Allocator, theme: Theme) !Tree {
        var tree = Tree{ .allocator = allocator, .theme = theme };
        const root = try tree.createNode(.{ .root = {} });
        tree.root = root;
        tree.get(root).style.direction = .column;
        return tree;
    }

    pub fn deinit(self: *Tree) void {
        for (self.nodes.items) |node| {
            if (node.alive) node.deinit(self.allocator);
            self.allocator.destroy(node);
        }
        self.nodes.deinit(self.allocator);
        self.key_index.deinit(self.allocator);
        self.free_ids.deinit(self.allocator);
        self.animating.deinit(self.allocator);
    }

    pub fn get(self: *Tree, id: NodeId) *Node {
        return self.nodes.items[id];
    }

    fn createNode(self: *Tree, data: Data) !NodeId {
        if (self.free_ids.pop()) |id| {
            self.nodes.items[id].* = .{ .id = id, .data = data };
            return id;
        }
        const id: NodeId = @intCast(self.nodes.items.len);
        const node = try self.allocator.create(Node);
        errdefer self.allocator.destroy(node);
        node.* = .{ .id = id, .data = data };
        try self.nodes.append(self.allocator, node);
        return id;
    }

    pub fn addContainer(self: *Tree, parent: ?NodeId, style: FlexStyle) !NodeId {
        const id = try self.createNode(.{ .container = .{} });
        self.get(id).style = style;
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addText(self: *Tree, parent: ?NodeId, content: []const u8, style: text.Style) !NodeId {
        const owned = try self.allocator.dupe(u8, content);
        errdefer self.allocator.free(owned);
        const id = try self.createNode(.{ .text = .{ .content = owned, .style = style } });
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addRichText(self: *Tree, parent: ?NodeId, spans: []const node_mod.RichSpan) !NodeId {
        const owned = try self.allocator.alloc(node_mod.RichSpan, spans.len);
        var copied: usize = 0;
        errdefer {
            for (owned[0..copied]) |span| self.freeRichSpan(span);
            self.allocator.free(owned);
        }
        for (spans, 0..) |span, i| {
            owned[i] = try self.ownRichSpan(span);
            copied += 1;
        }
        const embed_arena = try self.cloneEmbeds(owned);
        const id = try self.createNode(.{ .rich_text = .{ .spans = owned, .embed_arena = embed_arena } });
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    /// Deep-copies span embeds into a fresh arena (null when there are none).
    fn cloneEmbeds(self: *Tree, spans: []node_mod.RichSpan) !?*std.heap.ArenaAllocator {
        var any = false;
        for (spans) |span| if (span.embed != null) {
            any = true;
        };
        if (!any) return null;
        const arena = try self.allocator.create(std.heap.ArenaAllocator);
        arena.* = std.heap.ArenaAllocator.init(self.allocator);
        errdefer {
            arena.deinit();
            self.allocator.destroy(arena);
        }
        for (spans) |*span| {
            if (span.embed) |embed| span.embed = try embed.clone(arena.allocator());
        }
        return arena;
    }

    pub fn addVector(self: *Tree, parent: ?NodeId, drawing: *const node_mod.vector.Drawing) !NodeId {
        const arena = try self.allocator.create(std.heap.ArenaAllocator);
        arena.* = std.heap.ArenaAllocator.init(self.allocator);
        errdefer {
            arena.deinit();
            self.allocator.destroy(arena);
        }
        const copy = try drawing.clone(arena.allocator());
        const id = try self.createNode(.{ .vector = .{ .arena = arena, .drawing = copy, .hash = copy.hash } });
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn vectorSet(self: *Tree, id: NodeId, drawing: *const node_mod.vector.Drawing) !void {
        const node = self.get(id);
        const data = &node.data.vector;
        const hash = drawing.contentHash();
        if (hash == data.hash) return;
        const keep = try self.allocator.dupe(u8, data.copy);
        defer self.allocator.free(keep);
        _ = data.arena.reset(.retain_capacity);
        const copy = try drawing.clone(data.arena.allocator());
        data.drawing = copy;
        data.hash = hash;
        data.copy = try data.arena.allocator().dupe(u8, keep);
        self.markDirty(id, true, true);
    }

    fn ownRichSpan(self: *Tree, span: node_mod.RichSpan) !node_mod.RichSpan {
        var owned = span;
        owned.text = try self.allocator.dupe(u8, span.text);
        errdefer self.allocator.free(owned.text);
        owned.link = if (span.link.len > 0) try self.allocator.dupe(u8, span.link) else "";
        owned.copy = if (span.copy) |c| try self.allocator.dupe(u8, c) else null;
        return owned;
    }

    fn freeRichSpan(self: *Tree, span: node_mod.RichSpan) void {
        self.allocator.free(span.text);
        if (span.link.len > 0) self.allocator.free(span.link);
        if (span.copy) |c| self.allocator.free(c);
    }

    pub fn richTextSet(self: *Tree, id: NodeId, spans: []const node_mod.RichSpan) !void {
        const node = self.get(id);
        var data = &node.data.rich_text;
        data.invalidateLayout();
        for (data.spans) |span| self.freeRichSpan(span);
        self.allocator.free(data.spans);
        data.spans = &.{};
        const owned = try self.allocator.alloc(node_mod.RichSpan, spans.len);
        var copied: usize = 0;
        errdefer {
            for (owned[0..copied]) |span| self.freeRichSpan(span);
            self.allocator.free(owned);
        }
        for (spans, 0..) |span, i| {
            owned[i] = try self.ownRichSpan(span);
            copied += 1;
        }
        data.deinitEmbeds();
        data.embed_arena = try self.cloneEmbeds(owned);
        data.spans = owned;
        data.layout_dirty = true;
        self.markDirty(id, true, true);
    }

    pub fn addButton(
        self: *Tree,
        parent: ?NodeId,
        label: []const u8,
        button_type: node_mod.ButtonType,
        size: node_mod.ButtonSize,
        callback: ?Callback,
    ) !NodeId {
        const owned = try self.allocator.dupe(u8, label);
        errdefer self.allocator.free(owned);
        const id = try self.createNode(.{ .button = .{
            .label = owned,
            .button_type = button_type,
            .size = size,
            .font_size = button.fontSize(size),
            .on_click = callback,
        } });
        const node = self.get(id);
        node.focusable = true;
        node.visual.radius = self.theme.border_radius;
        node.style.height = button.controlHeight(size);
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addInput(
        self: *Tree,
        parent: ?NodeId,
        value: []const u8,
        placeholder: []const u8,
        on_change: ?Callback,
    ) !NodeId {
        var data = node_mod.InputData{ .placeholder = try self.allocator.dupe(u8, placeholder), .on_change = on_change };
        errdefer self.allocator.free(data.placeholder);
        try data.text.appendSlice(self.allocator, value);
        data.cursor = data.text.items.len;
        data.anchor = data.cursor;
        data.text_color = self.theme.color_text;
        input.rebuildDisplay(self.allocator, &data);
        const id = try self.createNode(.{ .input = data });
        const node = self.get(id);
        node.focusable = true;
        node.visual.radius = self.theme.border_radius;
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addToggle(self: *Tree, parent: ?NodeId, label: []const u8, kind: node_mod.ToggleKind, checked: bool) !NodeId {
        const owned = try self.allocator.dupe(u8, label);
        errdefer self.allocator.free(owned);
        const id = try self.createNode(.{ .toggle = .{
            .label = owned,
            .kind = kind,
            .checked = checked,
            .check = .at(if (checked) 1 else 0),
            .mark = .at(if (checked) 1 else 0),
            .check_t = if (checked) 1 else 0,
            .mark_t = if (checked) 1 else 0,
        } });
        const node = self.get(id);
        node.focusable = true;
        node.visual.radius = self.theme.border_radius_sm;
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addImage(self: *Tree, parent: ?NodeId, path: []const u8, fit: node_mod.ImageFit, radius: f32) !NodeId {
        const owned_path = try self.allocator.dupe(u8, path);
        errdefer self.allocator.free(owned_path);
        var data = node_mod.ImageData{ .path = owned_path, .fit = fit, .radius = radius };
        image_mod.load(&data);
        const id = try self.createNode(.{ .image = data });
        const node = self.get(id);
        node.visual.radius = radius;
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addSelect(self: *Tree, parent: ?NodeId, options: []const []const u8, selected: usize) !NodeId {
        var data = node_mod.SelectData{};
        for (options) |label| {
            const owned = try self.allocator.dupe(u8, label);
            try data.options.append(self.allocator, .{ .label = owned });
        }
        data.selected = @min(selected, if (options.len > 0) options.len - 1 else 0);
        const id = try self.createNode(.{ .select = data });
        const node = self.get(id);
        node.focusable = true;
        node.visual.radius = self.theme.border_radius;
        if (parent) |p| try self.addChild(p, id);
        return id;
    }

    pub fn addChild(self: *Tree, parent: NodeId, child: NodeId) !void {
        try self.get(parent).children.append(self.allocator, child);
        self.get(child).parent = parent;
        self.markDirty(parent, true, true);
    }

    pub fn setTooltip(self: *Tree, id: NodeId, value: ?[]const u8) void {
        const node = self.get(id);
        if (node.tooltip) |old| self.allocator.free(old);
        node.tooltip = if (value) |tip| self.allocator.dupe(u8, tip) catch null else null;
    }

    /// Window-space box of the laid-out node with `key` (null when absent).
    pub fn boundsOfKey(self: *const Tree, key: []const u8) ?geometry.Rect {
        if (self.key_index.get(key)) |id| {
            const node = self.nodes.items[id];
            if (node.alive and node.inFlow() and node.key != null and std.mem.eql(u8, node.key.?, key)) return node.bounds;
        }
        for (self.nodes.items) |node| {
            if (!node.alive or !node.inFlow()) continue;
            const k = node.key orelse continue;
            if (std.mem.eql(u8, k, key)) return node.bounds;
        }
        return null;
    }

    pub fn setKey(self: *Tree, id: NodeId, key: ?[]const u8) void {
        const node = self.get(id);
        self.unindexKey(id);
        if (node.key) |old| self.allocator.free(old);
        node.key = if (key) |value| self.allocator.dupe(u8, value) catch null else null;
        const k = node.key orelse return;
        const gop = self.key_index.getOrPut(self.allocator, k) catch return;
        // the map's key slice must be this node's (another node's may be freed)
        gop.key_ptr.* = k;
        gop.value_ptr.* = id;
    }

    /// Live, non-exiting node with `key` (index first, scan as fallback).
    pub fn idOfKey(self: *const Tree, key: []const u8) ?NodeId {
        if (self.key_index.get(key)) |id| {
            const node = self.nodes.items[id];
            if (node.alive and !node.exiting and node.key != null and std.mem.eql(u8, node.key.?, key)) return id;
        }
        for (self.nodes.items) |node| {
            if (!node.alive or node.exiting) continue;
            if (node.key) |k| if (std.mem.eql(u8, k, key)) return node.id;
        }
        return null;
    }

    fn unindexKey(self: *Tree, id: NodeId) void {
        const k = self.nodes.items[id].key orelse return;
        if (self.key_index.getEntry(k)) |e| {
            if (e.value_ptr.* == id) self.key_index.removeByPtr(e.key_ptr);
        }
    }

    /// Releases `id` and its whole subtree: owned memory is freed and the
    /// slots are recycled. The caller must already have removed `id` from its
    /// parent's `children` and cleared any runtime references to the subtree.
    pub fn freeSubtree(self: *Tree, id: NodeId) void {
        const node = self.nodes.items[id];
        if (!node.alive) return;
        for (node.children.items) |child| self.freeSubtree(child);
        if (self.focused == id) self.focused = null;
        if (node.anim_queued) {
            if (std.mem.indexOfScalar(NodeId, self.animating.items, id)) |i| _ = self.animating.swapRemove(i);
        }
        self.unindexKey(id);
        node.deinit(self.allocator);
        node.* = .{ .id = id, .data = .{ .container = .{} }, .visible = false, .alive = false };
        self.free_ids.append(self.allocator, id) catch {};
    }

    /// Repaints `id` and schedules it for animation (its state changed, so a
    /// transition may have to start). Cheap when nothing ends up animating.
    pub fn poke(self: *Tree, id: NodeId) void {
        const node = self.get(id);
        if (!node.alive) return;
        self.markDirty(id, false, true);
        if (node.anim_queued) return;
        self.animating.append(self.allocator, id) catch return;
        node.anim_queued = true;
    }

    /// Number of live (non-recycled) nodes.
    pub fn liveCount(self: *const Tree) usize {
        return self.nodes.items.len - self.free_ids.items.len;
    }

    pub fn markDirty(self: *Tree, id: NodeId, needs_layout: bool, needs_paint: bool) void {
        var current: ?NodeId = id;
        while (current) |cid| {
            const node = self.get(cid);
            node.mark(needs_layout, needs_paint);
            current = node.parent;
        }
    }

    pub fn needsLayout(self: *Tree) bool {
        return self.get(self.root).dirty.layout;
    }

    pub fn needsPaint(self: *Tree) bool {
        return self.get(self.root).dirty.paint;
    }

    pub fn markLayoutDirty(self: *Tree) void {
        self.markDirty(self.root, true, true);
    }

    pub fn markPaintDirty(self: *Tree) void {
        self.markDirty(self.root, false, true);
    }

    pub fn clearPaint(self: *Tree) void {
        for (self.nodes.items) |node| {
            node.dirty.paint = false;
        }
    }

    pub fn layout(self: *Tree, viewport: geometry.Size) void {
        // FLIP: remember where layout-transition nodes were painted, lay out,
        // then animate from the old box to the new one
        for (self.nodes.items) |node| {
            if (!node.alive or node.motion.layout_transition == null) continue;
            const anim = node.anim orelse continue;
            const rel = self.relativeBounds(node);
            for (0..4) |k| anim.flip_prev[k] = rel[k] + anim.flip_delta[k];
        }
        layout_mod.layoutTree(self.nodes.items, self.root, viewport, self.theme);
        for (self.nodes.items) |node| {
            if (!node.alive) continue;
            const tr = node.motion.layout_transition orelse continue;
            const anim = self.ensureAnim(node.id) orelse continue;
            const rel = self.relativeBounds(node);
            if (anim.flip_ready and motion.enabled and self.flip_enabled) {
                var moved = false;
                var from: [4]f32 = undefined;
                for (0..4) |k| {
                    from[k] = anim.flip_prev[k] - rel[k];
                    if (@abs(from[k]) > 0.5) moved = true;
                }
                if (moved) {
                    anim.flip_from = from;
                    anim.flip_delta = from;
                    anim.flip = motion.Animated.start(1, 0, self.now_ms, tr);
                    anim.flip_active = true;
                    self.poke(node.id);
                }
            } else if (!self.flip_enabled) {
                anim.flip = .at(0);
                anim.flip_active = false;
                anim.flip_delta = @splat(0);
            }
            anim.flip_ready = true;
        }
    }

    /// Bounds relative to the parent (x, y, w, h): scrolling or moving an
    /// ancestor does not count as a layout change.
    fn relativeBounds(self: *Tree, node: *const Node) [4]f32 {
        var x = node.bounds.x;
        var y = node.bounds.y;
        if (node.parent) |p| {
            const parent = self.get(p);
            x -= parent.bounds.x;
            y -= parent.bounds.y;
        }
        return .{ x, y, node.bounds.width, node.bounds.height };
    }

    /// Animation state of `id`, allocated on first use.
    pub fn ensureAnim(self: *Tree, id: NodeId) ?*node_mod.NodeAnim {
        const node = self.get(id);
        if (node.anim) |anim| return anim;
        const anim = self.allocator.create(node_mod.NodeAnim) catch return null;
        anim.* = .{};
        node.anim = anim;
        return anim;
    }

    pub fn paint(self: *Tree, canvas: Canvas) void {
        self.paintNode(self.get(self.root), canvas);
    }

    fn paintNode(self: *Tree, node: *Node, canvas: Canvas) void {
        if (!node.visible) return;
        const anim = node.anim orelse return self.paintNodeContent(node, canvas, node.bounds);
        // FLIP offset applies to the node's own box (leaf containers)
        var rect = node.bounds;
        if (anim.flip_active) {
            rect.x += anim.flip_delta[0];
            rect.y += anim.flip_delta[1];
            rect.width = @max(0, rect.width + anim.flip_delta[2]);
            rect.height = @max(0, rect.height + anim.flip_delta[3]);
        }
        const fx = anim.fx;
        if (fx.isIdentity()) return self.paintNodeContent(node, canvas, rect);
        if (fx.opacity <= 0.003) return;
        const b = node.bounds;
        const origin = motion.originPoint(motion.presenceOrigin(node.motion.presence, node.motion.origin), b.x, b.y, b.width, b.height);
        canvas.save();
        canvas.translate(origin[0] + fx.dx, origin[1] + fx.dy);
        canvas.scale(fx.scale_x, fx.scale_y);
        canvas.translate(-origin[0], -origin[1]);
        // an offscreen layer only while fading (opacity 1 draws directly)
        const layer = fx.opacity < 0.997;
        if (layer) {
            const pad = @max(8, 3 * node.visual.shadow_blur + @abs(node.visual.shadow_offset_y));
            canvas.saveLayerAlpha(geometry.Rect.init(b.x - pad, b.y - pad, b.width + pad * 2, b.height + pad * 2), fx.opacity);
        }
        self.paintNodeContent(node, canvas, rect);
        if (layer) canvas.restore();
        canvas.restore();
    }

    fn paintNodeContent(self: *Tree, node: *Node, canvas: Canvas, rect: geometry.Rect) void {
        if (node.visual.shadow_color) |shadow| {
            var shadow_rect = rect;
            shadow_rect.y += node.visual.shadow_offset_y;
            canvas.shadowRoundRect(shadow_rect, node.visual.radius, shadow, node.visual.shadow_blur);
        }
        // buttons, inputs and selects paint their own (state-dependent)
        // background; filling it here too would double translucent fills
        const self_filled = switch (node.data) {
            .button, .input, .select => true,
            else => false,
        };
        var hover_fill: ?u32 = null;
        var hover_border: ?u32 = null;
        switch (node.data) {
            .container => |*d| if (d.hover_visual) |hv| if (d.hover_t > 0.001) {
                if (hv.background) |bg| hover_fill = motion.mixColor(node.visual.background orelse (bg & 0x00ffffff), bg, d.hover_t);
                if (hv.border_color) |bc| hover_border = motion.mixColor(node.visual.border_color orelse (bc & 0x00ffffff), bc, d.hover_t);
            },
            else => {},
        }
        if ((node.visual.background != null or hover_fill != null) and !self_filled) {
            const color = hover_fill orelse node.visual.background.?;
            if (node.visual.radius > 0) {
                canvas.fillRoundRect(rect, node.visual.radius, color);
            } else {
                canvas.fillRect(rect, color);
            }
        }
        switch (node.data) {
            .root, .container => {},
            .text => |*data| {
                if (data.ensureLayout(data.layoutWidth(rect.width))) |value| {
                    if (node.text_sel) |r| paintSelection(canvas, value.handle, r, rect.x, rect.y, self.selection_color);
                    value.paint(canvas, rect.x, rect.y);
                } else |_| {}
            },
            .button => button.paint(node, canvas, self.theme),
            .input => input.paint(node, canvas, self.theme),
            .toggle => toggle.paint(node, canvas, self.theme),
            .image => image_mod.paint(node, canvas, self.theme),
            .select => select_mod.paint(node, canvas, self.theme),
            .rich_text => |*data| {
                if (data.ensureLayout(rect.width)) |rich_layout| {
                    if (node.text_sel) |r| paintSelection(canvas, rich_layout.handle, r, rect.x, rect.y, self.selection_color);
                    data.paintAt(canvas.ptr, rich_layout, rect.x, rect.y);
                } else |_| {}
            },
            .vector => |*data| {
                if (node.text_sel != null) canvas.fillRect(rect, self.selection_color);
                node_mod.vector.paint(canvas.ptr, data.drawing, rect.x, rect.y + data.drawing.ascent);
            },
        }

        const should_clip = node.visual.clip or std.meta.activeTag(node.data) == .input;
        if (should_clip) {
            canvas.save();
            canvas.clipRect(rect);
        }
        if (node.style.scroll) {
            // scroll containers: skip children far outside the viewport
            const top = node.bounds.y - 64;
            const bottom = node.bounds.y + node.bounds.height + 64;
            for (node.children.items) |id| {
                const child = self.get(id);
                if (child.bounds.y > bottom or child.bounds.y + child.bounds.height < top) continue;
                self.paintNode(child, canvas);
            }
        } else for (node.children.items) |id| {
            self.paintNode(self.get(id), canvas);
        }
        // borders frame the content: drawn after children so filled children
        // (headers, cells) never cover them
        if (hover_border orelse node.visual.border_color) |border| {
            if (node.visual.border_width > 0) {
                canvas.strokeRoundRect(
                    geometry.Rect.init(rect.x + 0.5, rect.y + 0.5, @max(0, rect.width - 1), @max(0, rect.height - 1)),
                    node.visual.radius,
                    border,
                    node.visual.border_width,
                );
            }
        }
        if (node.style.scroll) paintScrollIndicator(node, canvas, tokens.withAlpha(self.theme.color_text, 0x40));
        if (should_clip) canvas.restore();
    }

    fn paintSelection(canvas: Canvas, handle: ?*anyopaque, range: [2]u32, x: f32, y: f32, color: u32) void {
        if (range[0] >= range[1]) return;
        var rects: [64]text.NativeRect = undefined;
        const n = zant_text_layout_range_rects(handle, range[0], range[1], &rects, rects.len);
        for (rects[0..@intCast(@max(n, 0))]) |r| {
            canvas.fillRect(geometry.Rect.init(x + r.x, y + r.y, r.width, r.height), color);
        }
    }

    extern fn zant_text_layout_range_rects(layout: ?*anyopaque, start: usize, end: usize, rects: [*]text.NativeRect, max_rects: c_int) c_int;

    fn paintScrollIndicator(node: *Node, canvas: Canvas, color: u32) void {
        const data = switch (node.data) {
            .container => |value| value,
            else => return,
        };
        const view = node.bounds.height;
        if (view <= 0 or data.content_height <= view + 0.5) return;
        const track = view - 8;
        const thumb = @max(28, track * view / data.content_height);
        const progress = data.scroll_offset / data.maxScroll(view);
        const y = node.bounds.y + 4 + (track - thumb) * progress;
        const x = node.bounds.right() - 7;
        canvas.fillRoundRect(geometry.Rect.init(x, y, 3.5, thumb), 1.75, color);
    }

    /// Scrolls the innermost scroll container under `(x, y)` by `dy` points
    /// (positive dy moves content down, like AppKit's scrollingDeltaY).
    /// Returns true when an offset changed.
    pub fn scrollAt(self: *Tree, x: f32, y: f32, dy: f32) bool {
        const id = self.scrollContainerAt(self.get(self.root), x, y) orelse return false;
        // the user's own scrolling wins over a running scrollIntoView
        switch (self.get(id).data) {
            .container => |*data| data.scroll_animating = false,
            else => {},
        }
        return self.scrollBy(id, -dy);
    }

    /// Adds `delta` to a scroll container's offset (clamped). Returns true on change.
    pub fn scrollBy(self: *Tree, id: NodeId, delta: f32) bool {
        const node = self.get(id);
        const data = switch (node.data) {
            .container => |*value| value,
            else => return false,
        };
        const max = data.maxScroll(node.bounds.height);
        const next = @max(0, @min(max, data.scroll_offset + delta));
        if (next == data.scroll_offset) return false;
        const shift = data.scroll_offset - next;
        data.scroll_offset = next;
        for (node.children.items) |child| layout_mod.translateSubtreeY(self.nodes.items, self.get(child), shift);
        self.markDirty(id, false, true);
        return true;
    }

    fn scrollContainerAt(self: *Tree, node: *Node, x: f32, y: f32) ?NodeId {
        if (!node.inFlow() or !node.bounds.contains(x, y)) return null;
        var i = node.children.items.len;
        while (i > 0) {
            i -= 1;
            if (self.scrollContainerAt(self.get(node.children.items[i]), x, y)) |hit| return hit;
        }
        if (node.style.scroll and std.meta.activeTag(node.data) == .container) {
            if (node.data.container.content_height > node.bounds.height + 0.5) return node.id;
        }
        return null;
    }

    /// Innermost container with a hover look under `(x, y)`.
    pub fn hoverBoxAt(self: *Tree, x: f32, y: f32) ?NodeId {
        return self.hoverBoxNode(self.get(self.root), x, y);
    }

    fn hoverBoxNode(self: *Tree, node: *Node, x: f32, y: f32) ?NodeId {
        if (!node.inFlow() or !node.bounds.contains(x, y)) return null;
        var i = node.children.items.len;
        while (i > 0) {
            i -= 1;
            if (self.hoverBoxNode(self.get(node.children.items[i]), x, y)) |hit| return hit;
        }
        return switch (node.data) {
            .container => |*d| if (d.hover_visual != null) node.id else null,
            else => null,
        };
    }

    /// Innermost node under `(x, y)` that takes wheel input: a scroll
    /// container with overflow or a pointer area with `capture_scroll`.
    pub fn wheelTargetAt(self: *Tree, x: f32, y: f32) ?NodeId {
        return self.wheelTargetNode(self.get(self.root), x, y);
    }

    fn wheelTargetNode(self: *Tree, node: *Node, x: f32, y: f32) ?NodeId {
        if (!node.inFlow() or !node.bounds.contains(x, y)) return null;
        var i = node.children.items.len;
        while (i > 0) {
            i -= 1;
            if (self.wheelTargetNode(self.get(node.children.items[i]), x, y)) |hit| return hit;
        }
        switch (node.data) {
            .container => |*data| {
                if (data.pointer) |area| if (area.capture_scroll) return node.id;
                if (node.style.scroll and data.content_height > node.bounds.height + 0.5) return node.id;
            },
            else => {},
        }
        return null;
    }

    pub fn hitTest(self: *Tree, x: f32, y: f32) ?NodeId {
        return self.hitTestNode(self.get(self.root), x, y);
    }

    fn hitTestNode(self: *Tree, node: *Node, x: f32, y: f32) ?NodeId {
        if (!node.inFlow()) return null;
        if (!node.bounds.contains(x, y)) return null;
        var i = node.children.items.len;
        while (i > 0) {
            i -= 1;
            if (self.hitTestNode(self.get(node.children.items[i]), x, y)) |hit| return hit;
        }
        if (node.isInteractive() or node.visual.block_pointer) return node.id;
        return null;
    }

    pub fn focusedNode(self: *Tree) ?*Node {
        if (self.focused) |id| {
            const node = self.get(id);
            if (node.enabled and node.inFlow() and node.focusable) return node;
        }
        return null;
    }

    pub fn setFocus(self: *Tree, id: ?NodeId) void {
        for (self.nodes.items) |node| {
            if (!node.alive) continue;
            switch (node.data) {
                .button => |*data| data.focused = false,
                .input => |*data| {
                    if (data.focused) self.poke(node.id);
                    data.focused = false;
                    data.blink_on = true;
                    data.next_blink_ms = 0;
                },
                .toggle => |*data| data.focused = false,
                .select => |*data| {
                    if (data.focused or data.open) self.poke(node.id);
                    data.focused = false;
                    data.open = false;
                    data.hovered_option = null;
                },
                else => {},
            }
        }
        self.focused = null;
        if (id) |target_id| {
            const target = self.get(target_id);
            if (!target.enabled or !target.inFlow() or !target.focusable) return;
            self.focused = target_id;
            switch (target.data) {
                .button => |*data| data.focused = true,
                .input => |*data| {
                    data.focused = true;
                    data.blink_on = true;
                    data.next_blink_ms = 0;
                },
                .toggle => |*data| data.focused = true,
                .select => |*data| data.focused = true,
                else => {},
            }
            self.poke(target_id);
        }
    }

    fn collectFocusables(self: *Tree, node: *Node, out: *std.ArrayListUnmanaged(NodeId)) !void {
        if (!node.inFlow()) return;
        if (node.focusable and node.enabled) {
            try out.append(self.allocator, node.id);
        }
        for (node.children.items) |id| {
            try self.collectFocusables(self.get(id), out);
        }
    }

    /// The topmost (last painted) focus scope shown, if any.
    fn activeFocusScope(self: *Tree, node: *Node) ?NodeId {
        if (!node.inFlow()) return null;
        var i = node.children.items.len;
        while (i > 0) {
            i -= 1;
            if (self.activeFocusScope(self.get(node.children.items[i]))) |hit| return hit;
        }
        return if (node.visual.focus_scope) node.id else null;
    }

    pub fn tabFocus(self: *Tree, backwards: bool) void {
        var focusables: std.ArrayListUnmanaged(NodeId) = .empty;
        defer focusables.deinit(self.allocator);
        // inside an open dialog Tab cycles through the dialog only
        const scope = self.activeFocusScope(self.get(self.root)) orelse self.root;
        self.collectFocusables(self.get(scope), &focusables) catch return;
        if (focusables.items.len == 0) return;
        var index: usize = 0;
        var found = false;
        if (self.focused) |focused_id| {
            for (focusables.items, 0..) |id, i| {
                if (id == focused_id) {
                    index = i;
                    found = true;
                    break;
                }
            }
        }
        if (!found) {
            // focus outside the scope (or none): enter at its first / last
            self.setFocus(focusables.items[if (backwards) focusables.items.len - 1 else 0]);
            return;
        }
        if (self.focused != null) {
            if (backwards) {
                index = if (index == 0) focusables.items.len - 1 else index - 1;
            } else {
                index = (index + 1) % focusables.items.len;
            }
        } else if (backwards) {
            index = focusables.items.len - 1;
        }
        self.setFocus(focusables.items[index]);
    }

    pub fn invoke(self: *Tree, callback: ?Callback, id: NodeId) void {
        _ = self;
        if (callback) |cb| cb.func(cb.ctx, id);
    }

    pub fn setText(self: *Tree, id: NodeId, value: []const u8) !void {
        const node = self.get(id);
        const data = &node.data.text;
        const owned = try self.allocator.dupe(u8, value);
        self.allocator.free(data.content);
        data.content = owned;
        data.invalidate();
        self.markDirty(id, true, true);
    }

    /// Updates a text label whose outer bounds are fixed by its parent. This is
    /// used by the live input read-out so typing does not trigger a tree layout.
    pub fn setTextNoLayout(self: *Tree, id: NodeId, value: []const u8) !void {
        const node = self.get(id);
        const data = &node.data.text;
        const owned = try self.allocator.dupe(u8, value);
        self.allocator.free(data.content);
        data.content = owned;
        data.invalidate();
        self.markDirty(id, false, true);
    }

    pub fn setNodeEnabled(self: *Tree, id: NodeId, enabled: bool) void {
        const node = self.get(id);
        node.enabled = enabled;
        switch (node.data) {
            .button => |*data| {
                if (!enabled) {
                    data.hovered = false;
                    data.pressed = false;
                    data.focused = false;
                    data.hover_t = 0;
                    data.press_t = 0;
                }
                data.layout_dirty = true;
            },
            .input => |*data| {
                if (!enabled) {
                    data.focused = false;
                    data.hovered = false;
                }
            },
            else => {},
        }
        self.markDirty(id, true, true);
    }

    pub fn inputInsert(self: *Tree, id: NodeId, value: []const u8) bool {
        const node = self.get(id);
        const changed = input.insertText(self.allocator, &node.data.input, value);
        if (changed) self.inputChanged(id);
        return changed;
    }

    pub fn inputDeleteBackward(self: *Tree, id: NodeId) bool {
        const node = self.get(id);
        const changed = input.deleteBackward(self.allocator, &node.data.input);
        if (changed) self.inputChanged(id);
        return changed;
    }

    pub fn inputDeleteForward(self: *Tree, id: NodeId) bool {
        const node = self.get(id);
        const changed = input.deleteForward(self.allocator, &node.data.input);
        if (changed) self.inputChanged(id);
        return changed;
    }

    pub fn inputMove(self: *Tree, id: NodeId, target: input.Move, extend: bool) void {
        const node = self.get(id);
        const data = &node.data.input;
        if (target != .up and target != .down) data.preferred_x = null;
        switch (target) {
            .left => input.moveLeft(data, extend),
            .right => input.moveRight(data, extend),
            .home, .doc_start => input.moveHome(data, extend),
            .end, .doc_end => input.moveEnd(data, extend),
            .word_left => input.moveWordLeft(data, extend),
            .word_right => input.moveWordRight(data, extend),
            .up => input.moveVertical(data, false, extend),
            .down => input.moveVertical(data, true, extend),
            .line_start => input.moveLineEdge(data, false, extend),
            .line_end => input.moveLineEdge(data, true, extend),
        }
        self.markDirty(id, false, true);
    }

    pub fn inputSelectAll(self: *Tree, id: NodeId) void {
        input.selectAll(&self.get(id).data.input);
        self.markDirty(id, false, true);
    }

    pub fn inputChanged(self: *Tree, id: NodeId) void {
        const node = self.get(id);
        // Inputs are single-line controls with a fixed outer size; editing changes
        // paint only and must not force a whole-tree layout on every keystroke.
        self.markDirty(id, false, true);
        self.invoke(node.data.input.on_change, id);
        if (node.data.input.text_action) |action| {
            action.func(action.ctx, node.data.input.text.items);
        }
    }

    pub fn inputComposition(self: *Tree, id: NodeId, marked: []const u8, start: usize, end: usize) void {
        const node = self.get(id);
        input.beginComposition(self.allocator, &node.data.input, marked, start, end);
        self.markDirty(id, false, true);
    }

    pub fn inputCancelComposition(self: *Tree, id: NodeId) void {
        const node = self.get(id);
        input.cancelComposition(self.allocator, &node.data.input);
        self.markDirty(id, false, true);
    }

    pub fn inputSetValue(self: *Tree, id: NodeId, value: []const u8) void {
        const node = self.get(id);
        if (std.meta.activeTag(node.data) != .input) return;
        const data = &node.data.input;
        data.text.clearRetainingCapacity();
        data.text.appendSlice(self.allocator, value) catch return;
        data.cursor = @min(data.cursor, data.text.items.len);
        data.anchor = @min(data.anchor, data.text.items.len);
        data.has_composition = false;
        data.composition.clearRetainingCapacity();
        input.rebuildDisplay(self.allocator, data);
        self.markDirty(id, false, true);
    }

    pub fn inputSetPlaceholder(self: *Tree, id: NodeId, value: []const u8) void {
        const node = self.get(id);
        if (std.meta.activeTag(node.data) != .input) return;
        const data = &node.data.input;
        const owned = self.allocator.dupe(u8, value) catch return;
        self.allocator.free(data.placeholder);
        data.placeholder = owned;
        if (data.placeholder_layout) |*lay| lay.deinit();
        data.placeholder_layout = null;
        self.markDirty(id, false, true);
    }

    pub fn inputCaretRect(self: *Tree, id: NodeId) ?geometry.Rect {
        return input.caretRect(&self.get(id).data.input, self.get(id));
    }

    pub fn toggleSet(self: *Tree, id: NodeId, checked: bool) void {
        const node = self.get(id);
        if (std.meta.activeTag(node.data) != .toggle) return;
        node.data.toggle.checked = checked;
        self.poke(id);
        if (node.data.toggle.index_action) |action| {
            action.func(action.ctx, node.data.toggle.index);
        } else if (node.data.toggle.action) |action| {
            action.func(action.ctx, checked);
        }
    }

    pub fn toggleFlip(self: *Tree, id: NodeId) void {
        const node = self.get(id);
        if (std.meta.activeTag(node.data) != .toggle) return;
        self.toggleSet(id, !node.data.toggle.checked);
    }
};
