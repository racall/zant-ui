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
const motion = @import("motion/motion.zig");
const toggle_mod = @import("components/toggle.zig");
const tokens = @import("theme/tokens.zig");
const geometry = @import("core/geometry.zig");
const text_mod = @import("renderer/text.zig");
const select_mod = @import("components/select.zig");
const text_select = @import("core/text_select.zig");

const Tree = tree_mod.Tree;
const Node = node_mod.Node;
const NodeId = node_mod.NodeId;
const Theme = tokens.Theme;
const spec = ui;

pub const Config = struct {
    title: [:0]const u8 = "Zant UI",
    window: platform.WindowOptions = .{},
    theme: Theme = .{},
    /// Optional per-frame theme provider (dark mode etc.). Called before every
    /// rebuild; a changed theme invalidates cached text layouts and repaints.
    theme_fn: ?*const fn (userdata: *anyopaque) Theme = null,
    /// Optional application key handler, called before focused widgets see
    /// the key (except during IME composition). Return true to consume it.
    key_fn: ?*const fn (userdata: *anyopaque, key: events.KeyEvent) bool = null,
    /// Asked before the window closes or the app quits (close button, ⌘W,
    /// ⌘Q). Return false to keep it open (e.g. unsaved changes).
    close_fn: ?*const fn (userdata: *anyopaque) bool = null,
};

pub const Runtime = struct {
    allocator: std.mem.Allocator,
    theme: Theme,
    config: Config,
    tree: *Tree,
    userdata: *anyopaque,
    render_fn: *const fn (userdata: *anyopaque) ui.Node,
    window: ?*platform.Window = null,
    needs_build: bool = true,
    /// > 0 while an event is being dispatched. Callbacks can run a native
    /// modal loop (file dialogs, confirm alerts) that paints the window
    /// meanwhile; such nested frames must not rebuild the tree or finish exit
    /// animations, because the dispatching code still holds node pointers.
    dispatch_depth: u32 = 0,
    /// ZANT_LAYOUT_FILE: after every frame, write "key x y width height"
    /// (logical px, tab separated) of each keyed node, for UI automation.
    layout_file: ?[:0]u8 = null,
    /// ZANT_FRAME_STATS=1: once a second print frames drawn, the slowest
    /// frame (time spent in renderFrame) and the longest gap between frames.
    frame_stats: ?FrameStats = null,
    viewport: geometry.Size = .{},
    last_width: f32 = 0,
    last_height: f32 = 0,
    /// Time source (ms). Tests inject a fake clock.
    clock: *const fn () i64 = monotonicMs,
    /// Timestamp of the current frame / event, sampled from `clock`.
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
    /// Show / hide transition (antd zoom-big-fast) and the box computed when
    /// the tooltip appeared (kept while it zooms out).
    tooltip_anim: motion.Animated = .{},
    tooltip_rect: geometry.Rect = .{},
    tooltip_above: bool = false,
    tooltip_hiding: bool = false,
    /// Select whose popup is painted (stays set while the popup closes).
    popup_select: ?NodeId = null,
    /// Rebuild requested by `ui.wakeAfter` (absolute ms).
    wake_at_ms: ?i64 = null,
    /// Last `ui.documentEdited` value sent to the window.
    document_edited: bool = false,
    open_select: ?NodeId = null,
    pressed_link: ?NodeId = null,
    /// Pointer area holding the pointer between `down` and `up`.
    pointer_capture: ?NodeId = null,
    /// Pointer area last sent `hover` (gets `leave` when the pointer goes).
    pointer_hover_area: ?NodeId = null,
    /// Multi-click detection (double / triple click).
    last_click_ms: i64 = 0,
    last_click_x: f32 = 0,
    last_click_y: f32 = 0,
    click_count: u8 = 0,
    /// Text selection in a `selectable` container.
    sel_root: ?NodeId = null,
    sel_anchor: ?text_select.Point = null,
    sel_focus: ?text_select.Point = null,
    sel_dragging: bool = false,
    sel_mode: enum { char, word, block } = .char,
    /// Word / block under the pointer when a multi-click drag started.
    sel_unit: [2]text_select.Point = undefined,
    /// Nodes whose `text_sel` is set (cleared on the next change).
    sel_marked: std.ArrayListUnmanaged(NodeId) = .empty,
    /// Window clear colour; follows the root background (dark mode).
    clear_color: u32 = 0,

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
        tree.selection_color = tokens.withAlpha(config.theme.color_primary, 0x4d);
        self.* = .{
            .allocator = allocator,
            .theme = config.theme,
            .config = config,
            .tree = tree,
            .userdata = userdata,
            .render_fn = render_fn,
            .clear_color = config.window.background,
        };
        return self;
    }

    pub fn deinit(self: *Runtime) void {
        if (self.tooltip_layout) |*layout| layout.deinit();
        self.sel_marked.deinit(self.allocator);
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
        // motion off for Reduce Motion, ZANT_NO_MOTION=1 and snapshots
        // (decided here, not in init, so tests keep control of it)
        const no_motion = if (platform.getenv("ZANT_NO_MOTION")) |v| v.len > 0 and v[0] == '1' else false;
        motion.enabled = !(no_motion or platform.getenv("ZANT_SNAPSHOT") != null or platform.reduceMotion());
        const host = makeHost(self);
        const window = try platform.createWindow(self.config.title, self.config.window, &host);
        self.window = window;
        ui.wake_window.store(window, .release);
        defer {
            ui.wake_window.store(null, .release);
            platform.destroy(window);
            self.window = null;
        }
        std.debug.print("Zant UI app — renderer: {s}\n", .{platform.rendererName(window)});
        if (platform.getenv("ZANT_LAYOUT_FILE")) |path| self.layout_file = try self.allocator.dupeSentinel(u8, path, 0);
        if (platform.getenv("ZANT_FRAME_STATS")) |v| {
            if (v.len > 0 and v[0] == '1') {
                self.frame_stats = .{};
                @import("renderer/text.zig").layout_stats.enabled = true;
            }
        }
        defer if (self.layout_file) |path| {
            self.allocator.free(path);
            self.layout_file = null;
        };
        if (platform.getenv("ZANT_SNAPSHOT")) |path| {
            const copy = try self.allocator.dupeSentinel(u8, path, 0);
            defer self.allocator.free(copy);
            platform.scheduleSnapshot(window, copy, 600);
        }
        platform.run(window);
    }

    /// Windows draws its caption buttons itself for `.hidden_titlebar`.
    fn drawsCaptionButtons(self: *const Runtime) bool {
        return platform.is_windows and self.config.window.style == .hidden_titlebar;
    }

    /// Height of the app-drawn title bar (Windows 11 default 32).
    fn captionHeight(self: *const Runtime) f32 {
        const h = self.config.window.titlebar_height;
        return if (h > 0) h else 32;
    }

    fn captionMinimize(self: *Runtime) void {
        platform.minimize(self.window);
    }

    fn captionMaximize(self: *Runtime) void {
        platform.toggleMaximize(self.window);
    }

    fn captionClose(self: *Runtime) void {
        platform.close(self.window);
    }

    /// Window-drag hit test for app-drawn title bars: interactive widgets
    /// stay clickable, the rest of the bar moves the window.
    pub fn titlebarHit(self: *Runtime, x: f32, y: f32) c_int {
        if (self.tree.hitTest(x, y)) |id| {
            const node = self.tree.get(id);
            if (node.key) |key| {
                if (std.mem.eql(u8, key, ui.caption_maximize_key)) return platform.Hit.maximize;
            }
            return platform.Hit.client;
        }
        return platform.Hit.caption;
    }

    /// Runs one rebuild + reconcile without painting (tests).
    pub fn rebuildForTest(self: *Runtime) void {
        self.rebuild();
    }

    fn rebuild(self: *Runtime) void {
        if (self.config.theme_fn) |theme_fn| self.setTheme(theme_fn(self.userdata));
        ui.setBuildTheme(self.theme);
        ui.setBuildWindowStyle(self.config.window.style);
        ui.setBuildTree(self.tree);
        defer ui.setBuildTree(null);
        ui.beginFrame();
        defer ui.endFrame();
        var root = self.render_fn(self.userdata);
        if (self.drawsCaptionButtons() and root.element.kind == .container) {
            root = ui.appendChildren(root, &.{ui.windowControls(.{
                .height = self.captionHeight(),
                .maximized = platform.isMaximized(self.window),
                .active = self.active,
                .on_minimize = ui.action(self, captionMinimize),
                .on_maximize = ui.action(self, captionMaximize),
                .on_close = ui.action(self, captionClose),
            })});
        }
        self.wake_at_ms = if (ui.takeWake()) |ms| self.now_ms + ms else null;
        if (ui.takeDocumentEdited()) |edited| {
            if (edited != self.document_edited) {
                self.document_edited = edited;
                if (self.window) |w| platform.setDocumentEdited(w, edited);
            }
        }
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
        // normalise the spec first, then compare: the root always fills the
        // window and needs a background
        var want = root_spec.element.layout;
        want.width = null;
        want.height = null;
        want.grow = 0;
        var want_visual = root_spec.element.visual;
        if (want_visual.background == null) want_visual.background = self.theme.color_bg_base;
        var changed_layout = !std.meta.eql(root.style, want) or !std.meta.eql(root.visual, want_visual);
        root.style = want;
        root.visual = want_visual;
        // keep the window's own background (visible while resizing) and the
        // frame clear colour in sync with the app's root background
        if (want_visual.background) |bg| {
            if (bg != self.clear_color and (bg >> 24) == 0xff) {
                self.clear_color = bg;
                if (self.window) |w| platform.setBackground(w, bg);
            }
        }
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
        // walks `old` for unkeyed specs
        var cursor: usize = 0;

        for (specs) |child_spec| {
            const found: ?usize = blk: {
                if (child_spec.element.key) |key| {
                    // keyed specs only ever match a node with the same key
                    for (old, 0..) |id, j| {
                        if (used[j]) continue;
                        const node = self.tree.get(id);
                        if (node.key) |old_key| {
                            if (std.mem.eql(u8, old_key, key) and kindCompatible(node, child_spec.element.kind)) {
                                break :blk j;
                            }
                        }
                    }
                    break :blk null;
                }
                // unkeyed specs match the next unused unkeyed node in order
                // (nodes playing an exit transition are never reused)
                while (cursor < old_len) : (cursor += 1) {
                    const candidate = self.tree.get(old[cursor]);
                    if (used[cursor] or candidate.key != null or candidate.exiting) continue;
                    const j = cursor;
                    cursor += 1;
                    if (kindCompatible(self.tree.get(old[j]), child_spec.element.kind)) break :blk j;
                    break :blk null;
                }
                break :blk null;
            };

            if (found) |index| {
                used[index] = true;
                const id = old[index];
                const node = self.tree.get(id);
                if (node.exiting) {
                    // re-added while leaving: play the enter transition again
                    self.reviveNode(id);
                    changed_layout = true;
                    changed_children = true;
                }
                if (child_spec.element.memo_hash) |hash| {
                    if (node.memo_hash != null and node.memo_hash.? == hash) {
                        try new_children.append(self.allocator, id);
                        continue;
                    }
                }
                const real = resolveMemo(child_spec);
                if (try self.patchNode(id, real)) changed_layout = true;
                self.tree.get(id).memo_hash = child_spec.element.memo_hash;
                try new_children.append(self.allocator, id);
            } else {
                const id = try self.createNode(resolveMemo(child_spec));
                self.tree.get(id).memo_hash = child_spec.element.memo_hash;
                self.tree.get(id).parent = parent_id;
                self.beginEnter(id);
                try new_children.append(self.allocator, id);
                changed_layout = true;
                changed_children = true;
            }
        }

        // unmatched nodes leave: with a presence transition they stay (out of
        // flow) until it ends, anchored after their previous live sibling
        var leaving: std.ArrayListUnmanaged([2]NodeId) = .empty; // (id, anchor or none)
        defer leaving.deinit(self.allocator);
        const none_anchor = std.math.maxInt(NodeId);
        var last_live: NodeId = none_anchor;
        for (old, 0..) |id, j| {
            if (used[j]) {
                last_live = id;
                continue;
            }
            const node = self.tree.get(id);
            if (!node.exiting) {
                if (!motion.enabled or node.motion.presence == .none) {
                    self.removeSubtree(id);
                    changed_layout = true;
                    changed_children = true;
                    continue;
                }
                self.beginExit(id);
                changed_layout = true;
                changed_children = true;
            }
            try leaving.append(self.allocator, .{ id, last_live });
        }
        if (leaving.items.len > 0) {
            var merged: std.ArrayListUnmanaged(NodeId) = .empty;
            defer merged.deinit(self.allocator);
            for (leaving.items) |pair| if (pair[1] == none_anchor) try merged.append(self.allocator, pair[0]);
            for (new_children.items) |id| {
                try merged.append(self.allocator, id);
                for (leaving.items) |pair| if (pair[1] == id) try merged.append(self.allocator, pair[0]);
            }
            // anchors that were removed themselves: keep those nodes at the end
            for (leaving.items) |pair| {
                if (std.mem.indexOfScalar(NodeId, merged.items, pair[0]) == null) try merged.append(self.allocator, pair[0]);
            }
            new_children.clearRetainingCapacity();
            try new_children.appendSlice(self.allocator, merged.items);
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

    /// Starts a freshly mounted node's enter transition.
    fn beginEnter(self: *Runtime, id: NodeId) void {
        const node = self.tree.get(id);
        const kind = node.motion.presence;
        if (kind == .none or !motion.enabled) return;
        const anim = self.tree.ensureAnim(id) orelse return;
        anim.presence = motion.Animated.start(0, 1, self.now_ms, motion.presenceEnter(kind));
        anim.fx = motion.presenceFx(kind, 0);
        self.tree.poke(id);
    }

    /// Starts the exit transition: the node stops taking input right away
    /// and is freed by `tick` when the transition ends.
    fn beginExit(self: *Runtime, id: NodeId) void {
        const node = self.tree.get(id);
        node.exiting = true;
        self.forgetSubtree(id);
        const anim = self.tree.ensureAnim(id) orelse return self.removeSubtree(id);
        anim.presence.retarget(0, self.now_ms, motion.presenceExit(node.motion.presence));
        self.tree.poke(id);
    }

    fn reviveNode(self: *Runtime, id: NodeId) void {
        const node = self.tree.get(id);
        node.exiting = false;
        if (node.anim) |anim| anim.presence.retarget(1, self.now_ms, motion.presenceEnter(node.motion.presence));
        self.tree.poke(id);
    }

    /// Clears every runtime reference to `id` (hover, press, drag, focus,
    /// tooltip, open select, link press). Any new field that keeps a NodeId
    /// across frames must be cleared here too, because ids are recycled.
    fn forgetNode(self: *Runtime, id: NodeId) void {
        if (self.hovered == id) self.hovered = null;
        if (self.pressed_button == id) self.pressed_button = null;
        if (self.pressed_toggle == id) self.pressed_toggle = null;
        if (self.drag_input == id) self.drag_input = null;
        if (self.pressed_link == id) self.pressed_link = null;
        if (self.pointer_capture == id) self.pointer_capture = null;
        if (self.pointer_hover_area == id) self.pointer_hover_area = null;
        if (self.sel_root == id or (self.sel_anchor != null and self.sel_anchor.?.node == id) or
            (self.sel_focus != null and self.sel_focus.?.node == id)) self.clearTextSelection();
        if (self.open_select == id) self.open_select = null;
        if (self.tooltip_target == id) self.clearTooltip();
        if (self.popup_select == id) self.popup_select = null;
        if (self.tree.focused == id) {
            self.cancelCompositionFor(id);
            self.tree.setFocus(null);
        }
    }

    fn forgetSubtree(self: *Runtime, id: NodeId) void {
        const node = self.tree.get(id);
        if (!node.alive) return;
        for (node.children.items) |child| self.forgetSubtree(child);
        self.forgetNode(id);
    }

    /// Removes a node that left the UI: clears references, frees the subtree.
    fn removeSubtree(self: *Runtime, id: NodeId) void {
        self.forgetSubtree(id);
        self.tree.freeSubtree(id);
    }

    /// Expands a `ui.memo` placeholder into its built subtree.
    fn resolveMemo(memo_spec: ui.Node) ui.Node {
        const build = memo_spec.element.memo_build orelse return memo_spec;
        const built = build.func(build.ctx);
        if (memo_spec.element.key == null or built.element.key != null) return built;
        // keep the memo key on the built root
        return ui.withKey(built, memo_spec.element.key.?);
    }

    /// Switches the active theme, invalidating every cached text layout.
    pub fn setTheme(self: *Runtime, theme: Theme) void {
        if (std.meta.eql(self.theme, theme)) return;
        self.theme = theme;
        self.tree.theme = theme;
        self.tree.selection_color = tokens.withAlpha(theme.color_primary, 0x4d);
        for (self.tree.nodes.items) |node| {
            switch (node.data) {
                .text => |*data| data.invalidate(),
                .button => |*data| data.invalidateLayout(),
                .input => |*data| {
                    data.invalidateLayout();
                    if (data.placeholder_layout) |*layout| layout.deinit();
                    data.placeholder_layout = null;
                },
                .toggle => |*data| data.invalidateLayout(),
                .select => |*data| {
                    if (data.layout) |*layout| layout.deinit();
                    data.layout = null;
                    data.layout_dirty = true;
                    for (data.option_layouts.items) |*layout| layout.deinit();
                    data.option_layouts.clearRetainingCapacity();
                },
                .rich_text => |*data| data.invalidateLayout(),
                else => {},
            }
        }
        self.tree.markLayoutDirty();
        self.tree.markPaintDirty();
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
        node.motion = element.motion;
        if (node.enabled != element.enabled) {
            layout_changed = true;
            node.enabled = element.enabled;
            if (!element.enabled) {
                switch (node.data) {
                    .button => |*data| {
                        data.hovered = false;
                        data.pressed = false;
                        data.hover = .at(0);
                        data.press = .at(0);
                        data.wave = .at(1);
                        data.hover_t = 0;
                        data.press_t = 0;
                        data.wave_t = 1;
                    },
                    .input => |*data| {
                        data.hovered = false;
                        data.focused = false;
                        data.blink_on = true;
                        data.hover = .at(0);
                        data.focus_anim = .at(0);
                        data.hover_t = 0;
                        data.focus_t = 0;
                    },
                    .toggle => |*data| {
                        data.hovered = false;
                        data.pressed = false;
                        data.hover = .at(0);
                        data.press = .at(0);
                        data.hover_t = 0;
                        data.press_t = 0;
                    },
                    .select => |*data| {
                        data.hovered = false;
                        data.hover = .at(0);
                        data.focus_anim = .at(0);
                        data.hover_t = 0;
                        data.focus_t = 0;
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
            .container => |*data| {
                // a pointer area appearing / disappearing changes hit testing
                if ((data.pointer == null) != (element.pointer == null)) paint_changed = true;
                data.pointer = element.pointer;
                data.key_action = element.key_action;
                data.cursor = element.cursor;
                data.resize_action = element.on_resize;
                if (element.pointer == null and self.pointer_capture == id) self.pointer_capture = null;
            },
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
                if (data.button_type != element.button_type or data.size != element.button_size or data.font_size != element.text_style.font_size) {
                    data.button_type = element.button_type;
                    data.size = element.button_size;
                    data.font_size = element.text_style.font_size;
                    data.invalidateLayout();
                    layout_changed = true;
                    paint_changed = true;
                }
                data.action = element.on_click;
                data.index_action = element.index_action;
                data.index = element.index;
                node.focusable = element.focusable;
                if (data.icon != element.icon or data.icon_size != element.icon_size or
                    !std.meta.eql(data.text_color, element.text_color) or data.text_align != element.text_style.text_align or
                    data.label_padding != element.label_padding)
                {
                    data.icon = element.icon;
                    data.icon_size = element.icon_size;
                    data.text_color = element.text_color;
                    data.text_align = element.text_style.text_align;
                    data.label_padding = element.label_padding;
                    data.invalidateLayout();
                    paint_changed = true;
                }
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
                if (data.font_size != element.text_style.font_size or data.line_height != element.text_style.line_height) {
                    data.font_size = element.text_style.font_size;
                    data.line_height = element.text_style.line_height;
                    data.invalidateLayout();
                    paint_changed = true;
                }
                data.selection_action = element.on_selection;
                data.key_action = element.on_key;
                if (applySelection(data, element.selection)) paint_changed = true;
                if (data.content_max_width != element.content_max_width) {
                    data.content_max_width = element.content_max_width;
                    data.invalidateLayout();
                    paint_changed = true;
                }
                data.text_action = element.on_text;
                data.submit_action = element.on_click;
                data.undo_action = element.on_undo;
                data.redo_action = element.on_redo;
                data.focus_ring = element.focus_ring;
                if (data.multiline != element.multiline) {
                    data.multiline = element.multiline;
                    data.invalidateLayout();
                    paint_changed = true;
                }
                node.focusable = true;
            },
            .vector => {
                if (!std.mem.eql(u8, node.data.vector.copy, element.content)) {
                    node.data.vector.copy = try node.data.vector.arena.allocator().dupe(u8, element.content);
                }
                if (element.drawing) |drawing| {
                    const before = node.data.vector.hash;
                    try self.tree.vectorSet(id, drawing);
                    if (before != node.data.vector.hash) {
                        layout_changed = true;
                        paint_changed = true;
                    }
                }
            },
            .rich_text => |*data| {
                if (!richSpansEqual(data.spans, element.rich_spans)) {
                    try self.tree.richTextSet(id, element.rich_spans);
                    layout_changed = true;
                    paint_changed = true;
                }
                if (data.line_height != element.rich_line_height) {
                    data.line_height = element.rich_line_height;
                    data.invalidateLayout();
                    layout_changed = true;
                    paint_changed = true;
                }
                data.link_action = element.on_link;
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
                if (element.image_id != 0) {
                    if (data.source_id != element.image_id) {
                        const old_w = data.intrinsic_width;
                        const old_h = data.intrinsic_height;
                        @import("components/image.zig").setSource(data, element.image_handle, element.image_id);
                        if (old_w != data.intrinsic_width or old_h != data.intrinsic_height) layout_changed = true;
                        paint_changed = true;
                    }
                } else if (data.source_id != 0 or !std.mem.eql(u8, data.path, element.content)) {
                    const owned = try self.tree.allocator.dupe(u8, element.content);
                    self.tree.allocator.free(data.path);
                    data.path = owned;
                    data.source_id = 0;
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
                    self.tree.poke(id);
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
            .rich_text => try self.tree.addRichText(null, element.rich_spans),
            .vector => try self.tree.addVector(null, element.drawing orelse return error.InvalidElement),
            else => return error.InvalidElement,
        };
        _ = self.applyCommon(id, element);
        const node = self.tree.get(id);
        switch (node.data) {
            .container => |*data| {
                data.pointer = element.pointer;
                data.key_action = element.key_action;
                data.cursor = element.cursor;
                data.resize_action = element.on_resize;
            },
            .button => |*data| {
                data.action = element.on_click;
                data.index_action = element.index_action;
                data.index = element.index;
                data.font_size = element.text_style.font_size;
                data.text_align = element.text_style.text_align;
                data.label_padding = element.label_padding;
                data.text_color = element.text_color;
                data.icon = element.icon;
                data.icon_size = element.icon_size;
                data.invalidateLayout();
            },
            .input => |*data| {
                data.text_action = element.on_text;
                data.submit_action = element.on_click;
                data.undo_action = element.on_undo;
                data.redo_action = element.on_redo;
                data.read_only = element.read_only;
                data.password = element.password;
                data.multiline = element.multiline;
                data.focus_ring = element.focus_ring;
                data.font_size = element.text_style.font_size;
                data.line_height = element.text_style.line_height;
                data.selection_action = element.on_selection;
                data.key_action = element.on_key;
                _ = applySelection(data, element.selection);
                data.content_max_width = element.content_max_width;
                input_mod.rebuildDisplay(self.tree.allocator, data);
            },
            .rich_text => |*data| {
                data.line_height = element.rich_line_height;
                data.link_action = element.on_link;
            },
            .toggle => |*data| {
                data.action = element.on_bool;
                data.index_action = element.index_action;
                data.index = element.index;
                data.font_size = element.text_style.font_size;
                data.checked = element.checked;
                const v: f32 = if (element.checked) 1 else 0;
                data.check = .at(v);
                data.mark = .at(v);
                data.check_t = v;
                data.mark_t = v;
            },
            .vector => |*data| {
                data.copy = data.arena.allocator().dupe(u8, element.content) catch "";
            },
            .image => |*data| {
                data.fit = element.image_fit;
                if (element.image_id != 0) @import("components/image.zig").setSource(data, element.image_handle, element.image_id);
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

    fn applySelection(data: *node_mod.InputData, selection: ?[2]usize) bool {
        const sel = selection orelse return false;
        if (data.applied_selection) |prev| {
            if (prev[0] == sel[0] and prev[1] == sel[1]) return false;
        }
        data.applied_selection = sel;
        const len = data.text.items.len;
        data.anchor = clampBoundary(data.text.items, @min(sel[0], len));
        data.cursor = clampBoundary(data.text.items, @min(sel[1], len));
        data.reported_selection = .{ data.anchor, data.cursor };
        data.blink_on = true;
        data.preferred_x = null;
        data.reveal_caret = true;
        return true;
    }

    fn clampBoundary(bytes: []const u8, index: usize) usize {
        var i = index;
        while (i > 0 and i < bytes.len and (bytes[i] & 0xC0) == 0x80) i -= 1;
        return i;
    }

    /// Notifies the focused input's `on_selection` when anchor/cursor moved.
    fn reportSelection(self: *Runtime) void {
        const id = self.tree.focused orelse return;
        const node = self.tree.get(id);
        switch (node.data) {
            .input => |*data| {
                const action = data.selection_action orelse return;
                if (data.reported_selection[0] == data.anchor and data.reported_selection[1] == data.cursor) return;
                data.reported_selection = .{ data.anchor, data.cursor };
                action.func(action.ctx, data.anchor, data.cursor);
                self.needs_build = true;
            },
            else => {},
        }
    }

    fn richSpansEqual(a: []const node_mod.RichSpan, b: []const node_mod.RichSpan) bool {
        if (a.len != b.len) return false;
        for (a, b) |x, y| {
            if (x.font_size != y.font_size or x.weight != y.weight or x.color != y.color or
                x.italic != y.italic or x.family != y.family or x.background != y.background or
                x.underline != y.underline or x.strike != y.strike or
                !std.mem.eql(u8, x.text, y.text) or !std.mem.eql(u8, x.link, y.link)) return false;
            if ((x.copy == null) != (y.copy == null)) return false;
            if (x.copy) |c| if (!std.mem.eql(u8, c, y.copy.?)) return false;
            if ((x.embed == null) != (y.embed == null)) return false;
            if (x.embed) |e| if (e.contentHash() != y.embed.?.contentHash()) return false;
        }
        return true;
    }

    fn selectOptionsEqual(data: *const node_mod.SelectData, options: []const []const u8) bool {
        if (data.options.items.len != options.len) return false;
        for (data.options.items, options) |old, new| {
            if (!std.mem.eql(u8, old.label, new)) return false;
        }
        return true;
    }

    fn paintSelectPopup(self: *Runtime, canvas: Canvas) void {
        if (self.open_select) |id| self.popup_select = id;
        const id = self.popup_select orelse return;
        const node = self.tree.get(id);
        if (!node.alive or std.meta.activeTag(node.data) != .select) {
            self.popup_select = null;
            return;
        }
        // painted until the closing transition has finished
        if (!node.data.select.open and node.data.select.open_t <= 0.003) {
            self.popup_select = null;
            return;
        }
        select_mod.paintPopup(node, canvas, self.theme, self.viewport);
    }

    fn closeSelect(self: *Runtime) void {
        if (self.open_select) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .select) {
                node.data.select.open = false;
                node.data.select.hovered_option = null;
                self.tree.poke(id);
            }
            self.open_select = null;
        }
    }

    /// Samples the clock for this frame / event.
    fn sampleClock(self: *Runtime) void {
        self.now_ms = self.clock();
        self.tree.now_ms = self.now_ms;
        ui.setNow(self.now_ms);
    }

    /// Advances one node's transitions; true while it keeps animating.
    fn animateNode(self: *Runtime, node: *Node) bool {
        const now = self.now_ms;
        var active = switch (node.data) {
            .container => |*data| blk: {
                if (!data.scroll_animating) break :blk false;
                const target = data.scroll_anim.value(now);
                _ = self.tree.scrollBy(node.id, target - data.scroll_offset);
                if (data.scroll_anim.done(now)) data.scroll_animating = false;
                break :blk data.scroll_animating;
            },
            .button => |*data| button_mod.animate(data, now, self.theme),
            .toggle => |*data| toggle_mod.animate(data, now, self.theme),
            .input => |*data| input_mod.animate(data, now, self.theme),
            .select => |*data| select_mod.animate(data, now, self.theme),
            else => false,
        };
        if (node.anim) |anim| {
            const kind = node.motion.presence;
            if (anim.presence.done(now)) {
                anim.fx = if (node.exiting) motion.presenceFx(kind, 0) else .{};
            } else {
                anim.fx = motion.presenceFx(kind, anim.presence.value(now));
                active = true;
            }
            if (anim.flip_active) {
                const k = anim.flip.value(now);
                for (0..4) |c| anim.flip_delta[c] = anim.flip_from[c] * k;
                if (anim.flip.done(now)) {
                    anim.flip_active = false;
                    anim.flip_delta = @splat(0);
                } else active = true;
            }
        }
        return active;
    }

    fn tick(self: *Runtime) void {
        // only nodes whose state changed (`Tree.poke`) are visited
        var finished: std.ArrayListUnmanaged(NodeId) = .empty;
        defer finished.deinit(self.allocator);
        var i: usize = 0;
        while (i < self.tree.animating.items.len) {
            const id = self.tree.animating.items[i];
            const node = self.tree.get(id);
            const active = node.alive and self.animateNode(node);
            if (node.alive) self.tree.markDirty(id, false, true);
            if (active) {
                i += 1;
            } else {
                node.anim_queued = false;
                _ = self.tree.animating.swapRemove(i);
                if (node.alive and node.exiting) finished.append(self.allocator, id) catch {};
            }
        }
        // exit transitions that ended: detach and free
        for (finished.items) |id| {
            const node = self.tree.get(id);
            if (!node.alive or !node.exiting) continue;
            if (node.parent) |pid| {
                const parent = self.tree.get(pid);
                if (std.mem.indexOfScalar(NodeId, parent.children.items, id)) |at| _ = parent.children.orderedRemove(at);
                self.tree.markDirty(pid, false, true);
            }
            self.removeSubtree(id);
        }
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (node.enabled and std.meta.activeTag(node.data) == .input) {
                if (input_mod.updateBlink(&node.data.input, node, self.now_ms)) self.tree.markDirty(id, false, true);
            }
        }
        if (self.tooltip_target != null and !self.tooltip_visible and self.now_ms >= self.tooltip_show_at) {
            self.showTooltip();
        }
        var tooltip_moving = false;
        if (self.tooltip_visible or self.tooltip_hiding) {
            if (self.tooltip_anim.done(self.now_ms)) {
                if (self.tooltip_hiding) self.dropTooltipLayout();
            } else tooltip_moving = true;
            self.tree.markPaintDirty();
        }
        self.any_animation = self.tree.animating.items.len > 0 or tooltip_moving;
    }

    /// Runs `ui.focus` / `ui.scrollIntoView` requests (after layout, so
    /// nodes created by the same update exist and have bounds).
    fn runCommands(self: *Runtime) void {
        var buf: [ui.max_commands]ui.Command = undefined;
        for (ui.takeCommands(&buf)) |*cmd| {
            const id = self.findKey(cmd.key()) orelse continue;
            switch (cmd.kind) {
                .focus => {
                    const node = self.tree.get(id);
                    if (node.focusable and node.enabled and node.inFlow()) self.focusNode(id);
                },
                .scroll_into_view => self.scrollIntoView(id),
                .scroll_to_end => {
                    const node = self.tree.get(id);
                    switch (node.data) {
                        .container => |*data| if (node.style.scroll) {
                            data.scroll_animating = false;
                            _ = self.tree.scrollBy(id, data.maxScroll(node.bounds.height) - data.scroll_offset);
                        },
                        else => {},
                    }
                },
            }
        }
    }

    fn findKey(self: *Runtime, key: []const u8) ?NodeId {
        return self.tree.idOfKey(key);
    }

    fn scrollIntoView(self: *Runtime, id: NodeId) void {
        const target = self.tree.get(id);
        var current = target.parent;
        while (current) |pid| : (current = self.tree.get(pid).parent) {
            const container = self.tree.get(pid);
            if (!container.style.scroll) continue;
            const data = switch (container.data) {
                .container => |*d| d,
                else => continue,
            };
            const max = data.maxScroll(container.bounds.height);
            const goal = std.math.clamp(data.scroll_offset + (target.bounds.y - container.bounds.y) - 12, 0, max);
            if (@abs(goal - data.scroll_offset) < 0.5) return;
            if (motion.enabled) {
                data.scroll_anim = motion.Animated.start(data.scroll_offset, goal, self.now_ms, .{ .duration_ms = 250, .easing = .ease_in_out });
                data.scroll_animating = true;
                self.tree.poke(pid);
            } else {
                _ = self.tree.scrollBy(pid, goal - data.scroll_offset);
            }
            return;
        }
    }

    fn computeNextDelay(self: *Runtime) void {
        var delay: c_int = -1;
        // transitions may also start after `tick` (FLIP in layout, scroll
        // commands), so the animating list counts too
        if (self.any_animation or self.tree.animating.items.len > 0) delay = 16;
        if (self.wake_at_ms) |at| {
            const wake: c_int = @intCast(std.math.clamp(at - self.now_ms, 1, 60_000));
            if (delay < 0 or wake < delay) delay = wake;
        }
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

    /// Calls `on_resize` of containers whose size changed in the last
    /// layout; true when any was called.
    fn notifyResizes(self: *Runtime) bool {
        // callbacks may read the new boxes with ui.keyBounds
        ui.setBuildTree(self.tree);
        defer ui.setBuildTree(null);
        var any = false;
        for (self.tree.nodes.items) |node| {
            if (!node.alive or !node.inFlow()) continue;
            const data = switch (node.data) {
                .container => |*d| d,
                else => continue,
            };
            const action = data.resize_action orelse continue;
            const b = node.bounds;
            if (@abs(b.width - data.reported_width) < 0.5 and @abs(b.height - data.reported_height) < 0.5) continue;
            data.reported_width = b.width;
            data.reported_height = b.height;
            action.func(action.ctx);
            any = true;
        }
        return any;
    }

    pub fn renderFrame(self: *Runtime, canvas: Canvas, width: f32, height: f32, dpr: f32) void {
        ui.setDisplayScale(dpr);
        const stats_start = if (self.frame_stats != null) platform.monotonicUs() else 0;
        defer if (self.frame_stats) |*stats| stats.frame(stats_start, platform.monotonicUs(), width, height);
        self.sampleClock();
        if (self.wake_at_ms) |at| {
            if (self.now_ms >= at) {
                self.wake_at_ms = null;
                self.needs_build = true;
            }
        }
        // a frame painted from inside an event callback (modal dialog loop)
        // shows the current tree as is; the rebuild waits for the event to end
        const nested = self.dispatch_depth > 0;
        if (self.needs_build and !nested) {
            const t = self.statsClock();
            self.rebuild();
            if (self.frame_stats) |*stats| stats.phase(.rebuild, t);
        }
        if (!nested) self.tick();
        self.viewport = .{ .width = width, .height = height };
        var resized = false;
        if (width != self.last_width or height != self.last_height) {
            self.last_width = width;
            self.last_height = height;
            self.tree.markLayoutDirty();
            resized = true;
        }
        if (self.tree.needsLayout()) {
            // a window resize moves things without a state change: no FLIP
            self.tree.flip_enabled = !resized;
            const t = self.statsClock();
            self.tree.layout(self.viewport);
            // size-dependent UI (on_resize): rebuild and lay out again now,
            // so the new size never shows a frame of the old layout
            if (!nested and self.notifyResizes()) {
                self.rebuild();
                if (self.tree.needsLayout()) self.tree.layout(self.viewport);
                _ = self.notifyResizes();
            }
            if (self.frame_stats) |*stats| stats.phase(.layout, t);
            self.tree.flip_enabled = true;
        }
        if (resized) if (self.frame_stats) |*stats| stats.resized(platform.monotonicUs());
        self.runCommands();
        const paint_start = self.statsClock();
        canvas.clear(self.clear_color);
        self.tree.paint(canvas);
        self.paintSelectPopup(canvas);
        self.paintTooltip(canvas);
        if (self.frame_stats) |*stats| stats.phase(.paint, paint_start);
        self.tree.clearPaint();
        self.computeNextDelay();
        if (self.layout_file) |path| self.writeLayoutFile(path);
    }

    fn statsClock(self: *const Runtime) i64 {
        return if (self.frame_stats != null) platform.monotonicUs() else 0;
    }

    fn writeLayoutFile(self: *Runtime, path: [:0]const u8) void {
        var out: std.ArrayList(u8) = .empty;
        defer out.deinit(self.allocator);
        for (self.tree.nodes.items) |node| {
            if (!node.alive or !node.inFlow()) continue;
            const key = node.key orelse continue;
            const b = node.bounds;
            out.print(self.allocator, "{s}\t{d:.1}\t{d:.1}\t{d:.1}\t{d:.1}\n", .{ key, b.x, b.y, b.width, b.height }) catch return;
        }
        @import("platform/dialog.zig").writeFile(path, out.items) catch {};
    }

    pub fn dispatch(self: *Runtime, event: events.Event) c_int {
        self.dispatch_depth += 1;
        defer self.dispatch_depth -= 1;
        self.sampleClock();
        const result = self.dispatchEvent(event);
        self.reportSelection();
        return switch (event) {
            // pointer / wheel events: non-zero only when something must be
            // redrawn, so plain mouse movement over static UI paints nothing
            .pointer_move, .pointer_down, .pointer_up, .pointer_enter, .pointer_leave, .scroll => @intFromBool(self.needs_build or self.tree.needsPaint() or self.tree.needsLayout()),
            else => result,
        };
    }

    fn dispatchEvent(self: *Runtime, event: events.Event) c_int {
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
            .pointer_leave => |p| {
                if (self.pointer_hover_area) |prev| {
                    self.pointer_hover_area = null;
                    self.sendPointer(prev, .leave, p, 0, 0);
                }
                self.setHover(null);
                self.drag_input = null;
                self.clearPressed();
                platform.setCursor(self.window, .arrow);
                return 1;
            },
            .key_down => |k| {
                if (self.config.key_fn) |key_fn| {
                    if (!self.focusedHasComposition() and key_fn(self.userdata, k)) {
                        self.needs_build = true;
                        return 1;
                    }
                }
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
                self.sel_dragging = false;
                self.releasePointerCapture(.{ .x = self.last_click_x, .y = self.last_click_y });
                if (self.drawsCaptionButtons()) self.needs_build = true;
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
                if (self.drawsCaptionButtons()) self.needs_build = true;
                return 0;
            },
            .window_resize => {
                if (self.frame_stats) |*stats| stats.resizeRequested(platform.monotonicUs());
                self.tree.markLayoutDirty();
                // maximize / restore glyph
                if (self.drawsCaptionButtons()) self.needs_build = true;
                return 0;
            },
            .window_close => return 0,
            .window_close_request => {
                const close_fn = self.config.close_fn orelse return 0;
                return @intFromBool(!close_fn(self.userdata));
            },
            .scroll => |scroll_event| {
                if (self.tree.wheelTargetAt(scroll_event.x, scroll_event.y)) |wid| {
                    if (self.pointerAreaNode(wid)) |area| if (area.capture_scroll) {
                        self.sendPointer(wid, .scroll, .{ .x = scroll_event.x, .y = scroll_event.y, .modifiers = scroll_event.modifiers }, scroll_event.dx, scroll_event.dy);
                        return 1;
                    };
                }
                const target = self.tree.hitTest(scroll_event.x, scroll_event.y);
                if (target) |id| {
                    const node = self.tree.get(id);
                    switch (node.data) {
                        .input => |*data| {
                            if (data.multiline) {
                                const view_h = @max(0, node.bounds.height - 16);
                                data.scroll_y = @max(0, @min(data.scroll_y - scroll_event.dy, input_mod.maxScrollY(data, view_h)));
                                self.tree.markDirty(id, false, true);
                                return 1;
                            }
                        },
                        else => {},
                    }
                }
                if (self.tree.scrollAt(scroll_event.x, scroll_event.y, scroll_event.dy)) {
                    self.updateCursor(self.tree.hitTest(scroll_event.x, scroll_event.y));
                    return 1;
                }
                return 0;
            },
        }
    }

    fn focusedHasComposition(self: *Runtime) bool {
        const id = self.tree.focused orelse return false;
        const node = self.tree.get(id);
        return switch (node.data) {
            .input => |data| data.has_composition,
            else => false,
        };
    }

    fn linkAt(self: *Runtime, id: NodeId, x: f32, y: f32) ?[]const u8 {
        const node = self.tree.get(id);
        switch (node.data) {
            .rich_text => |*data| {
                if (data.link_action == null) return null;
                const layout = data.ensureLayout(node.bounds.width) catch return null;
                const byte = layout.indexAt(x - node.bounds.x, y - node.bounds.y);
                // indexAt returns the nearest caret; probe the glyph on both sides.
                if (data.linkAtByte(byte)) |link| return link;
                if (byte > 0) return data.linkAtByte(byte - 1);
                return null;
            },
            else => return null,
        }
    }

    fn clearPressed(self: *Runtime) void {
        if (self.pressed_button) |id| {
            self.tree.get(id).data.button.pressed = false;
            self.tree.poke(id);
        }
        if (self.pressed_toggle) |id| {
            self.tree.get(id).data.toggle.pressed = false;
            self.tree.poke(id);
        }
        self.pressed_button = null;
        self.pressed_toggle = null;
    }

    // ---- text selection (BoxOptions.selectable) -----------------------

    pub fn clearTextSelection(self: *Runtime) void {
        for (self.sel_marked.items) |id| {
            if (id < self.tree.nodes.items.len) self.tree.get(id).text_sel = null;
        }
        if (self.sel_marked.items.len > 0) self.tree.markPaintDirty();
        self.sel_marked.clearRetainingCapacity();
        self.sel_root = null;
        self.sel_anchor = null;
        self.sel_focus = null;
        self.sel_dragging = false;
    }

    /// The selected text (empty when nothing is selected); caller frees.
    pub fn selectedText(self: *Runtime, allocator: std.mem.Allocator) ![]u8 {
        const root = self.sel_root orelse return allocator.dupe(u8, "");
        var nodes: std.ArrayListUnmanaged(NodeId) = .empty;
        defer nodes.deinit(self.allocator);
        try text_select.collect(self.tree, root, self.allocator, &nodes);
        return text_select.copyText(self.tree, nodes.items, self.sel_anchor orelse return allocator.dupe(u8, ""), self.sel_focus.?, allocator);
    }

    /// Marks the selected range on each text node (painted behind the text).
    fn applyTextSelection(self: *Runtime) void {
        for (self.sel_marked.items) |id| {
            if (id < self.tree.nodes.items.len) self.tree.get(id).text_sel = null;
        }
        self.sel_marked.clearRetainingCapacity();
        self.tree.markPaintDirty();
        const root = self.sel_root orelse return;
        const a = self.sel_anchor orelse return;
        const f = self.sel_focus orelse return;
        var nodes: std.ArrayListUnmanaged(NodeId) = .empty;
        defer nodes.deinit(self.allocator);
        text_select.collect(self.tree, root, self.allocator, &nodes) catch return;
        const range = text_select.ordered(nodes.items, a, f) orelse return;
        var i = range.si;
        while (i <= range.ei) : (i += 1) {
            const node = self.tree.get(nodes.items[i]);
            const from = if (i == range.si) range.start.byte else 0;
            const to = if (i == range.ei) range.end.byte else text_select.textLen(node);
            if (from >= to) continue;
            node.text_sel = .{ @intCast(from), @intCast(to) };
            self.sel_marked.append(self.allocator, node.id) catch return;
        }
    }

    fn unitAt(self: *Runtime, point: text_select.Point) [2]text_select.Point {
        const node = self.tree.get(point.node);
        return switch (self.sel_mode) {
            .char => .{ point, point },
            .word => blk: {
                const w = text_select.wordAt(node, point.byte, self.allocator);
                break :blk .{ .{ .node = point.node, .byte = w[0] }, .{ .node = point.node, .byte = w[1] } };
            },
            .block => .{ .{ .node = point.node, .byte = 0 }, .{ .node = point.node, .byte = text_select.textLen(node) } },
        };
    }

    /// Starts a text selection when the press lands in a `selectable`
    /// container on something that does not take clicks itself.
    fn beginTextSelection(self: *Runtime, target: ?NodeId, p: events.PointerEvent) bool {
        if (p.button != .left) return false;
        if (target) |id| {
            const node = self.tree.get(id);
            switch (node.data) {
                .rich_text => if (self.linkAt(id, p.x, p.y) != null) return false,
                .container => |*d| if (d.pointer != null) return false,
                else => return false,
            }
        }
        const root = text_select.rootAt(self.tree, p.x, p.y) orelse {
            if (target == null) self.clearTextSelection();
            return false;
        };
        // a blocking panel (modal, popover) over selectable text: only text
        // inside the panel itself starts a selection
        if (target) |id| if (!self.isAncestor(root, id) and !self.isAncestor(id, root)) return false;
        var nodes: std.ArrayListUnmanaged(NodeId) = .empty;
        defer nodes.deinit(self.allocator);
        text_select.collect(self.tree, root, self.allocator, &nodes) catch return false;
        const point = text_select.pointAt(self.tree, nodes.items, p.x, p.y) orelse return false;
        self.focusNode(null);
        self.countClick(p);
        if (p.modifiers.shift and self.sel_root == root and self.sel_anchor != null) {
            self.sel_focus = point;
        } else {
            self.sel_root = root;
            self.sel_mode = switch (self.click_count) {
                0, 1 => .char,
                2 => .word,
                else => .block,
            };
            self.sel_unit = self.unitAt(point);
            self.sel_anchor = self.sel_unit[0];
            self.sel_focus = self.sel_unit[1];
        }
        self.sel_dragging = true;
        self.applyTextSelection();
        return true;
    }

    fn isAncestor(self: *Runtime, ancestor: NodeId, id: NodeId) bool {
        var cur: ?NodeId = id;
        while (cur) |c| : (cur = self.tree.get(c).parent) if (c == ancestor) return true;
        return false;
    }

    fn dragTextSelection(self: *Runtime, p: events.PointerEvent) void {
        const root = self.sel_root orelse return;
        var nodes: std.ArrayListUnmanaged(NodeId) = .empty;
        defer nodes.deinit(self.allocator);
        text_select.collect(self.tree, root, self.allocator, &nodes) catch return;
        // auto-scroll the enclosing scroll container past its edges
        var cur = self.tree.get(root).parent;
        while (cur) |pid| : (cur = self.tree.get(pid).parent) {
            const c = self.tree.get(pid);
            if (!c.style.scroll) continue;
            if (p.y < c.bounds.y) _ = self.tree.scrollBy(pid, (p.y - c.bounds.y) * 0.5) else if (p.y > c.bounds.bottom()) _ = self.tree.scrollBy(pid, (p.y - c.bounds.bottom()) * 0.5);
            break;
        }
        const point = text_select.pointAt(self.tree, nodes.items, p.x, p.y) orelse return;
        if (self.sel_mode == .char) {
            self.sel_focus = point;
        } else {
            const unit = self.unitAt(point);
            const first = text_select.ordered(nodes.items, unit[0], self.sel_unit[0]) orelse return;
            if (first.start.node == unit[0].node and first.start.byte == unit[0].byte and
                !(unit[0].node == self.sel_unit[0].node and unit[0].byte == self.sel_unit[0].byte))
            {
                // dragging backwards: from the end of the first unit
                self.sel_anchor = self.sel_unit[1];
                self.sel_focus = unit[0];
            } else {
                self.sel_anchor = self.sel_unit[0];
                self.sel_focus = unit[1];
            }
        }
        self.applyTextSelection();
    }

    fn selectAllText(self: *Runtime, root: NodeId) void {
        var nodes: std.ArrayListUnmanaged(NodeId) = .empty;
        defer nodes.deinit(self.allocator);
        text_select.collect(self.tree, root, self.allocator, &nodes) catch return;
        if (nodes.items.len == 0) return;
        const last = nodes.items[nodes.items.len - 1];
        self.sel_root = root;
        self.sel_anchor = .{ .node = nodes.items[0], .byte = 0 };
        self.sel_focus = .{ .node = last, .byte = text_select.textLen(self.tree.get(last)) };
        self.applyTextSelection();
    }

    fn copyTextSelection(self: *Runtime) bool {
        const value = self.selectedText(self.allocator) catch return false;
        defer self.allocator.free(value);
        if (value.len == 0) return false;
        return platform.setClipboard(value);
    }

    /// A text node of a selectable container under (x, y) (I-beam cursor).
    fn selectableTextAt(self: *Runtime, x: f32, y: f32) bool {
        const root = text_select.rootAt(self.tree, x, y) orelse return false;
        return self.textNodeUnder(self.tree.get(root), x, y);
    }

    fn textNodeUnder(self: *Runtime, node: *Node, x: f32, y: f32) bool {
        if (!node.inFlow() or !node.bounds.contains(x, y)) return false;
        if (text_select.isText(node)) return true;
        for (node.children.items) |child| if (self.textNodeUnder(self.tree.get(child), x, y)) return true;
        return false;
    }

    fn pointerAreaNode(self: *Runtime, id: NodeId) ?node_mod.PointerArea {
        const node = self.tree.get(id);
        if (!node.alive) return null;
        return switch (node.data) {
            .container => |data| data.pointer,
            else => null,
        };
    }

    /// Calls a pointer area's handler with coordinates relative to it.
    fn sendPointer(self: *Runtime, id: NodeId, phase: actions.PointerPhase, p: events.PointerEvent, dx: f32, dy: f32) void {
        const area = self.pointerAreaNode(id) orelse return;
        const b = self.tree.get(id).bounds;
        const info: actions.PointerInfo = .{
            .phase = phase,
            .x = p.x - b.x,
            .y = p.y - b.y,
            .window_x = p.x,
            .window_y = p.y,
            .width = b.width,
            .height = b.height,
            .button = p.button,
            .modifiers = p.modifiers,
            .click_count = if (phase == .down or phase == .up) @max(1, self.click_count) else 1,
            .dx = dx,
            .dy = dy,
        };
        if (area.action.func(area.action.ctx, info)) self.needs_build = true;
    }

    /// Hover / leave for pointer areas under the pointer (no button held).
    fn updatePointerHover(self: *Runtime, target: ?NodeId, p: events.PointerEvent) void {
        const area: ?NodeId = if (target) |id| (if (self.pointerAreaNode(id) != null) id else null) else null;
        if (self.pointer_hover_area) |prev| {
            if (area == null or area.? != prev) {
                self.pointer_hover_area = null;
                self.sendPointer(prev, .leave, p, 0, 0);
            }
        }
        if (area) |id| {
            self.pointer_hover_area = id;
            self.sendPointer(id, .hover, p, 0, 0);
        }
    }

    /// Ends a drag that lost its pointer (window deactivated, area removed).
    fn releasePointerCapture(self: *Runtime, p: events.PointerEvent) void {
        const id = self.pointer_capture orelse return;
        self.pointer_capture = null;
        self.sendPointer(id, .up, p, 0, 0);
    }

    fn handlePointerMove(self: *Runtime, p: events.PointerEvent) void {
        if (self.sel_dragging) {
            self.dragTextSelection(p);
            platform.setCursor(self.window, .ibeam);
            return;
        }
        if (self.pointer_capture) |id| {
            // dragging: the capturing area gets every move, even outside it
            self.sendPointer(id, .drag, p, 0, 0);
            self.updateCursor(id);
            return;
        }
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
        self.updatePointerHover(target, p);
        if (self.pressed_button) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            if (node.data.button.pressed != inside) {
                node.data.button.pressed = inside;
                self.tree.poke(id);
            }
        }
        if (self.pressed_toggle) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            if (node.data.toggle.pressed != inside) {
                node.data.toggle.pressed = inside;
                self.tree.poke(id);
            }
        }
        if (self.drag_input) |id| self.setInputCursor(id, p.x, p.y, true);
        self.updateCursorAt(target, .{ p.x, p.y });
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
                .select => |*data| data.hovered = false,
                else => {},
            }
            self.tree.poke(previous);
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
                .select => |*data| data.hovered = node.enabled,
                else => {},
            }
            self.tree.poke(id);
        }
    }

    /// Hides the tooltip: a visible one zooms out (its text layout lives
    /// until the transition ends), a pending one is cancelled.
    fn clearTooltip(self: *Runtime) void {
        if (self.tooltip_visible) {
            self.tree.markPaintDirty();
            self.tooltip_anim.retarget(0, self.now_ms, motion.presenceExit(.zoom_big_fast));
            self.tooltip_hiding = true;
        } else if (!self.tooltip_hiding) {
            self.dropTooltipLayout();
        }
        self.tooltip_target = null;
        self.tooltip_visible = false;
        self.tooltip_show_at = 0;
    }

    fn dropTooltipLayout(self: *Runtime) void {
        if (self.tooltip_layout) |*layout| layout.deinit();
        self.tooltip_layout = null;
        self.tooltip_hiding = false;
        self.tooltip_anim = .{};
    }

    /// Lays out the pending tooltip once and starts its enter transition
    /// (centred below the target, flipped above near the window bottom).
    fn showTooltip(self: *Runtime) void {
        const id = self.tooltip_target orelse return;
        const node = self.tree.get(id);
        const content = node.tooltip orelse return;
        if (!node.inFlow()) return;
        // a new tooltip replaces one that is still zooming out
        self.dropTooltipLayout();
        self.tooltip_layout = text_mod.TextLayout.init(content, .{
            .font_size = 12,
            .color = 0xffffffff,
            .wrap = false,
        }, 1.0e6) catch return;
        const layout = &self.tooltip_layout.?;
        const w = layout.metrics.longest_line + 16;
        const h = layout.metrics.height + 8;
        var x = node.bounds.x + (node.bounds.width - w) * 0.5;
        var y = node.bounds.bottom() + 6;
        x = std.math.clamp(x, 8, @max(8, self.viewport.width - w - 8));
        self.tooltip_above = false;
        if (y + h > self.viewport.height - 8) {
            y = @max(8, node.bounds.y - h - 6);
            self.tooltip_above = true;
        }
        self.tooltip_rect = geometry.Rect.init(x, y, w, h);
        self.tooltip_anim = motion.Animated.start(0, 1, self.now_ms, motion.presenceEnter(.zoom_big_fast));
        self.tooltip_visible = true;
        self.tree.markPaintDirty();
    }

    fn paintTooltip(self: *Runtime, canvas: Canvas) void {
        if (!self.tooltip_visible and !self.tooltip_hiding) return;
        const layout = if (self.tooltip_layout) |*l| l else return;
        const rect = self.tooltip_rect;
        const fx = motion.presenceFx(.zoom_big_fast, self.tooltip_anim.value(self.now_ms));
        if (fx.opacity <= 0.003) return;
        const origin = motion.originPoint(if (self.tooltip_above) .bottom else .top, rect.x, rect.y, rect.width, rect.height);
        canvas.save();
        canvas.translate(origin[0], origin[1]);
        canvas.scale(fx.scale_x, fx.scale_y);
        canvas.translate(-origin[0], -origin[1]);
        const layer = fx.opacity < 0.997;
        if (layer) canvas.saveLayerAlpha(geometry.Rect.init(rect.x - 4, rect.y - 4, rect.width + 8, rect.height + 8), fx.opacity);
        // antd colorBgSpotlight
        canvas.fillRoundRect(rect, 6, 0xd9000000);
        layout.paint(canvas, rect.x + 8, rect.y + 4);
        if (layer) canvas.restore();
        canvas.restore();
    }

    fn updateCursor(self: *Runtime, target: ?NodeId) void {
        self.updateCursorAt(target, null);
    }

    fn updateCursorAt(self: *Runtime, target: ?NodeId, point: ?[2]f32) void {
        const cursor: platform.Cursor = if (target) |id| switch (self.tree.get(id).data) {
            .input => .ibeam,
            .button => |data| switch (data.button_type) {
                .plain, .caption, .caption_close => .arrow,
                else => .pointing_hand,
            },
            .toggle => .pointing_hand,
            .container => |data| data.cursor orelse .arrow,
            .rich_text => if (point) |p| (if (self.linkAt(id, p[0], p[1]) != null) .pointing_hand else .arrow) else .arrow,
            else => .arrow,
        } else if (point) |p| (if (self.selectableTextAt(p[0], p[1])) .ibeam else .arrow) else .arrow;
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
        if (self.beginTextSelection(target, p)) return;
        if (target) |id| {
            const node = self.tree.get(id);
            switch (node.data) {
                .button => {
                    if (node.focusable) self.focusNode(id);
                    if (!node.enabled) return;
                    node.data.button.pressed = true;
                    node.data.button.hovered = true;
                    self.pressed_button = id;
                    self.tree.poke(id);
                },
                .toggle => {
                    self.focusNode(id);
                    if (!node.enabled) return;
                    node.data.toggle.pressed = true;
                    node.data.toggle.hovered = true;
                    self.pressed_toggle = id;
                    self.tree.poke(id);
                },
                .input => {
                    if (!node.enabled) return;
                    self.focusNode(id);
                    self.drag_input = id;
                    self.setInputCursor(id, p.x, p.y, p.modifiers.shift);
                },
                .rich_text => {
                    if (self.linkAt(id, p.x, p.y) != null) self.pressed_link = id;
                },
                .select => {
                    if (!node.enabled) return;
                    self.focusNode(id);
                    node.data.select.open = true;
                    node.data.select.hovered_option = null;
                    self.open_select = id;
                    self.tree.poke(id);
                },
                .container => |*data| if (data.pointer != null and node.enabled) {
                    self.focusNode(if (node.focusable) id else null);
                    self.countClick(p);
                    self.pointer_capture = id;
                    self.sendPointer(id, .down, p, 0, 0);
                } else {
                    // blocking panels (block_pointer) still blur inputs
                    self.focusNode(null);
                },
                else => {},
            }
            self.updateCursor(target);
        } else {
            self.focusNode(null);
        }
    }

    fn countClick(self: *Runtime, p: events.PointerEvent) void {
        const near = @abs(p.x - self.last_click_x) <= 4 and @abs(p.y - self.last_click_y) <= 4;
        if (near and self.now_ms - self.last_click_ms <= 500 and self.click_count > 0) {
            self.click_count = if (self.click_count >= 3) 1 else self.click_count + 1;
        } else {
            self.click_count = 1;
        }
        self.last_click_ms = self.now_ms;
        self.last_click_x = p.x;
        self.last_click_y = p.y;
    }

    fn handlePointerUp(self: *Runtime, p: events.PointerEvent) void {
        self.drag_input = null;
        if (self.sel_dragging) {
            self.sel_dragging = false;
            // a plain click selects nothing
            if (self.sel_mode == .char) if (self.sel_anchor) |a| if (self.sel_focus) |f| {
                if (a.node == f.node and a.byte == f.byte) self.clearTextSelection();
            };
        }
        // a drag is not a click: the next press starts a new click count
        if (@abs(p.x - self.last_click_x) > 4 or @abs(p.y - self.last_click_y) > 4) self.click_count = 0;
        if (self.pointer_capture) |id| {
            self.pointer_capture = null;
            self.sendPointer(id, .up, p, 0, 0);
        }
        if (self.pressed_button) |id| {
            self.pressed_button = null;
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            node.data.button.pressed = false;
            self.tree.poke(id);
            if (inside and node.enabled) {
                button_mod.startWave(&node.data.button, self.now_ms);
                // copy the callbacks first: an action may change the UI (or
                // run a modal dialog), after which `node` must not be read
                const index_action = node.data.button.index_action;
                const index = node.data.button.index;
                const action = node.data.button.action;
                const on_click = node.data.button.on_click;
                if (index_action) |a| {
                    a.func(a.ctx, index);
                } else if (action) |a| {
                    a.func(a.ctx);
                }
                if (on_click) |callback| callback.func(callback.ctx, id);
            }
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
            self.tree.poke(id);
        }
        if (self.pressed_link) |id| {
            self.pressed_link = null;
            const node = self.tree.get(id);
            if (node.bounds.contains(p.x, p.y)) {
                if (self.linkAt(id, p.x, p.y)) |link| {
                    if (node.data.rich_text.link_action) |link_action| {
                        // The span memory may be replaced by the callback's rebuild;
                        // hand the callback a stable copy.
                        const copy = self.allocator.dupe(u8, link) catch return;
                        defer self.allocator.free(copy);
                        link_action.func(link_action.ctx, copy);
                    }
                }
            }
        }
        self.updateCursorAt(self.tree.hitTest(p.x, p.y), .{ p.x, p.y });
    }

    fn focusNode(self: *Runtime, id: ?NodeId) void {
        if (self.tree.focused) |old| {
            if (old != id) self.cancelCompositionFor(old);
        }
        self.tree.setFocus(id);
        self.closeSelectUnlessFocused();
    }

    /// Focus moved away from an open select: close its popup so it cannot
    /// swallow the next click.
    fn closeSelectUnlessFocused(self: *Runtime) void {
        const sid = self.open_select orelse return;
        if (self.tree.focused != sid) self.closeSelect();
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
        const origin_x = input_mod.textLeft(data, node) - data.scroll_x;
        const origin_y = if (data.multiline) node.bounds.y + 8 - data.scroll_y else node.bounds.y + (node.bounds.height - layout.metrics.height) * 0.5;
        var byte = layout.indexAt(x - origin_x, y - origin_y);
        byte = @min(byte, data.text.items.len);
        data.cursor = byte;
        if (!extend) data.anchor = byte;
        self.tree.markDirty(id, false, true);
    }

    fn handleKeyDown(self: *Runtime, k: events.KeyEvent) c_int {
        const shift = k.modifiers.shift;
        // focused pointer areas (canvases, viewers) see keys first
        if (self.tree.focused) |fid| {
            switch (self.tree.get(fid).data) {
                .container => |*data| if (data.key_action) |hook| {
                    if (hook.func(hook.ctx, k)) {
                        self.needs_build = true;
                        return 1;
                    }
                },
                else => {},
            }
        }
        // selected text: copy / clear / select all (unless an input is focused)
        const input_focused = if (self.tree.focused) |fid| std.meta.activeTag(self.tree.get(fid).data) == .input else false;
        if (!input_focused) {
            const primary = @import("core/keys.zig").primary(k.modifiers);
            if (self.sel_anchor != null) {
                if (primary and events.characterEquals(k.key, 'c')) {
                    _ = self.copyTextSelection();
                    return 1;
                }
                if (k.key == .escape) {
                    self.clearTextSelection();
                    return 1;
                }
            }
            if (primary and events.characterEquals(k.key, 'a')) {
                if (self.sel_root) |root| {
                    self.selectAllText(root);
                    return 1;
                }
            }
        }
        // input key hooks see every key first (Tab, Enter, shortcuts)
        if (self.tree.focused) |fid| {
            const fnode = self.tree.get(fid);
            switch (fnode.data) {
                .input => |*data| if (!data.has_composition) {
                    if (data.key_action) |hook| {
                        if (hook.func(hook.ctx, k, data.text.items, data.anchor, data.cursor)) {
                            self.needs_build = true;
                            return 1;
                        }
                    }
                },
                else => {},
            }
        }
        switch (k.key) {
            .tab => {
                self.cancelFocusedComposition();
                self.tree.tabFocus(shift);
                self.closeSelectUnlessFocused();
                return 1;
            },
            .escape => {
                if (self.open_select != null) {
                    self.closeSelect();
                    return 1;
                }
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
                        button_mod.startWave(data, self.now_ms);
                        self.tree.poke(id);
                        // copy first: `data` must not be read after an action
                        const index_action = data.index_action;
                        const index = data.index;
                        const action = data.action;
                        const on_click = data.on_click;
                        if (index_action) |a| {
                            a.func(a.ctx, index);
                        } else if (action) |a| {
                            a.func(a.ctx);
                        }
                        if (on_click) |callback| callback.func(callback.ctx, id);
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
                            self.tree.poke(id);
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
                        if (events.characterEquals(k.key, 'z')) {
                            if (k.modifiers.shift) {
                                if (data.redo_action) |action| {
                                    action.func(action.ctx);
                                    self.needs_build = true;
                                    return 1;
                                }
                            } else if (data.undo_action) |action| {
                                action.func(action.ctx);
                                self.needs_build = true;
                                return 1;
                            }
                        }
                        if (events.characterEquals(k.key, 'y')) {
                            if (data.redo_action) |action| {
                                action.func(action.ctx);
                                self.needs_build = true;
                                return 1;
                            }
                        }
                        // macOS text navigation: ⌘← / ⌘→ line edges, ⌘↑ / ⌘↓ document edges
                        if (k.modifiers.meta) switch (k.key) {
                            .left => {
                                self.tree.inputMove(id, .line_start, shift);
                                return 1;
                            },
                            .right => {
                                self.tree.inputMove(id, .line_end, shift);
                                return 1;
                            },
                            .up => if (data.multiline) {
                                self.tree.inputMove(id, .doc_start, shift);
                                return 1;
                            },
                            .down => if (data.multiline) {
                                self.tree.inputMove(id, .doc_end, shift);
                                return 1;
                            },
                            else => {},
                        };
                        // Windows text navigation: Ctrl+← / Ctrl+→ words,
                        // Ctrl+Home / Ctrl+End document edges, Ctrl+Backspace /
                        // Ctrl+Delete delete a word
                        if (platform.is_windows and k.modifiers.control and !k.modifiers.alt) switch (k.key) {
                            .left => {
                                self.tree.inputMove(id, .word_left, shift);
                                return 1;
                            },
                            .right => {
                                self.tree.inputMove(id, .word_right, shift);
                                return 1;
                            },
                            .home => {
                                self.tree.inputMove(id, if (data.multiline) .doc_start else .home, shift);
                                return 1;
                            },
                            .end => {
                                self.tree.inputMove(id, if (data.multiline) .doc_end else .end, shift);
                                return 1;
                            },
                            .backspace => {
                                self.tree.inputMove(id, .word_left, true);
                                _ = self.tree.inputDeleteBackward(id);
                                return 1;
                            },
                            .delete => {
                                self.tree.inputMove(id, .word_right, true);
                                _ = self.tree.inputDeleteForward(id);
                                return 1;
                            },
                            else => {},
                        };
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
                            self.tree.inputMove(id, if (data.multiline) .line_start else .home, shift);
                            return 1;
                        },
                        .end => {
                            self.tree.inputMove(id, if (data.multiline) .line_end else .end, shift);
                            return 1;
                        },
                        .up => {
                            if (!data.multiline) return 0;
                            self.tree.inputMove(id, .up, shift);
                            return 1;
                        },
                        .down => {
                            if (!data.multiline) return 0;
                            self.tree.inputMove(id, .down, shift);
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

const monotonicMs = platform.monotonicMs;

/// Frame timing for ZANT_FRAME_STATS (microsecond clock, printed per second):
/// frames, slowest frame and its phases (rebuild / layout / paint, the time
/// spent in Zig and in recording Skia draw calls; GPU work is not included),
/// the longest gap between frames, and for window resizes how many size
/// changes were drawn and the longest delay from a resize to its frame.
const FrameStats = struct {
    const Phase = enum { rebuild, layout, paint };

    window_start: i64 = 0,
    last_end: i64 = 0,
    frames: u32 = 0,
    max_frame_us: i64 = 0,
    max_gap_us: i64 = 0,
    max_phase_us: [3]i64 = .{ 0, 0, 0 },
    resize_frames: u32 = 0,
    pending_resize: i64 = 0,
    max_resize_latency_us: i64 = 0,
    /// phases of the frame being drawn (for the slow-frame line)
    cur_phase_us: [3]i64 = .{ 0, 0, 0 },

    fn phase(self: *FrameStats, which: Phase, start: i64) void {
        const i = @backingInt(which);
        const us = platform.monotonicUs() - start;
        self.cur_phase_us[i] = us;
        self.max_phase_us[i] = @max(self.max_phase_us[i], us);
    }

    fn resizeRequested(self: *FrameStats, now: i64) void {
        if (self.pending_resize == 0) self.pending_resize = now;
    }

    fn resized(self: *FrameStats, now: i64) void {
        self.resize_frames += 1;
        if (self.pending_resize != 0) {
            self.max_resize_latency_us = @max(self.max_resize_latency_us, now - self.pending_resize);
            self.pending_resize = 0;
        }
    }

    fn ms(us: i64) f64 {
        return @as(f64, @floatFromInt(us)) / 1000.0;
    }

    fn frame(self: *FrameStats, start: i64, end: i64, width: f32, height: f32) void {
        if (self.window_start == 0) self.window_start = start;
        if (self.last_end != 0) self.max_gap_us = @max(self.max_gap_us, start - self.last_end);
        self.last_end = end;
        self.frames += 1;
        self.max_frame_us = @max(self.max_frame_us, end - start);
        // frames over half the 60 Hz budget, one line each
        const text = @import("renderer/text.zig").layout_stats.take();
        if (end - start > 8_000) {
            std.debug.print("[slow-frame] ms={d:.2} rebuild={d:.2} layout={d:.2} paint={d:.2} text_layouts={d} text_ms={d:.2}\n", .{ ms(end - start), ms(self.cur_phase_us[0]), ms(self.cur_phase_us[1]), ms(self.cur_phase_us[2]), text.count, ms(text.us) });
        }
        self.cur_phase_us = .{ 0, 0, 0 };
        const elapsed = end - self.window_start;
        if (elapsed < 1_000_000) return;
        const fps = @as(f64, @floatFromInt(self.frames)) * 1_000_000.0 / @as(f64, @floatFromInt(elapsed));
        std.debug.print("[frames] fps={d:.1} max_frame_ms={d:.2} (rebuild={d:.2} layout={d:.2} paint={d:.2}) max_gap_ms={d:.2} resize_frames={d} max_resize_latency_ms={d:.2} size={d:.0}x{d:.0}\n", .{
            fps,
            ms(self.max_frame_us),
            ms(self.max_phase_us[0]),
            ms(self.max_phase_us[1]),
            ms(self.max_phase_us[2]),
            ms(self.max_gap_us),
            self.resize_frames,
            ms(self.max_resize_latency_us),
            width,
            height,
        });
        self.* = .{ .last_end = end, .pending_resize = self.pending_resize };
    }
};

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

fn titlebarHitCallback(userdata: ?*anyopaque, x: f32, y: f32) callconv(.c) c_int {
    const self: *Runtime = @ptrCast(@alignCast(userdata orelse return platform.Hit.caption));
    return self.titlebarHit(x, y);
}

fn wakeCallback(userdata: ?*anyopaque) callconv(.c) void {
    const self: *Runtime = @ptrCast(@alignCast(userdata orelse return));
    self.needs_build = true;
}

fn makeHost(runtime: *Runtime) platform.Host {
    return .{
        .wake = wakeCallback,
        .userdata = runtime,
        .draw = drawCallback,
        .event = eventCallback,
        .ime_rect = imeRectCallback,
        .next_frame_delay = nextFrameDelayCallback,
        .titlebar_hit = titlebarHitCallback,
    };
}
