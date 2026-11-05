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

pub const Node = node_mod.Node;
pub const NodeId = node_mod.NodeId;
pub const Data = node_mod.Data;
pub const FlexStyle = node_mod.FlexStyle;
pub const Visual = node_mod.Visual;
pub const Callback = node_mod.Callback;
pub const Theme = tokens.Theme;

pub const Tree = struct {
    allocator: std.mem.Allocator,
    nodes: std.ArrayListUnmanaged(*Node) = .empty,
    root: NodeId = 0,
    focused: ?NodeId = null,
    theme: Theme,

    pub fn init(allocator: std.mem.Allocator, theme: Theme) !Tree {
        var tree = Tree{ .allocator = allocator, .theme = theme };
        const root = try tree.createNode(.{ .root = {} });
        tree.root = root;
        tree.get(root).style.direction = .column;
        return tree;
    }

    pub fn deinit(self: *Tree) void {
        for (self.nodes.items) |node| {
            node.deinit(self.allocator);
            self.allocator.destroy(node);
        }
        self.nodes.deinit(self.allocator);
    }

    pub fn get(self: *Tree, id: NodeId) *Node {
        return self.nodes.items[id];
    }

    fn createNode(self: *Tree, data: Data) !NodeId {
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
            .check_t = if (checked) 1 else 0,
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

    pub fn setKey(self: *Tree, id: NodeId, key: ?[]const u8) void {
        const node = self.get(id);
        if (node.key) |old| self.allocator.free(old);
        node.key = if (key) |value| self.allocator.dupe(u8, value) catch null else null;
    }

    pub fn detach(self: *Tree, id: NodeId) void {
        const node = self.get(id);
        node.visible = false;
        node.parent = null;
        if (self.focused == id) self.focused = null;
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
        layout_mod.layoutTree(self.nodes.items, self.root, viewport, self.theme);
    }

    pub fn paint(self: *Tree, canvas: Canvas) void {
        self.paintNode(self.get(self.root), canvas);
    }

    fn paintNode(self: *Tree, node: *Node, canvas: Canvas) void {
        if (!node.visible) return;
        const rect = node.bounds;
        if (node.visual.background) |color| {
            if (node.visual.radius > 0) {
                canvas.fillRoundRect(rect, node.visual.radius, color);
            } else {
                canvas.fillRect(rect, color);
            }
        }
        switch (node.data) {
            .root, .container => {},
            .text => |*data| {
                const width = if (data.style.wrap) rect.width else 1.0e6;
                if (data.ensureLayout(width)) |value| {
                    value.paint(canvas, rect.x, rect.y);
                } else |_| {}
            },
            .button => button.paint(node, canvas, self.theme),
            .input => input.paint(node, canvas, self.theme),
            .toggle => toggle.paint(node, canvas, self.theme),
            .image => image_mod.paint(node, canvas),
            .select => select_mod.paint(node, canvas, self.theme),
        }

        const should_clip = node.visual.clip or std.meta.activeTag(node.data) == .input;
        if (should_clip) {
            canvas.save();
            canvas.clipRect(rect);
        }
        if (node.visual.border_color) |border| {
            if (node.visual.border_width > 0) {
                canvas.strokeRoundRect(
                    geometry.Rect.init(rect.x + 0.5, rect.y + 0.5, @max(0, rect.width - 1), @max(0, rect.height - 1)),
                    node.visual.radius,
                    border,
                    node.visual.border_width,
                );
            }
        }
        for (node.children.items) |id| {
            self.paintNode(self.get(id), canvas);
        }
        if (should_clip) canvas.restore();
    }

    pub fn hitTest(self: *Tree, x: f32, y: f32) ?NodeId {
        return self.hitTestNode(self.get(self.root), x, y);
    }

    fn hitTestNode(self: *Tree, node: *Node, x: f32, y: f32) ?NodeId {
        if (!node.visible) return null;
        if (!node.bounds.contains(x, y)) return null;
        var i = node.children.items.len;
        while (i > 0) {
            i -= 1;
            if (self.hitTestNode(self.get(node.children.items[i]), x, y)) |hit| return hit;
        }
        if (node.isInteractive()) return node.id;
        return null;
    }

    pub fn focusedNode(self: *Tree) ?*Node {
        if (self.focused) |id| {
            const node = self.get(id);
            if (node.enabled and node.visible and node.focusable) return node;
        }
        return null;
    }

    pub fn setFocus(self: *Tree, id: ?NodeId) void {
        for (self.nodes.items) |node| {
            switch (node.data) {
                .button => |*data| data.focused = false,
                .input => |*data| {
                    data.focused = false;
                    data.blink_on = true;
                    data.next_blink_ms = 0;
                },
                .toggle => |*data| data.focused = false,
                .select => |*data| {
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
            if (!target.enabled or !target.visible or !target.focusable) return;
            self.focused = target_id;
            switch (target.data) {
                .button => |*data| {
                    data.focused = true;
                    data.hover_t = @max(data.hover_t, 0.0);
                },
                .input => |*data| {
                    data.focused = true;
                    data.blink_on = true;
                    data.next_blink_ms = 0;
                },
                .toggle => |*data| data.focused = true,
                .select => |*data| data.focused = true,
                else => {},
            }
            self.markDirty(target_id, false, true);
        }
    }

    fn collectFocusables(self: *Tree, node: *Node, out: *std.ArrayListUnmanaged(NodeId)) !void {
        if (!node.visible) return;
        if (node.focusable and node.enabled) {
            try out.append(self.allocator, node.id);
        }
        for (node.children.items) |id| {
            try self.collectFocusables(self.get(id), out);
        }
    }

    pub fn tabFocus(self: *Tree, backwards: bool) void {
        var focusables: std.ArrayListUnmanaged(NodeId) = .empty;
        defer focusables.deinit(self.allocator);
        self.collectFocusables(self.get(self.root), &focusables) catch return;
        if (focusables.items.len == 0) return;
        var index: usize = 0;
        if (self.focused) |focused_id| {
            for (focusables.items, 0..) |id, i| {
                if (id == focused_id) {
                    index = i;
                    break;
                }
            }
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
        switch (target) {
            .left => input.moveLeft(data, extend),
            .right => input.moveRight(data, extend),
            .home => input.moveHome(data, extend),
            .end => input.moveEnd(data, extend),
            .word_left => input.moveWordLeft(data, extend),
            .word_right => input.moveWordRight(data, extend),
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
        self.markDirty(id, false, true);
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
