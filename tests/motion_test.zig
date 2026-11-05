//! Motion system: easing curves, time-stamped tweens, presence, FLIP.
const std = @import("std");
const zant = @import("zant");
const motion = zant.motion;
const Easing = motion.Easing;
const Animated = motion.Animated;

test "every easing curve maps 0 to 0 and 1 to 1" {
    for (std.enums.values(Easing)) |e| {
        try std.testing.expectApproxEqAbs(@as(f32, 0), motion.easing.apply(e, 0), 1e-4);
        try std.testing.expectApproxEqAbs(@as(f32, 1), motion.easing.apply(e, 1), 1e-4);
    }
    // antd ease-in-out is near-symmetric (y2 = 1, so slightly above 0.5)
    try std.testing.expectApproxEqAbs(@as(f32, 0.5), motion.easing.apply(.ease_in_out, 0.5), 0.02);
    // reference value: CSS `ease` at 50% ≈ 0.8024
    try std.testing.expectApproxEqAbs(@as(f32, 0.8024), motion.easing.apply(.ease, 0.5), 2e-3);
    // back-out overshoots
    var max: f32 = 0;
    var i: usize = 1;
    while (i < 100) : (i += 1) max = @max(max, motion.easing.apply(.ease_out_back, @as(f32, @floatFromInt(i)) / 100));
    try std.testing.expect(max > 1);
    // ease-out is ahead of linear early on
    try std.testing.expect(motion.easing.apply(.ease_out, 0.3) > 0.3);
}

test "animated values follow the clock" {
    var a = Animated.start(0, 10, 1000, .{ .duration_ms = 200, .easing = .linear });
    try std.testing.expectEqual(@as(f32, 0), a.value(1000));
    try std.testing.expectApproxEqAbs(@as(f32, 5), a.value(1100), 1e-3);
    try std.testing.expect(!a.done(1100));
    try std.testing.expectEqual(@as(f32, 10), a.value(1200));
    try std.testing.expect(a.done(1200));

    // delay holds the start value
    const d = Animated.start(0, 1, 0, .{ .duration_ms = 100, .delay_ms = 50, .easing = .linear });
    try std.testing.expectEqual(@as(f32, 0), d.value(40));
    try std.testing.expectApproxEqAbs(@as(f32, 0.5), d.value(100), 1e-3);
    try std.testing.expect(d.done(150));

    // retargeting mid-flight continues from the current value
    a = Animated.start(0, 1, 0, .{ .duration_ms = 200, .easing = .linear });
    const mid = a.value(100);
    a.retarget(0, 100, .{ .duration_ms = 200, .easing = .linear });
    try std.testing.expectApproxEqAbs(mid, a.value(100), 1e-3);
    try std.testing.expect(a.duration_ms < 200);
    try std.testing.expect(a.done(100 + 200));
    // retarget to the same target is a no-op
    const before = a;
    a.retarget(0, 150, .{});
    try std.testing.expectEqual(before.start_ms, a.start_ms);
}

test "disabled motion settles immediately" {
    motion.enabled = false;
    defer motion.enabled = true;
    const a = Animated.start(0, 1, 0, .{ .duration_ms = 300 });
    try std.testing.expectEqual(@as(f32, 1), a.value(0));
    try std.testing.expect(a.done(0));
}

test "presence transforms" {
    const hidden = motion.presenceFx(.slide_up, 0);
    try std.testing.expectEqual(@as(f32, 0), hidden.opacity);
    try std.testing.expectApproxEqAbs(@as(f32, 0.8), hidden.scale_y, 1e-4);
    try std.testing.expect(motion.presenceFx(.zoom_big, 1).isIdentity());
    try std.testing.expectApproxEqAbs(@as(f32, -8), motion.presenceFx(.move_up, 0).dy, 1e-4);
    try std.testing.expectEqual(motion.Origin.top_right, motion.presenceOrigin(.slide_up, .top_right));
    try std.testing.expectEqual(@as(u32, 0xff808080), motion.mixColor(0xff000000, 0xffffffff, 0.502));
}

// ---- runtime integration (fake clock) ----

const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;

var fake_now: i64 = 1000;
fn fakeClock() i64 {
    return fake_now;
}

const Harness = struct {
    runtime: *Runtime,
    renderer: Renderer,

    fn init(state: anytype, comptime render: fn (*anyopaque) ui.Node) !Harness {
        fake_now = 1000;
        const runtime = try Runtime.init(std.testing.allocator, state, render, .{ .window = .{ .width = 400, .height = 300 } });
        runtime.clock = fakeClock;
        return .{ .runtime = runtime, .renderer = try Renderer.createRaster(400, 300) };
    }

    fn deinit(self: *Harness) void {
        self.renderer.deinit();
        self.runtime.deinit();
    }

    fn frame(self: *Harness) void {
        const canvas = self.renderer.begin(1.0).?;
        self.runtime.renderFrame(canvas, 400, 300, 1.0);
        self.renderer.end();
    }

    fn find(self: *Harness, key: []const u8) *zant.node.Node {
        for (self.runtime.tree.nodes.items) |node| {
            if (!node.alive) continue;
            if (node.key) |k| if (std.mem.eql(u8, k, key)) return node;
        }
        @panic("key not found");
    }
};

const ButtonState = struct {
    fn render(_: *anyopaque) ui.Node {
        return ui.column(.{ .padding = ui.inset(20), .children = &.{ui.button(.{ .key = "b", .text = "Hover me" })} });
    }
};

test "button hover eases over time and then stops animating" {
    motion.enabled = true;
    var state = ButtonState{};
    var h = try Harness.init(&state, ButtonState.render);
    defer h.deinit();
    h.frame();
    const button = h.find("b");
    const center = zant.events.PointerEvent{ .x = button.bounds.x + 10, .y = button.bounds.y + 8 };
    _ = h.runtime.dispatch(.{ .pointer_move = center });
    h.frame();
    try std.testing.expectEqual(@as(f32, 0), button.data.button.hover_t);
    fake_now += 100;
    h.frame();
    const mid = button.data.button.hover_t;
    try std.testing.expect(mid > 0 and mid < 1);
    try std.testing.expectEqual(@as(c_int, 16), h.runtime.next_delay_ms);
    fake_now += 150;
    h.frame();
    try std.testing.expectEqual(@as(f32, 1), button.data.button.hover_t);
    try std.testing.expectEqual(@as(usize, 0), h.runtime.tree.animating.items.len);
    try std.testing.expectEqual(@as(c_int, -1), h.runtime.next_delay_ms);
}

test "with motion disabled state changes apply at once" {
    motion.enabled = false;
    defer motion.enabled = true;
    var state = ButtonState{};
    var h = try Harness.init(&state, ButtonState.render);
    defer h.deinit();
    h.frame();
    const button = h.find("b");
    _ = h.runtime.dispatch(.{ .pointer_move = .{ .x = button.bounds.x + 10, .y = button.bounds.y + 8 } });
    h.frame();
    try std.testing.expectEqual(@as(f32, 1), button.data.button.hover_t);
}

const PanelState = struct {
    show: bool = false,

    fn render(ctx: *anyopaque) ui.Node {
        const self: *PanelState = @ptrCast(@alignCast(ctx));
        const panel = ui.styled(ui.column(.{ .key = "panel", .width = 120, .height = 40, .children = &.{} }), .{
            .motion = .slide_up,
            .block_pointer = true,
            .background = 0xffeeeeee,
        });
        const a = ui.column(.{ .key = "a", .width = 50, .height = 20, .children = &.{} });
        const b = ui.column(.{ .key = "b", .width = 50, .height = 20, .children = &.{} });
        if (self.show) return ui.column(.{ .padding = ui.inset(10), .children = &.{ a, panel, b } });
        return ui.column(.{ .padding = ui.inset(10), .children = &.{ a, b } });
    }
};

fn findLive(h: *Harness, key: []const u8) ?*zant.node.Node {
    for (h.runtime.tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.key) |k| if (std.mem.eql(u8, k, key)) return node;
    }
    return null;
}

test "presence: enter, exit out of flow, free, revive" {
    motion.enabled = true;
    var state = PanelState{};
    var h = try Harness.init(&state, PanelState.render);
    defer h.deinit();
    h.frame();
    const b_y_without = h.find("b").bounds.y;

    // enter
    state.show = true;
    h.runtime.invalidate();
    h.frame();
    const panel = h.find("panel");
    const panel_id = panel.id;
    try std.testing.expectEqual(@as(f32, 0), panel.anim.?.fx.opacity);
    fake_now += 100;
    h.frame();
    try std.testing.expect(panel.anim.?.fx.opacity > 0 and panel.anim.?.fx.opacity < 1);
    fake_now += 150;
    h.frame();
    try std.testing.expect(panel.anim.?.fx.isIdentity());
    try std.testing.expect(h.find("b").bounds.y > b_y_without);

    // exit: still a child, out of flow and not hittable
    state.show = false;
    h.runtime.invalidate();
    h.frame();
    try std.testing.expect(panel.alive and panel.exiting);
    const parent = h.runtime.tree.get(panel.parent.?);
    try std.testing.expect(std.mem.indexOfScalar(u32, parent.children.items, panel_id) != null);
    const c = panel.bounds;
    try std.testing.expect(h.runtime.tree.hitTest(c.x + c.width / 2, c.y + c.height / 2) != panel_id);
    try std.testing.expectEqual(b_y_without, h.find("b").bounds.y);

    // revive mid-exit keeps the same node
    fake_now += 100;
    h.frame();
    state.show = true;
    h.runtime.invalidate();
    h.frame();
    try std.testing.expectEqual(panel_id, h.find("panel").id);
    try std.testing.expect(!panel.exiting);
    fake_now += 300;
    h.frame();
    try std.testing.expect(panel.anim.?.fx.isIdentity());

    // exit fully: freed after the transition
    state.show = false;
    h.runtime.invalidate();
    h.frame();
    fake_now += 250;
    h.frame();
    try std.testing.expect(findLive(&h, "panel") == null);
    try std.testing.expect(std.mem.indexOfScalar(u32, h.runtime.tree.free_ids.items, panel_id) != null);
    try std.testing.expectEqual(@as(usize, 0), h.runtime.tree.animating.items.len);
}

const SegState = struct {
    selected: usize = 0,

    fn render(ctx: *anyopaque) ui.Node {
        const self: *SegState = @ptrCast(@alignCast(ctx));
        return ui.column(.{ .padding = ui.inset(10), .align_items = .center, .grow = 1, .children = &.{ui.segmented(.{
            .key = "seg",
            .options = &.{ .{ .label = "One" }, .{ .label = "Two" }, .{ .label = "Three" } },
            .selected = self.selected,
        })} });
    }
};

test "segmented thumb slides with FLIP; resizing does not animate" {
    motion.enabled = true;
    var state = SegState{};
    var h = try Harness.init(&state, SegState.render);
    defer h.deinit();
    h.frame();
    const thumb = h.find("seg-thumb");
    try std.testing.expect(!thumb.anim.?.flip_active);
    const x0 = thumb.bounds.x;

    state.selected = 2;
    h.runtime.invalidate();
    h.frame();
    try std.testing.expect(thumb.bounds.x > x0);
    try std.testing.expect(thumb.anim.?.flip_active);
    // the next frame is scheduled even though the slide started in layout
    try std.testing.expectEqual(@as(c_int, 16), h.runtime.next_delay_ms);
    // painted at the old position on the first frame
    try std.testing.expectApproxEqAbs(x0, thumb.bounds.x + thumb.anim.?.flip_delta[0], 0.5);
    fake_now += 150;
    h.frame();
    const d = thumb.anim.?.flip_delta[0];
    try std.testing.expect(d < 0 and d > x0 - thumb.bounds.x);
    fake_now += 200;
    h.frame();
    try std.testing.expectEqual(@as(f32, 0), thumb.anim.?.flip_delta[0]);
    try std.testing.expect(!thumb.anim.?.flip_active);

    // a viewport change re-lays out without starting a transition
    const before_resize = thumb.bounds.x;
    const canvas = h.renderer.begin(1.0).?;
    h.runtime.renderFrame(canvas, 360, 300, 1.0);
    h.renderer.end();
    try std.testing.expect(thumb.bounds.x != before_resize);
    try std.testing.expect(!thumb.anim.?.flip_active);
}

const CmdState = struct {
    renders: usize = 0,
    wake: bool = false,

    fn render(ctx: *anyopaque) ui.Node {
        const self: *CmdState = @ptrCast(@alignCast(ctx));
        self.renders += 1;
        if (self.wake) ui.wakeAfter(100);
        const arena = ui.frameAllocator();
        const rows = arena.alloc(ui.Node, 30) catch unreachable;
        for (rows, 0..) |*row, i| row.* = ui.column(.{
            .key = if (i == 29) "far" else null,
            .height = 40,
            .children = &.{},
        });
        return ui.column(.{ .grow = 1, .children = &.{
            ui.input(.{ .key = "x", .value = "" }),
            ui.column(.{ .key = "list", .grow = 1, .scroll = true, .children = rows }),
        } });
    }
};

test "ui.focus and ui.scrollIntoView run after the next layout" {
    motion.enabled = true;
    var state = CmdState{};
    var h = try Harness.init(&state, CmdState.render);
    defer h.deinit();
    h.frame();
    ui.focus("x");
    h.frame();
    try std.testing.expectEqual(h.find("x").id, h.runtime.tree.focused.?);

    ui.scrollIntoView("far");
    h.frame();
    const list = h.find("list");
    try std.testing.expectEqual(@as(c_int, 16), h.runtime.next_delay_ms);
    fake_now += 300;
    h.frame();
    try std.testing.expect(list.data.container.scroll_offset > 0);
    const far = h.find("far");
    try std.testing.expect(far.bounds.y < list.bounds.y + list.bounds.height);
}

test "ui.wakeAfter rebuilds once the time has passed" {
    var state = CmdState{ .wake = true };
    var h = try Harness.init(&state, CmdState.render);
    defer h.deinit();
    h.frame();
    const n = state.renders;
    try std.testing.expect(h.runtime.next_delay_ms > 0 and h.runtime.next_delay_ms <= 100);
    fake_now += 50;
    h.frame();
    try std.testing.expectEqual(n, state.renders);
    fake_now += 60;
    h.frame();
    try std.testing.expectEqual(n + 1, state.renders);
}
