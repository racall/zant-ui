//! Node lifecycle: removed subtrees are freed and recycled, runtime
//! references to them are cleared, keyed children match strictly.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const events = zant.events;

const State = struct {
    show_panel: bool = true,
    panel_key: []const u8 = "menu",
    clicks: usize = 0,

    fn click(self: *State) void {
        self.clicks += 1;
    }

    fn render(context: *anyopaque) ui.Node {
        const self: *State = @ptrCast(@alignCast(context));
        if (!self.show_panel) return ui.column(.{ .padding = ui.inset(16), .children = &.{ui.text("empty", .{})} });
        return ui.column(.{
            .padding = ui.inset(16),
            .gap = 8,
            .children = &.{
                ui.text("header", .{}),
                ui.column(.{ .key = self.panel_key, .gap = 4, .children = &.{
                    ui.button(.{ .key = "action", .text = "Action", .on_click = ui.action(self, click) }),
                    ui.input(.{ .key = "field", .value = "abc" }),
                    ui.text("footer", .{}),
                } }),
            },
        });
    }
};

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 400, 300, 1.0);
    renderer.end();
}

fn findKey(tree: *zant.tree.Tree, key: []const u8) ?*zant.node.Node {
    for (tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.key) |k| if (std.mem.eql(u8, k, key)) return node;
    }
    return null;
}

fn center(node: *const zant.node.Node) events.PointerEvent {
    return .{ .x = node.bounds.x + node.bounds.width * 0.5, .y = node.bounds.y + node.bounds.height * 0.5 };
}

test "toggling a subtree recycles node slots" {
    var state = State{};
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(400, 300);
    defer renderer.deinit();

    frame(runtime, &renderer);
    var high_water: usize = 0;
    for (0..100) |_| {
        state.show_panel = !state.show_panel;
        runtime.invalidate();
        frame(runtime, &renderer);
        high_water = @max(high_water, runtime.tree.nodes.items.len);
    }
    // slots are reused: the slot count stops growing after the first toggles
    try std.testing.expect(runtime.tree.nodes.items.len <= high_water);
    try std.testing.expect(high_water < 20);
}

test "removed nodes leave no dangling runtime references" {
    var state = State{};
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(400, 300);
    defer renderer.deinit();
    frame(runtime, &renderer);

    const button = findKey(runtime.tree, "action").?;
    const field = findKey(runtime.tree, "field").?;
    const p = center(button);
    // focus the input, then hover and press the button
    _ = runtime.dispatch(.{ .pointer_down = center(field) });
    _ = runtime.dispatch(.{ .pointer_up = center(field) });
    try std.testing.expectEqual(field.id, runtime.tree.focused.?);
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    try std.testing.expect(runtime.pressed_button != null);

    state.show_panel = false;
    runtime.invalidate();
    frame(runtime, &renderer);
    try std.testing.expect(runtime.pressed_button == null);
    try std.testing.expect(runtime.hovered == null);
    try std.testing.expect(runtime.tree.focused == null);
    // releasing the mouse over where the button was must not fire or crash
    _ = runtime.dispatch(.{ .pointer_up = p });
    try std.testing.expectEqual(@as(usize, 0), state.clicks);
}

test "keyed children never take over a node with another key" {
    var state = State{};
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(400, 300);
    defer renderer.deinit();
    frame(runtime, &renderer);

    const menu_id = findKey(runtime.tree, "menu").?.id;
    state.panel_key = "outline";
    runtime.invalidate();
    frame(runtime, &renderer);
    const outline = findKey(runtime.tree, "outline").?;
    try std.testing.expect(findKey(runtime.tree, "menu") == null);
    // the old "menu" node was freed (its slot may be reused by a new node)
    const old_slot = runtime.tree.nodes.items[menu_id];
    try std.testing.expect(!old_slot.alive or old_slot.key == null or !std.mem.eql(u8, old_slot.key.?, "menu"));
    try std.testing.expect(outline.alive);
}

test "an unchanged rebuild does not relayout the tree" {
    var state = State{};
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(400, 300);
    defer renderer.deinit();
    frame(runtime, &renderer);
    runtime.invalidate();
    frame(runtime, &renderer);
    runtime.needs_build = true;
    runtime.rebuildForTest();
    try std.testing.expect(!runtime.tree.needsLayout());
}

test "close requests are vetoed by close_fn" {
    var state = State{};
    const Veto = struct {
        fn keepOpen(_: *anyopaque) bool {
            return false;
        }
    };
    const runtime = try Runtime.init(std.testing.allocator, &state, State.render, .{
        .window = .{ .width = 400, .height = 300 },
        .close_fn = Veto.keepOpen,
    });
    defer runtime.deinit();
    try std.testing.expectEqual(@as(c_int, 1), runtime.dispatch(.window_close_request));
    runtime.config.close_fn = null;
    try std.testing.expectEqual(@as(c_int, 0), runtime.dispatch(.window_close_request));
}

/// A click handler that runs a native modal loop (file dialog, confirm
/// alert) gets frames painted while it waits. Those frames must not rebuild
/// the tree under the dispatching code: here the rebuild would free the
/// clicked button and recycle its slot as a container.
const ModalState = struct {
    runtime: ?*Runtime = null,
    renderer: ?*Renderer = null,
    replaced: bool = false,

    fn openDialog(self: *ModalState) void {
        self.replaced = true;
        self.runtime.?.invalidate();
        // what the modal loop does: paint a frame before the action returns
        frame(self.runtime.?, self.renderer.?);
    }

    fn render(context: *anyopaque) ui.Node {
        const self: *ModalState = @ptrCast(@alignCast(context));
        if (self.replaced) return ui.column(.{ .padding = ui.inset(16), .children = &.{
            ui.column(.{ .key = "panel", .height = 40, .children = &.{} }),
            ui.column(.{ .key = "other", .height = 40, .children = &.{} }),
        } });
        return ui.column(.{ .padding = ui.inset(16), .children = &.{
            ui.button(.{ .key = "open", .text = "Open", .on_click = ui.action(self, openDialog) }),
        } });
    }
};

test "a frame painted inside a click handler does not rebuild under it" {
    var state = ModalState{};
    const runtime = try Runtime.init(std.testing.allocator, &state, ModalState.render, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(400, 300);
    defer renderer.deinit();
    state.runtime = runtime;
    state.renderer = &renderer;

    frame(runtime, &renderer);
    const p = center(findKey(runtime.tree, "open").?);
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    try std.testing.expect(state.replaced);
    // the deferred rebuild runs on the next normal frame
    frame(runtime, &renderer);
    try std.testing.expect(findKey(runtime.tree, "open") == null);
    try std.testing.expect(findKey(runtime.tree, "panel") != null);
}
