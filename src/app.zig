const std = @import("std");
const builtin = @import("builtin");
const platform = @import("platform/platform.zig");
const Canvas = @import("renderer/canvas.zig").Canvas;
const events = @import("core/events.zig");
const tree_mod = @import("core/tree.zig");
const node_mod = @import("core/node.zig");
const input_mod = @import("components/input.zig");
const tokens = @import("theme/tokens.zig");
const geometry = @import("core/geometry.zig");
const gallery = @import("gallery.zig");

const Tree = tree_mod.Tree;
const NodeId = node_mod.NodeId;
const Event = events.Event;
const Key = events.Key;

fn monotonicMs() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.MONOTONIC, &ts);
    return @as(i64, @intCast(ts.sec)) * 1000 + @divTrunc(@as(i64, @intCast(ts.nsec)), 1_000_000);
}

pub const App = struct {
    allocator: std.mem.Allocator,
    theme: tokens.Theme,
    tree: *Tree,
    gallery: gallery.GalleryState,
    window: ?*platform.MacWindow = null,
    viewport: geometry.Size = .{},
    last_width: f32 = 0,
    last_height: f32 = 0,
    last_tick_ms: i64 = 0,
    now_ms: i64 = 0,
    next_delay_ms: c_int = -1,
    any_animation: bool = false,
    pressed_button: ?NodeId = null,
    drag_input: ?NodeId = null,
    hovered: ?NodeId = null,
    active: bool = true,

    pub fn create(allocator: std.mem.Allocator) !*App {
        const self = try allocator.create(App);
        errdefer allocator.destroy(self);
        const theme = tokens.antLight();
        const tree = try allocator.create(Tree);
        errdefer allocator.destroy(tree);
        tree.* = try Tree.init(allocator, theme);
        errdefer tree.deinit();
        self.* = .{
            .allocator = allocator,
            .theme = theme,
            .tree = tree,
            .gallery = undefined,
            .last_tick_ms = monotonicMs(),
        };
        try self.gallery.build(allocator, tree, theme);
        self.tree.markLayoutDirty();
        return self;
    }

    pub fn destroy(self: *App) void {
        self.tree.deinit();
        self.allocator.destroy(self.tree);
        self.allocator.destroy(self);
    }

    pub fn draw(self: *App, canvas: Canvas, width: f32, height: f32, dpr: f32) void {
        _ = dpr;
        self.tick();
        self.viewport = .{ .width = width, .height = height };
        if (width != self.last_width or height != self.last_height) {
            self.last_width = width;
            self.last_height = height;
            self.tree.markLayoutDirty();
        }
        if (self.tree.needsLayout()) {
            self.tree.layout(self.viewport);
        }
        // A draw callback already implies that a frame is needed. GPU backends
        // hand us a fresh drawable each time, so the tree must be replayed even
        // when no new invalidation arrived since the previous frame.
        canvas.clear(self.theme.color_bg_base);
        self.tree.paint(canvas);
        self.tree.clearPaint();
        self.computeNextDelay();
    }

    fn tick(self: *App) void {
        self.now_ms = monotonicMs();
        var dt: f32 = @floatFromInt(@max(0, self.now_ms - self.last_tick_ms));
        dt = @min(dt, 50);
        self.last_tick_ms = self.now_ms;
        self.any_animation = false;

        for (self.tree.nodes.items) |node| {
            switch (node.data) {
                .button => |*data| {
                    if (inputEnabled(node)) {
                        if (buttonAnimate(data, dt, self.theme, node)) {
                            self.any_animation = true;
                            self.tree.markDirty(node.id, false, true);
                        }
                    }
                },
                .input => |*data| {
                    if (self.tree.focused == node.id and node.enabled) {
                        if (input_mod.updateBlink(data, node, self.now_ms)) {
                            self.tree.markDirty(node.id, false, true);
                        }
                    }
                },
                else => {},
            }
        }
    }

    fn buttonAnimate(data: *node_mod.ButtonData, dt: f32, theme: tokens.Theme, node: *node_mod.Node) bool {
        _ = node;
        _ = dt;
        return @import("components/button.zig").animate(data, monotonicMs(), theme);
    }

    fn computeNextDelay(self: *App) void {
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
        self.next_delay_ms = delay;
    }

    pub fn handle(self: *App, event: Event) c_int {
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
                return 1;
            },
            .pointer_up => |p| {
                self.handlePointerUp(p);
                return 1;
            },
            .pointer_leave => {
                self.setHover(null);
                self.drag_input = null;
                if (self.pressed_button) |id| {
                    self.tree.get(id).data.button.pressed = false;
                    self.pressed_button = null;
                    self.tree.markPaintDirty();
                }
                platform.setCursor(self.window, .arrow);
                return 1;
            },
            .key_down => |k| return self.handleKeyDown(k),
            .key_up => return 0,
            .text_input => |value| return self.handleTextInput(value),
            .composition => |composition| return self.handleComposition(composition),
            .window_blur => {
                self.active = false;
                self.cancelFocusedComposition();
                self.tree.setFocus(null);
                self.setHover(null);
                if (self.pressed_button) |id| {
                    self.tree.get(id).data.button.pressed = false;
                    self.pressed_button = null;
                }
                self.tree.markPaintDirty();
                return 0;
            },
            .window_focus => {
                self.active = true;
                self.tree.markPaintDirty();
                return 0;
            },
            .window_resize => {
                self.tree.markLayoutDirty();
                return 0;
            },
            .window_close, .window_close_request => return 0,
            .scroll => return 0,
        }
    }

    fn handlePointerMove(self: *App, p: events.PointerEvent) void {
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
        if (self.drag_input) |id| {
            self.setInputCursor(id, p.x, p.y, true);
        }
        self.updateCursor(target);
    }

    fn setHover(self: *App, target: ?NodeId) void {
        if (self.hovered == target) return;
        if (self.hovered) |previous| {
            const node = self.tree.get(previous);
            switch (node.data) {
                .button => |*data| data.hovered = false,
                .input => |*data| data.hovered = false,
                else => {},
            }
            self.tree.markDirty(previous, false, true);
        }
        self.hovered = target;
        if (target) |id| {
            const node = self.tree.get(id);
            switch (node.data) {
                .button => |*data| data.hovered = node.enabled,
                .input => |*data| data.hovered = node.enabled,
                else => {},
            }
            self.tree.markDirty(id, false, true);
        }
        self.tree.markPaintDirty();
    }

    fn updateCursor(self: *App, target: ?NodeId) void {
        const cursor: platform.Cursor = if (target) |id| switch (self.tree.get(id).data) {
            .input => .ibeam,
            .button => .pointing_hand,
            else => .arrow,
        } else .arrow;
        platform.setCursor(self.window, cursor);
    }

    fn handlePointerDown(self: *App, p: events.PointerEvent) void {
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
                .input => {
                    if (!node.enabled) return;
                    self.focusNode(id);
                    self.drag_input = id;
                    self.setInputCursor(id, p.x, p.y, p.modifiers.shift);
                },
                else => {},
            }
            self.updateCursor(target);
        } else {
            self.focusNode(null);
            self.tree.markPaintDirty();
        }
    }

    fn handlePointerUp(self: *App, p: events.PointerEvent) void {
        self.drag_input = null;
        if (self.pressed_button) |id| {
            const node = self.tree.get(id);
            const inside = node.bounds.contains(p.x, p.y);
            node.data.button.pressed = false;
            if (inside and node.enabled) {
                if (node.data.button.on_click) |callback| {
                    callback.func(callback.ctx, id);
                }
            }
            self.pressed_button = null;
            self.tree.markDirty(id, false, true);
        }
        self.updateCursor(self.tree.hitTest(p.x, p.y));
    }

    fn focusNode(self: *App, id: ?NodeId) void {
        if (self.tree.focused) |old| {
            if (old != id) self.cancelCompositionFor(old);
        }
        self.tree.setFocus(id);
        self.tree.markPaintDirty();
    }

    fn cancelCompositionFor(self: *App, id: NodeId) void {
        const node = self.tree.get(id);
        if (std.meta.activeTag(node.data) != .input) return;
        if (node.data.input.has_composition) {
            self.tree.inputCancelComposition(id);
        }
    }

    fn cancelFocusedComposition(self: *App) void {
        if (self.tree.focused) |id| self.cancelCompositionFor(id);
    }

    fn setInputCursor(self: *App, id: NodeId, x: f32, y: f32, extend: bool) void {
        const node = self.tree.get(id);
        const data = &node.data.input;
        if (data.has_composition) return;
        const layout = input_mod.ensureLayout(data) catch return;
        const origin_x = node.bounds.x + input_mod.padding_x - data.scroll_x;
        const origin_y = node.bounds.y + (node.bounds.height - layout.metrics.height) * 0.5;
        var byte = layout.indexAt(x - origin_x, y - origin_y);
        byte = @min(byte, data.text.items.len);
        if (extend) {
            data.cursor = byte;
        } else {
            data.cursor = byte;
            data.anchor = byte;
        }
        self.tree.markDirty(id, false, true);
    }

    fn handleKeyDown(self: *App, k: events.KeyEvent) c_int {
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
                        if (data.on_click) |callback| callback.func(callback.ctx, id);
                        return 1;
                    }
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
                            if (self.copySelection(id)) {
                                _ = self.tree.inputDeleteBackward(id);
                            }
                            return 1;
                        }
                        if (events.characterEquals(k.key, 'v')) {
                            self.pasteInto(id);
                            return 1;
                        }
                        if (events.characterEquals(k.key, 'a') or events.characterEquals(k.key, 'A')) {
                            self.tree.inputSelectAll(id);
                            return 1;
                        }
                        return 0;
                    }
                    switch (k.key) {
                        .left => {
                            if (k.modifiers.alt) {
                                self.tree.inputMove(id, .word_left, shift);
                            } else {
                                self.tree.inputMove(id, .left, shift);
                            }
                            return 1;
                        },
                        .right => {
                            if (k.modifiers.alt) {
                                self.tree.inputMove(id, .word_right, shift);
                            } else {
                                self.tree.inputMove(id, .right, shift);
                            }
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
                            if (data.on_submit) |callback| callback.func(callback.ctx, id);
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

    fn handleTextInput(self: *App, value: []const u8) c_int {
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .input) {
                if (self.tree.inputInsert(id, value)) return 1;
            }
        }
        return 0;
    }

    fn handleComposition(self: *App, composition: events.Composition) c_int {
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .input) {
                if (composition.text.len == 0) {
                    self.tree.inputCancelComposition(id);
                } else {
                    self.tree.inputComposition(
                        id,
                        composition.text,
                        composition.selection_start,
                        composition.selection_end,
                    );
                }
                return 1;
            }
        }
        return 0;
    }

    fn copySelection(self: *App, id: NodeId) bool {
        const node = self.tree.get(id);
        const data = &node.data.input;
        if (!input_mod.hasSelection(data)) return false;
        const range = input_mod.selectionRange(data);
        if (range.start == range.end) return false;
        return platform.setClipboard(data.text.items[range.start..range.end]);
    }

    fn pasteInto(self: *App, id: NodeId) void {
        const clipboard = platform.clipboardAlloc(self.allocator) catch return;
        defer self.allocator.free(clipboard);
        if (clipboard.len == 0) return;
        _ = self.tree.inputInsert(id, clipboard);
    }

    pub fn imeRect(self: *App) ?geometry.Rect {
        if (self.tree.focused) |id| {
            const node = self.tree.get(id);
            if (std.meta.activeTag(node.data) == .input) {
                return self.tree.inputCaretRect(id);
            }
        }
        return null;
    }

    pub fn nextFrameDelay(self: *App) c_int {
        return self.next_delay_ms;
    }
};

fn inputEnabled(node: *const node_mod.Node) bool {
    return node.enabled;
}

pub fn drawCallback(userdata: ?*anyopaque, canvas_ptr: ?*anyopaque, width: f32, height: f32, dpr: f32) callconv(.c) void {
    const self: *App = @ptrCast(@alignCast(userdata orelse return));
    self.draw(Canvas.fromOpaque(canvas_ptr), width, height, dpr);
}

pub fn eventCallback(userdata: ?*anyopaque, raw: *const platform.Event) callconv(.c) c_int {
    const self: *App = @ptrCast(@alignCast(userdata orelse return 0));
    const event = events.translate(raw);
    return self.handle(event);
}

pub fn imeRectCallback(userdata: ?*anyopaque, x: *f32, y: *f32, w: *f32, h: *f32) callconv(.c) c_int {
    const self: *App = @ptrCast(@alignCast(userdata orelse return 0));
    if (self.imeRect()) |rect| {
        x.* = rect.x;
        y.* = rect.y;
        w.* = rect.width;
        h.* = rect.height;
        return 1;
    }
    return 0;
}

pub fn nextFrameDelayCallback(userdata: ?*anyopaque) callconv(.c) c_int {
    const self: *App = @ptrCast(@alignCast(userdata orelse return -1));
    return self.nextFrameDelay();
}

pub fn makeHost(app: *App) platform.Host {
    return .{
        .userdata = app,
        .draw = drawCallback,
        .event = eventCallback,
        .ime_rect = imeRectCallback,
        .next_frame_delay = nextFrameDelayCallback,
    };
}
