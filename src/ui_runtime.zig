const std = @import("std");
const platform = @import("platform/platform.zig");
const Canvas = @import("renderer/canvas.zig").Canvas;
const ui = @import("ui.zig");
const node_mod = @import("core/node.zig");
const tree_mod = @import("core/tree.zig");
const actions = @import("core/actions.zig");
const events = @import("core/events.zig");
const input_mod = @import("components/input.zig");
const button_mod = @import("components/button.zig");
const toggle_mod = @import("components/toggle.zig");
const tokens = @import("theme/tokens.zig");
const geometry = @import("core/geometry.zig");
const text_mod = @import("renderer/text.zig");
const select_mod = @import("components/select.zig");

const Tree = tree_mod.Tree;
const Node = node_mod.Node;
const NodeId = node_mod.NodeId;
const Theme = tokens.Theme;
const spec = ui;

pub const Config = struct {
    title: [:0]const u8 = "Zant UI",
    window: platform.WindowOptions = .{},
    theme: Theme = .{},
};

pub const Runtime = struct {
    allocator: std.mem.Allocator,
    theme: Theme,
    config: Config,
    tree: *Tree,
    userdata: *anyopaque,
    render_fn: *const fn (userdata: *anyopaque) ui.Node,
    window: ?*platform.MacWindow = null,
    needs_build: bool = true,
    viewport: geometry.Size = .{},
    last_width: f32 = 0,
    last_height: f32 = 0,
    last_tick_ms: i64 = 0,
    now_ms: i64 = 0,
    next_delay_ms: c_int = -1,
    any_animation: bool = false,
    hovered: ?NodeId = null,
    pressed_button: ?NodeId = null,
    pressed_toggle: ?NodeId = null,
    drag_input: ?NodeId = null,
    active: bool = true,
    tooltip_target: ?NodeId = null,
    tooltip_show_at: i64 = 0,
    tooltip_visible: bool = false,
    tooltip_layout: ?text_mod.TextLayout = null,
    open_select: ?NodeId = null,

    pub fn init(
        allocator: std.mem.Allocator,
        userdata: *anyopaque,
        render_fn: *const fn (userdata: *anyopaque) ui.Node,
        config: Config,
    ) !*Runtime {
        const self = try allocator.create(Runtime);
        errdefer allocator.destroy(self);
        const tree = try allocator.create(Tree);
        errdefer allocator.destroy(tree);
        tree.* = try Tree.init(allocator, config.theme);
        errdefer tree.deinit();
        const root = tree.get(tree.root);
        root.style.direction = .column;
        root.style.align_items = .stretch;
        root.visual.background = config.window.background;
        self.* = .{
            .allocator = allocator,
            .theme = config.theme,
            .config = config,
            .tree = tree,
            .userdata = userdata,
            .render_fn = render_fn,
            .last_tick_ms = monotonicMs(),
        };
        return self;
    }

    pub fn deinit(self: *Runtime) void {
        if (self.tooltip_layout) |*layout| layout.deinit();
        self.tree.deinit();
        self.allocator.destroy(self.tree);
        self.allocator.destroy(self);
    }

    pub fn invalidate(self: *Runtime) void {
        self.needs_build = true;
        platform.requestRedraw(self.window, 0);
    }

    pub fn invalidateCallback(ctx: *anyopaque) void {
        const self: *Runtime = @ptrCast(@alignCast(ctx));
        self.invalidate();
    }

    pub fn run(self: *Runtime) !void {
        const host = makeHost(self);
        const window = try platform.createWindow(self.config.title, self.config.window, &host);
        self.window = window;
        defer {
            platform.destroy(window);
            self.window = null;
        }
        std.debug.print("Zant UI app — renderer: {s}\n", .{platform.rendererName(window)});
        if (std.c.getenv("ZANT_SNAPSHOT")) |path| {
            const len = std.mem.len(path);
            const copy = try self.allocator.allocSentinel(u8, len, 0);
            defer self.allocator.free(copy);
            @memcpy(copy[0..len], path[0..len]);
            platform.scheduleSnapshot(window, copy, 600);
        }
        platform.run(window);
    }

    fn rebuild(self: *Runtime) void {
        ui.beginFrame();
        defer ui.endFrame();
        const root = self.render_fn(self.userdata);
        if (root.element.kind != .container) {
            std.debug.print("Zant UI: render() must return a container (ui.column/ui.row/ui.card)\n", .{});
            self.needs_build = false;
            return;
        }
        _ = self.reconcileRoot(root) catch |err| {
            std.debug.print("Zant UI: reconcile failed: {s}\n", .{@errorName(err)});
            self.needs_build = false;
            return;
        };
        self.needs_build = false;
    }

    fn reconcileRoot(self: *Runtime, root_spec: ui.Node) anyerror!bool {
        const root = self.tree.get(self.tree.root);
        var changed_layout = !std.meta.eql(root.style, root_spec.element.layout) or
            !std.meta.eql(root.visual, root_spec.element.visual);
        root.style = root_spec.element.layout;
        root.style.width = null;
        root.style.height = null;
        root.style.grow = 0;
        root.visual = root_spec.element.visual;
        if (root.visual.background == null) root.visual.background = self.theme.color_bg_base;
        root.enabled = true;
        root.focusable = false;
        if (try self.reconcileChildren(root.id, root_spec.element.children)) changed_layout = true;
        if (changed_layout) self.tree.markDirty(root.id, true, true);
        return changed_layout;
    }

    fn reconcileChildren(self: *Runtime, parent_id: NodeId, specs: []const ui.Node) anyerror!bool {
        const parent = self.tree.get(parent_id);
        const old_len = parent.children.items.len;
        const old = self.allocator.alloc(NodeId, old_len) catch return error.OutOfMemory;
        defer self.allocator.free(old);
        @memcpy(old, parent.children.items);
        const used = self.allocator.alloc(bool, old_len) catch return error.OutOfMemory;
        defer self.allocator.free(used);
        @memset(used, false);

        var new_children: std.ArrayListUnmanaged(NodeId) = .empty;
        defer new_children.deinit(self.allocator);
        var changed_layout = false;
        var changed_children = false;

        for (specs, 0..) |child_spec, i| {
            const found: ?usize = blk: {
                if (child_spec.element.key) |key| {
                    for (old, 0..) |id, j| {
                        if (used[j]) continue;
                        const node = self.tree.get(id);
                        if (node.key) |old_key| {
                            if (std.mem.eql(u8, old_key, key) and kindCompatible(node, child_spec.element.kind)) {
                                break :blk j;
                            }
                        }
                    }
                }
                if (i < old_len and !used[i] and kindCompatible(self.tree.get(old[i]), child_spec.element.kind)) {
                    break :blk i;
                }
                break :blk null;
            };

            if (found) |index| {
                used[index] = true;
                const id = old[index];
                if (try self.patchNode(id, child_spec)) changed_layout = true;
                try new_children.append(self.allocator, id);
            } else {
                const id = try self.createNode(child_spec);
                self.tree.get(id).parent = parent_id;
                try new_children.append(self.allocator, id);
                changed_layout = true;
                changed_children = true;
            }
        }

        for (old, 0..) |id, j| {
            if (used[j]) continue;
            self.tree.detach(id);
            changed_layout = true;
            changed_children = true;
        }

        if (old_len != new_children.items.len) {
            changed_children = true;
        } else if (!std.mem.eql(NodeId, old, new_children.items)) {
            changed_children = true;
            changed_layout = true;
        }

        if (changed_children) {
            parent.children.clearRetainingCapacity();
            parent.children.appendSlice(self.allocator, new_children.items) catch return error.OutOfMemory;
            for (new_children.items) |id| self.tree.get(id).parent = parent_id;
            self.tree.markDirty(parent_id, true, true);
        }
        if (changed_layout) self.tree.markDirty(parent_id, true, true);
        return changed_layout;
    }

    fn kindCompatible(node: *Node, kind: node_mod.NodeKind) bool {
        if (node.kind() != kind) return false;
        return true;
    }

    fn applyCommon(self: *Runtime, id: NodeId, element: *const ui.Element) bool {
        const node = self.tree.get(id);
        var layout_changed = !std.meta.eql(node.style, element.layout) or
            !std.meta.eql(node.visual, element.visual);
        node.style = element.layout;
        node.visual = element.visual;
        if (node.enabled != element.enabled) {
            layout_changed = true;
            node.enabled = element.enabled;
            if (!element.enabled) {
                switch (node.data) {
                    .button => |*data| {
                        data.hovered = false;
                        data.pressed = false;
                        data.hover_t = 0;
                        data.press_t = 0;
                    },
                    .input => |*data| {
                        data.hovered = false;
                        data.focused = false;
                        data.blink_on = true;
                    },
                    .toggle => |*data| {
                        data.hovered = false;
                        data.pressed = false;
                    },
                    else => {},
                }
                if (self.tree.focused == id) self.tree.setFocus(null);
            }
        }
        node.focusable = element.focusable;
        if (node.key == null) {
            if (element.key) |key| self.tree.setKey(id, key);
        }
        const needs_tooltip = if (node.tooltip) |old| blk: {
            if (element.tooltip) |new| break :blk !std.mem.eql(u8, old, new);
            break :blk true;
        } else element.tooltip != null;
        if (needs_tooltip) self.tree.setTooltip(id, element.tooltip);
        return layout_changed;
    }

    fn patchNode(self: *Runtime, id: NodeId, child_spec: ui.Node) anyerror!bool {
        const node = self.tree.get(id);
        const element = child_spec.element;
        var layout_changed = self.applyCommon(id, element);
        var paint_changed = layout_changed;

        switch (node.data) {
            .container => {},
            .text => |*data| {
                if (!std.mem.eql(u8, data.content, element.content)) {
                    try self.tree.setText(id, element.content);
                    layout_changed = true;
                    paint_changed = true;
                }
                if (!std.meta.eql(data.style, element.text_style)) {
                    data.style = element.text_style;
                    data.invalidate();
                    layout_changed = true;
                    paint_changed = true;
                }
            },
            .button => |*data| {
                if (!std.mem.eql(u8, data.label, element.content)) {
                    const owned = try self.tree.allocator.dupe(u8, element.content);
                    self.tree.allocator.free(data.label);
                    data.label = owned;
                    data.invalidateLayout();
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.button_type != element.button_type or data.size != element.button_size or data.font_size != button_mod.fontSize(element.button_size)) {
                    data.button_type = element.button_type;
                    data.size = element.button_size;
                    data.font_size = button_mod.fontSize(element.button_size);
                    data.invalidateLayout();
                    layout_changed = true;
                    paint_changed = true;
                }
                data.action = element.on_click;
                data.index_action = element.index_action;
                data.index = element.index;
                node.focusable = true;
            },
            .input => |*data| {
                if (element.controlled and !std.mem.eql(u8, data.text.items, element.content)) {
                    self.tree.inputSetValue(id, element.content);
                    paint_changed = true;
                }
                if (!std.mem.eql(u8, data.placeholder, element.placeholder)) {
                    self.tree.inputSetPlaceholder(id, element.placeholder);
                    paint_changed = true;
                }
                if (data.read_only != element.read_only) {
                    data.read_only = element.read_only;
                    paint_changed = true;
                }
                if (data.password != element.password) {
                    data.password = element.password;
                    input_mod.rebuildDisplay(self.tree.allocator, data);
                    paint_changed = true;
                }
                if (data.font_size != element.text_style.font_size) {
                    data.font_size = element.text_style.font_size;
                    data.invalidateLayout();
                    paint_changed = true;
                }
                data.text_action = element.on_text;
                data.submit_action = element.on_click;
                node.focusable = true;
            },
            .select => |*data| {
                if (!selectOptionsEqual(data, element.options)) {
                    data.clearOptions(self.tree.allocator);
                    for (element.options) |label| {
                        const owned = try self.tree.allocator.dupe(u8, label);
                        try data.options.append(self.tree.allocator, .{ .label = owned });
                    }
                    if (data.selected >= data.options.items.len) data.selected = 0;
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.selected != element.selected) {
                    data.selected = @min(element.selected, if (data.options.items.len > 0) data.options.items.len - 1 else 0);
                    data.layout_dirty = true;
                    paint_changed = true;
                }
                data.action = element.index_action;
                node.focusable = true;
            },
            .image => |*data| {
                if (!std.mem.eql(u8, data.path, element.content)) {
                    const owned = try self.tree.allocator.dupe(u8, element.content);
                    self.tree.allocator.free(data.path);
                    data.path = owned;
                    @import("components/image.zig").load(data);
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.fit != element.image_fit) {
                    data.fit = element.image_fit;
                    paint_changed = true;
                }
            },
            .toggle => |*data| {
                if (!std.mem.eql(u8, data.label, element.content)) {
                    const owned = try self.tree.allocator.dupe(u8, element.content);
                    self.tree.allocator.free(data.label);
                    data.label = owned;
                    data.invalidateLayout();
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.kind != element.toggle_kind) {
                    data.kind = element.toggle_kind;
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.font_size != element.text_style.font_size) {
                    data.font_size = element.text_style.font_size;
                    data.invalidateLayout();
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.checked != element.checked) {
                    data.checked = element.checked;
                    paint_changed = true;
                }
                data.action = element.on_bool;
                data.index_action = element.index_action;
                data.index = element.index;
                node.focusable = true;
            },
            else => {},
        }

        if (std.meta.activeTag(node.data) == .container) {
            if (try self.reconcileChildren(id, element.children)) layout_changed = true;
        }
        if (layout_changed) {
            self.tree.markDirty(id, true, true);
        } else if (paint_changed) {
            self.tree.markDirty(id, false, true);
        }
        return layout_changed;
    }

    fn createNode(self: *Runtime, child_spec: ui.Node) anyerror!NodeId {
        const element = child_spec.element;
        const id: NodeId = switch (element.kind) {
            .container => try self.tree.addContainer(null, element.layout),
            .text => try self.tree.addText(null, element.content, element.text_style),
            .button => try self.tree.addButton(null, element.content, element.button_type, element.button_size, null),
            .input => try self.tree.addInput(null, element.content, element.placeholder, null),
            .toggle => try self.tree.addToggle(null, element.content, element.toggle_kind, element.checked),
            .image => try self.tree.addImage(null, element.content, element.image_fit, element.visual.radius),
            .select => try self.tree.addSelect(null, element.options, element.selected),
            else => return error.InvalidElement,
        };
        _ = self.applyCommon(id, element);
        const node = self.tree.get(id);
        switch (node.data) {
            .button => |*data| {
                data.action = element.on_click;
                data.index_action = element.index_action;
                data.index = element.index;
                data.font_size = button_mod.fontSize(element.button_size);
                data.invalidateLayout();
            },
            .input => |*data| {
                data.text_action = element.on_text;
                data.submit_action = element.on_click;
                data.read_only = element.read_only;
                data.password = element.password;
                data.multiline = element.multiline;
                data.font_size = element.text_style.font_size;
                input_mod.rebuildDisplay(self.tree.allocator, data);
            },
            .toggle => |*data| {
                data.action = element.on_bool;
                data.index_action = element.index_action;
                data.index = element.index;
                data.font_size = element.text_style.font_size;
                data.checked = element.checked;
                data.check_t = if (element.checked) 1 else 0;
            },
            .image => |*data| {
                data.fit = element.image_fit;
            },
            .select => |*data| {
                data.action = element.index_action;
                data.selected = element.selected;
            },
            else => {},
        }
        if (element.key) |key| self.tree.setKey(id, key);
        if (element.kind == .container) {
            _ = try self.reconcileChildren(id, element.children);
        }
        return id;
    }

    fn selectOptionsEqual(data: *const node_mod.SelectData, options: []const []const u8) bool {
        if (data.options.items.len != options.len) return false;
        for (data.options.items, options) |old, new| {
            if (!std.mem.eql(u8, old.label, new)) return false;
        }
        return true;
    }

    fn paintSelectPopup(self: *Runtime, canvas: Canvas) void {
        const id = self.open_select orelse return;
        if (id >= self.tree.nodes.items.len) return;
        const node = self.tree.get(id);
        if (std.meta.activeTag(node.data) != .select or !node.data.select.open) return;
        select_mod.paintPopup(node, canvas, self.theme, self.viewport);
    }

    fn closeSelect(self: *Runtime) void {
        if (self.open_select) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .select) {
                node.data.select.open = false;
                node.data.select.hovered_option = null;
                self.tree.markDirty(id, false, true);
            }
            self.open_select = null;
        }
    }

    fn tick(self: *Runtime) void {
        self.now_ms = monotonicMs();
        var dt: f32 = @floatFromInt(@max(0, self.now_ms - self.last_tick_ms));
        dt = @min(dt, 50);
        self.last_tick_ms = self.now_ms;
        self.any_animation = false;
        for (self.tree.nodes.items) |node| {
            if (!node.enabled) continue;
            switch (node.data) {
                .button => |*data| {
                    if (button_mod.animate(data, dt, self.theme)) {
                        self.any_animation = true;
                        self.tree.markDirty(node.id, false, true);
                    }
                },
                .toggle => |*data| {
                    if (toggle_mod.animate(data, dt, self.theme)) {
                        self.any_animation = true;
                        self.tree.markDirty(node.id, false, true);
                    }
                },
                .input => |*data| {
                    if (self.tree.focused == node.id) {
                        if (input_mod.updateBlink(data, node, self.now_ms)) {
                            self.tree.markDirty(node.id, false, true);
                        }
                    }
                },
                else => {},
            }
        }
        if (self.tooltip_target != null and !self.tooltip_visible and self.now_ms >= self.tooltip_show_at) {
            self.tooltip_visible = true;
            self.tree.markPaintDirty();
        }
    }

    fn computeNextDelay(self: *Runtime) void {
        var delay: c_int = -1;
        if (self.any_animation) delay = 16;
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (node.enabled and std.meta.activeTag(node.data) == .input) {
                const data = &node.data.input;
                const remaining: i64 = @max(1, data.next_blink_ms - self.now_ms);
                const blink_delay: c_int = @intCast(@min(remaining, 2000));
                if (delay < 0 or blink_delay < delay) delay = blink_delay;
            }
        }
        if (self.tooltip_target != null and !self.tooltip_visible) {
            const remaining: i64 = @max(1, self.tooltip_show_at - self.now_ms);
            const tooltip_delay: c_int = @intCast(@min(remaining, 1000));
            if (delay < 0 or tooltip_delay < delay) delay = tooltip_delay;
        }
        self.next_delay_ms = delay;
    }

    pub fn renderFrame(self: *Runtime, canvas: Canvas, width: f32, height: f32, dpr: f32) void {
        _ = dpr;
        if (self.needs_build) self.rebuild();
        self.tick();
        self.viewport = .{ .width = width, .height = height };
        if (width != self.last_width or height != self.last_height) {
            self.last_width = width;
            self.last_height = height;
            self.tree.markLayoutDirty();
        }
        if (self.tree.needsLayout()) self.tree.layout(self.viewport);
        canvas.clear(self.config.window.background);
        self.tree.paint(canvas);
        self.paintSelectPopup(canvas);
        self.paintTooltip(canvas);
        self.tree.clearPaint();
        self.computeNextDelay();
    }

    pub fn dispatch(self: *Runtime, event: events.Event) c_int {
        switch (event) {
            .pointer_move => |p| {
                self.handlePointerMove(p);
                return 1;
            },
            .pointer_enter => |p| {
                self.handlePointerMove(p);
                return 1;
            },
            .pointer_down => |p| {
                self.handlePointerDown(p);
                self.needs_build = true;
                return 1;
            },
            .pointer_up => |p| {
                self.handlePointerUp(p);
                self.needs_build = true;
                return 1;
            },
            .pointer_leave => {
                self.setHover(null);
                self.drag_input = null;
                self.clearPressed();
                platform.setCursor(self.window, .arrow);
                return 1;
            },
            .key_down => |k| {
                const result = self.handleKeyDown(k);
                self.needs_build = true;
                return result;
            },
            .key_up => return 0,
            .text_input => |value| {
                const result = self.handleTextInput(value);
                self.needs_build = true;
                return result;
            },
            .composition => |composition| {
                const result = self.handleComposition(composition);
                self.needs_build = true;
                return result;
            },
            .window_blur => {
                self.active = false;
                self.cancelFocusedComposition();
                self.closeSelect();
                self.tree.setFocus(null);
                self.setHover(null);
                self.clearPressed();
                self.tree.markPaintDirty();
                return 0;
            },
            .window_focus => {
                self.active = true;
                return 0;
            },
            .window_resize => {
                self.tree.markLayoutDirty();
                return 0;
            },
            .window_close => return 0,
            .scroll => return 0,
        }
    }

    fn clearPressed(self: *Runtime) void {
        if (self.pressed_button) |id| {
            self.tree.get(id).data.button.pressed = false;
            self.tree.markDirty(id, false, true);
        }
        if (self.pressed_toggle) |id| {
            self.tree.get(id).data.toggle.pressed = false;
            self.tree.markDirty(id, false, true);
        }
        self.pressed_button = null;
        self.pressed_toggle = null;
    }

    fn handlePointerMove(self: *Runtime, p: events.PointerEvent) void {
        if (self.open_select) |sid| {
            const snode = self.tree.get(sid);
            if (std.meta.activeTag(snode.data) == .select) {
                const hovered = select_mod.optionAt(snode, p.x, p.y, self.viewport);
                if (snode.data.select.hovered_option != hovered) {
                    snode.data.select.hovered_option = hovered;
                    self.tree.markDirty(sid, false, true);
                }
                platform.setCursor(self.window, if (snode.data.select.open) .pointing_hand else .arrow);
                return;
            }
        }
        const target = self.tree.hitTest(p.x, p.y);
        self.setHover(target);
        if (self.pressed_button) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            if (node.data.button.pressed != inside) {
                node.data.button.pressed = inside;
                self.tree.markDirty(id, false, true);
            }
        }
        if (self.pressed_toggle) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            if (node.data.toggle.pressed != inside) {
                node.data.toggle.pressed = inside;
                self.tree.markDirty(id, false, true);
            }
        }
        if (self.drag_input) |id| self.setInputCursor(id, p.x, p.y, true);
        self.updateCursor(target);
    }

    fn setHover(self: *Runtime, target: ?NodeId) void {
        if (self.hovered == target) return;
        self.clearTooltip();
        if (self.hovered) |previous| {
            const node = self.tree.get(previous);
            switch (node.data) {
                .button => |*data| data.hovered = false,
                .input => |*data| data.hovered = false,
                .toggle => |*data| data.hovered = false,
                else => {},
            }
            self.tree.markDirty(previous, false, true);
        }
        self.hovered = target;
        if (target) |id| {
            const node = self.tree.get(id);
            if (node.tooltip != null) {
                self.tooltip_target = id;
                self.tooltip_show_at = self.now_ms + 450;
            }
            switch (node.data) {
                .button => |*data| data.hovered = node.enabled,
                .input => |*data| data.hovered = node.enabled,
                .toggle => |*data| data.hovered = node.enabled,
                else => {},
            }
            self.tree.markDirty(id, false, true);
        }
    }

    fn clearTooltip(self: *Runtime) void {
        if (self.tooltip_layout) |*layout| layout.deinit();
        self.tooltip_layout = null;
        self.tooltip_target = null;
        self.tooltip_visible = false;
        self.tooltip_show_at = 0;
    }

    fn paintTooltip(self: *Runtime, canvas: Canvas) void {
        if (!self.tooltip_visible) return;
        const id = self.tooltip_target orelse return;
        const node = self.tree.get(id);
        if (!node.visible) return;
        const content = node.tooltip orelse return;
        if (self.tooltip_layout == null) {
            self.tooltip_layout = text_mod.TextLayout.init(content, .{
                .font_size = 12,
                .color = 0xffffffff,
                .wrap = false,
            }, 1.0e6) catch return;
        }
        const layout = &self.tooltip_layout.?;
        const w = layout.metrics.longest_line + 16;
        const h = layout.metrics.height + 8;
        var x = node.bounds.x;
        var y = node.bounds.bottom() + 6;
        if (x + w > self.viewport.width - 8) x = @max(8, self.viewport.width - w - 8);
        if (y + h > self.viewport.height - 8) y = @max(8, node.bounds.y - h - 6);
        canvas.fillRoundRect(geometry.Rect.init(x, y, w, h), 4, 0xf0000000);
        layout.paint(canvas, x + 8, y + 4);
    }

    fn updateCursor(self: *Runtime, target: ?NodeId) void {
        const cursor: platform.Cursor = if (target) |id| switch (self.tree.get(id).data) {
            .input => .ibeam,
            .button, .toggle => .pointing_hand,
            else => .arrow,
        } else .arrow;
        platform.setCursor(self.window, cursor);
    }

    fn handlePointerDown(self: *Runtime, p: events.PointerEvent) void {
        if (self.open_select) |sid| {
            const snode = self.tree.get(sid);
            if (std.meta.activeTag(snode.data) == .select) {
                if (select_mod.optionAt(snode, p.x, p.y, self.viewport)) |index| {
                    snode.data.select.selected = index;
                    snode.data.select.layout_dirty = true;
                    if (snode.data.select.action) |action| action.func(action.ctx, index);
                    self.closeSelect();
                    self.needs_build = true;
                    return;
                }
                if (snode.bounds.contains(p.x, p.y)) {
                    self.closeSelect();
                    return;
                }
            }
            self.closeSelect();
            return;
        }
        const target = self.tree.hitTest(p.x, p.y);
        self.setHover(target);
        if (target) |id| {
            const node = self.tree.get(id);
            switch (node.data) {
                .button => {
                    self.focusNode(id);
                    if (!node.enabled) return;
                    node.data.button.pressed = true;
                    node.data.button.hovered = true;
                    self.pressed_button = id;
                    self.tree.markDirty(id, false, true);
                },
                .toggle => {
                    self.focusNode(id);
                    if (!node.enabled) return;
                    node.data.toggle.pressed = true;
                    node.data.toggle.hovered = true;
                    self.pressed_toggle = id;
                    self.tree.markDirty(id, false, true);
                },
                .input => {
                    if (!node.enabled) return;
                    self.focusNode(id);
                    self.drag_input = id;
                    self.setInputCursor(id, p.x, p.y, p.modifiers.shift);
                },
                .select => {
                    if (!node.enabled) return;
                    self.focusNode(id);
                    node.data.select.open = true;
                    node.data.select.hovered_option = null;
                    self.open_select = id;
                    self.tree.markDirty(id, false, true);
                },
                else => {},
            }
            self.updateCursor(target);
        } else {
            self.focusNode(null);
        }
    }

    fn handlePointerUp(self: *Runtime, p: events.PointerEvent) void {
        self.drag_input = null;
        if (self.pressed_button) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            node.data.button.pressed = false;
            if (inside and node.enabled) {
                if (node.data.button.index_action) |action| {
                    action.func(action.ctx, node.data.button.index);
                } else if (node.data.button.action) |action| {
                    action.func(action.ctx);
                }
                if (node.data.button.on_click) |callback| callback.func(callback.ctx, id);
            }
            self.pressed_button = null;
            self.tree.markDirty(id, false, true);
        }
        if (self.pressed_toggle) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            node.data.toggle.pressed = false;
            if (inside and node.enabled) {
                if (node.data.toggle.kind == .radio) {
                    if (!node.data.toggle.checked) self.tree.toggleSet(id, true);
                } else {
                    self.tree.toggleFlip(id);
                }
            }
            self.pressed_toggle = null;
            self.tree.markDirty(id, false, true);
        }
        self.updateCursor(self.tree.hitTest(p.x, p.y));
    }

    fn focusNode(self: *Runtime, id: ?NodeId) void {
        if (self.tree.focused) |old| {
            if (old != id) self.cancelCompositionFor(old);
        }
        self.tree.setFocus(id);
    }

    fn cancelCompositionFor(self: *Runtime, id: NodeId) void {
        const node = self.tree.get(id);
        if (std.meta.activeTag(node.data) != .input) return;
        if (node.data.input.has_composition) self.tree.inputCancelComposition(id);
    }

    fn cancelFocusedComposition(self: *Runtime) void {
        if (self.tree.focused) |id| self.cancelCompositionFor(id);
    }

    fn setInputCursor(self: *Runtime, id: NodeId, x: f32, y: f32, extend: bool) void {
        const node = self.tree.get(id);
        const data = &node.data.input;
        if (data.has_composition) return;
        const layout = input_mod.ensureLayout(data) catch return;
        const origin_x = node.bounds.x + input_mod.padding_x - data.scroll_x;
        const origin_y = node.bounds.y + (node.bounds.height - layout.metrics.height) * 0.5;
        var byte = layout.indexAt(x - origin_x, y - origin_y);
        byte = @min(byte, data.text.items.len);
        data.cursor = byte;
        if (!extend) data.anchor = byte;
        self.tree.markDirty(id, false, true);
    }

    fn handleKeyDown(self: *Runtime, k: events.KeyEvent) c_int {
        const shift = k.modifiers.shift;
        switch (k.key) {
            .tab => {
                self.cancelFocusedComposition();
                self.tree.tabFocus(shift);
                return 1;
            },
            .escape => {
                if (self.tree.focused) |id| {
                    const node = self.tree.get(id);
                    if (std.meta.activeTag(node.data) == .input and node.data.input.has_composition) {
                        self.tree.inputCancelComposition(id);
                        return 1;
                    }
                }
                self.focusNode(null);
                return 1;
            },
            else => {},
        }

        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            switch (node.data) {
                .button => |*data| {
                    if (k.key == .enter or events.characterEquals(k.key, ' ')) {
                        if (data.index_action) |action| {
                            action.func(action.ctx, data.index);
                        } else if (data.action) |action| {
                            action.func(action.ctx);
                        }
                        if (data.on_click) |callback| callback.func(callback.ctx, id);
                        return 1;
                    }
                },
                .toggle => |*data| {
                    if (k.key == .enter or events.characterEquals(k.key, ' ')) {
                        if (data.kind == .radio) {
                            if (!data.checked) self.tree.toggleSet(id, true);
                        } else {
                            self.tree.toggleFlip(id);
                        }
                        return 1;
                    }
                },
                .select => |*data| {
                    if (k.key == .escape) {
                        self.closeSelect();
                        return 1;
                    }
                    if (k.key == .enter or events.characterEquals(k.key, ' ')) {
                        if (data.open) {
                            self.closeSelect();
                        } else {
                            data.open = true;
                            self.open_select = id;
                            self.tree.markDirty(id, false, true);
                        }
                        return 1;
                    }
                    if (k.key == .up or k.key == .down) {
                        if (data.options.items.len == 0) return 1;
                        if (k.key == .up) {
                            data.selected = if (data.selected == 0) data.options.items.len - 1 else data.selected - 1;
                        } else {
                            data.selected = (data.selected + 1) % data.options.items.len;
                        }
                        data.layout_dirty = true;
                        if (data.action) |action| action.func(action.ctx, data.selected);
                        self.tree.markDirty(id, false, true);
                        return 1;
                    }
                    return 0;
                },
                .input => |*data| {
                    if (data.has_composition) return 0;
                    if (k.modifiers.meta or k.modifiers.control) {
                        if (events.characterEquals(k.key, 'a')) {
                            self.tree.inputSelectAll(id);
                            return 1;
                        }
                        if (events.characterEquals(k.key, 'c')) {
                            _ = self.copySelection(id);
                            return 1;
                        }
                        if (events.characterEquals(k.key, 'x')) {
                            if (self.copySelection(id)) _ = self.tree.inputDeleteBackward(id);
                            return 1;
                        }
                        if (events.characterEquals(k.key, 'v')) {
                            self.pasteInto(id);
                            return 1;
                        }
                        return 0;
                    }
                    switch (k.key) {
                        .left => {
                            if (k.modifiers.alt) self.tree.inputMove(id, .word_left, shift) else self.tree.inputMove(id, .left, shift);
                            return 1;
                        },
                        .right => {
                            if (k.modifiers.alt) self.tree.inputMove(id, .word_right, shift) else self.tree.inputMove(id, .right, shift);
                            return 1;
                        },
                        .home => {
                            self.tree.inputMove(id, .home, shift);
                            return 1;
                        },
                        .end => {
                            self.tree.inputMove(id, .end, shift);
                            return 1;
                        },
                        .backspace => {
                            _ = self.tree.inputDeleteBackward(id);
                            return 1;
                        },
                        .delete => {
                            _ = self.tree.inputDeleteForward(id);
                            return 1;
                        },
                        .enter => {
                            if (data.multiline) {
                                _ = self.tree.inputInsert(id, "\n");
                            } else if (data.submit_action) |action| action.func(action.ctx);
                            return 1;
                        },
                        .character => return 0,
                        else => return 0,
                    }
                },
                else => {},
            }
        }
        return 0;
    }

    fn handleTextInput(self: *Runtime, value: []const u8) c_int {
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .input) {
                if (self.tree.inputInsert(id, value)) return 1;
            }
        }
        return 0;
    }

    fn handleComposition(self: *Runtime, composition: events.Composition) c_int {
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .input) {
                if (composition.text.len == 0) {
                    self.tree.inputCancelComposition(id);
                } else {
                    self.tree.inputComposition(id, composition.text, composition.selection_start, composition.selection_end);
                }
                return 1;
            }
        }
        return 0;
    }

    fn copySelection(self: *Runtime, id: NodeId) bool {
        const node = self.tree.get(id);
        const data = &node.data.input;
        if (!input_mod.hasSelection(data)) return false;
        const range = input_mod.selectionRange(data);
        if (range.start == range.end) return false;
        return platform.setClipboard(data.text.items[range.start..range.end]);
    }

    fn pasteInto(self: *Runtime, id: NodeId) void {
        const clipboard = platform.clipboardAlloc(self.allocator) catch return;
        defer self.allocator.free(clipboard);
        if (clipboard.len == 0) return;
        _ = self.tree.inputInsert(id, clipboard);
    }

    fn imeRect(self: *Runtime) ?geometry.Rect {
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .input) return self.tree.inputCaretRect(id);
        }
        return null;
    }
};

fn monotonicMs() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.MONOTONIC, &ts);
    return @as(i64, @intCast(ts.sec)) * 1000 + @divTrunc(@as(i64, @intCast(ts.nsec)), 1_000_000);
}

fn drawCallback(userdata: ?*anyopaque, canvas_ptr: ?*anyopaque, width: f32, height: f32, dpr: f32) callconv(.c) void {
    const self: *Runtime = @ptrCast(@alignCast(userdata orelse return));
    self.renderFrame(Canvas.fromOpaque(canvas_ptr), width, height, dpr);
}

fn eventCallback(userdata: ?*anyopaque, raw: *const platform.Event) callconv(.c) c_int {
    const self: *Runtime = @ptrCast(@alignCast(userdata orelse return 0));
    return self.dispatch(events.translate(raw));
}

fn imeRectCallback(userdata: ?*anyopaque, x: *f32, y: *f32, w: *f32, h: *f32) callconv(.c) c_int {
    const self: *Runtime = @ptrCast(@alignCast(userdata orelse return 0));
    if (self.imeRect()) |rect| {
        x.* = rect.x;
        y.* = rect.y;
        w.* = rect.width;
        h.* = rect.height;
        return 1;
    }
    return 0;
}

fn nextFrameDelayCallback(userdata: ?*anyopaque) callconv(.c) c_int {
    const self: *Runtime = @ptrCast(@alignCast(userdata orelse return -1));
    return self.next_delay_ms;
}

fn makeHost(runtime: *Runtime) platform.Host {
    return .{
        .userdata = runtime,
        .draw = drawCallback,
        .event = eventCallback,
        .ime_rect = imeRectCallback,
        .next_frame_delay = nextFrameDelayCallback,
    };
}
