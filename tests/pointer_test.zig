//! ui.pointerArea: capture, local coordinates, multi-click, wheel capture,
//! keys; BoxOptions.on_resize; in-memory images.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;

const Log = struct {
    phases: [32]ui.PointerPhase = undefined,
    infos: [32]ui.PointerInfo = undefined,
    n: usize = 0,
    keys: usize = 0,
    resizes: usize = 0,
    wide: bool = false,

    fn onPointer(self: *Log, info: ui.PointerInfo) bool {
        if (self.n < self.phases.len) {
            self.phases[self.n] = info.phase;
            self.infos[self.n] = info;
            self.n += 1;
        }
        return info.phase != .hover;
    }

    fn onKey(self: *Log, k: zant.events.KeyEvent) bool {
        _ = k;
        self.keys += 1;
        return true;
    }

    fn onResize(self: *Log) void {
        self.resizes += 1;
        if (ui.keyBounds("area")) |b| self.wide = b.width > 300;
    }

    fn last(self: *const Log) ui.PointerInfo {
        return self.infos[self.n - 1];
    }

    fn renderOpaque(ctx: *anyopaque) ui.Node {
        return render(@ptrCast(@alignCast(ctx)));
    }

    fn render(self: *Log) ui.Node {
        return ui.column(.{ .grow = 1, .padding = ui.inset(20), .children = &.{
            ui.pointerArea(.{
                .box = .{ .key = "area", .width_percent = 100, .height = 100, .on_resize = ui.action(self, onResize) },
                .on_pointer = ui.pointerAction(self, onPointer),
                .capture_scroll = true,
                .cursor = .crosshair,
                .focusable = true,
                .on_key = ui.keyAction(self, onKey),
            }),
        } });
    }
};

fn frame(runtime: *Runtime, renderer: *zant.renderer.Renderer, w: f32, h: f32) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, w, h, 1.0);
    renderer.end();
}

test "pointer area: capture, local coordinates, clicks, wheel and keys" {
    var log = Log{};
    const runtime = try Runtime.init(std.testing.allocator, &log, Log.renderOpaque, .{ .window = .{ .width = 400, .height = 300 } });
    defer runtime.deinit();
    var renderer = try zant.renderer.Renderer.createRaster(400, 300);
    defer renderer.deinit();
    var now: i64 = 1000;
    const Clock = struct {
        var t: *i64 = undefined;
        fn get() i64 {
            return t.*;
        }
    };
    Clock.t = &now;
    runtime.clock = Clock.get;
    frame(runtime, &renderer, 400, 300);
    try std.testing.expectEqual(@as(usize, 1), log.resizes);
    try std.testing.expect(log.wide);

    // hover, then press inside and drag outside: the area keeps the pointer
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 30, .y = 40 } });
    try std.testing.expectEqual(ui.PointerPhase.hover, log.last().phase);
    try std.testing.expectApproxEqAbs(@as(f32, 10), log.last().x, 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 20), log.last().y, 0.01);
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 30, .y = 40 } });
    try std.testing.expectEqual(ui.PointerPhase.down, log.last().phase);
    try std.testing.expectEqual(@as(u8, 1), log.last().click_count);
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 390, .y = 290 } });
    try std.testing.expectEqual(ui.PointerPhase.drag, log.last().phase);
    try std.testing.expectApproxEqAbs(@as(f32, 370), log.last().x, 0.01);
    try std.testing.expectApproxEqAbs(@as(f32, 270), log.last().y, 0.01);
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 390, .y = 290 } });
    try std.testing.expectEqual(ui.PointerPhase.up, log.last().phase);
    // focused by the click: keys go to on_key
    _ = runtime.dispatch(.{ .key_down = .{ .key = .{ .character = 'x' } } });
    try std.testing.expectEqual(@as(usize, 1), log.keys);

    // double click (same spot, quickly)
    now += 1000;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 50, .y = 50 } });
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 50, .y = 50 } });
    now += 120;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 51, .y = 50 } });
    try std.testing.expectEqual(@as(u8, 2), log.last().click_count);
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 51, .y = 50 } });
    now += 800;
    _ = runtime.dispatch(.{ .pointer_down = .{ .x = 51, .y = 50 } });
    try std.testing.expectEqual(@as(u8, 1), log.last().click_count);
    _ = runtime.dispatch(.{ .pointer_up = .{ .x = 51, .y = 50 } });

    // the wheel goes to the area
    _ = runtime.dispatch(.{ .scroll = .{ .x = 60, .y = 60, .dx = 0, .dy = -12, .modifiers = .{} } });
    try std.testing.expectEqual(ui.PointerPhase.scroll, log.last().phase);
    try std.testing.expectApproxEqAbs(@as(f32, -12), log.last().dy, 0.01);

    // window resize: on_resize runs and the UI rebuilds in the same frame
    frame(runtime, &renderer, 200, 300);
    try std.testing.expectEqual(@as(usize, 2), log.resizes);
    try std.testing.expect(!log.wide);
}

test "image: in-memory pixels shown by ui.image" {
    var pixels: [4 * 4 * 4]u8 = undefined;
    @memset(&pixels, 0xff);
    const img = zant.image.Image.fromPixels(&pixels, 4, 4, 16) orelse return error.ImageFailed;
    defer img.deinit();
    try std.testing.expectEqual(@as(u32, 4), img.width);
    const copy = img.retain();
    copy.deinit();

    const State = struct {
        img: zant.image.Image,
        fn renderOpaque(ctx: *anyopaque) ui.Node {
            return render(@ptrCast(@alignCast(ctx)));
        }
        fn render(self: *@This()) ui.Node {
            return ui.column(.{ .children = &.{ui.image(.{ .key = "img", .image = self.img })} });
        }
    };
    var state = State{ .img = img };
    const runtime = try Runtime.init(std.testing.allocator, &state, State.renderOpaque, .{ .window = .{ .width = 100, .height = 100 } });
    defer runtime.deinit();
    runtime.rebuildForTest();
    var found = false;
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive) continue;
        if (node.key) |k| if (std.mem.eql(u8, k, "img")) {
            found = true;
            try std.testing.expectEqual(@as(c_int, 4), node.data.image.intrinsic_width);
        };
    }
    try std.testing.expect(found);
}

test "container hover look: no rebuild, follows the pointer, hidden under a modal" {
    const App = struct {
        builds: usize = 0,
        modal: bool = false,
        fn renderOpaque(ctx: *anyopaque) ui.Node {
            const self: *@This() = @ptrCast(@alignCast(ctx));
            self.builds += 1;
            const rows = ui.column(.{ .key = "rows", .align_items = .stretch, .children = &.{
                ui.row(.{ .key = "row-a", .height = 40, .hover_background = 0xff112233, .children = &.{ui.button(.{ .key = "btn-a", .text = "A" })} }),
                ui.row(.{ .key = "row-b", .height = 40, .hover_background = 0xff445566 }),
            } });
            if (!self.modal) return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{rows} });
            return ui.column(.{ .grow = 1, .align_items = .stretch, .children = &.{ rows, ui.modal(.{ .content = ui.text("x", .{}) }) } });
        }
    };
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, App.renderOpaque, .{ .window = .{ .width = 300, .height = 300 } });
    defer runtime.deinit();
    var r = try zant.renderer.Renderer.createRaster(300, 300);
    defer r.deinit();
    frame(runtime, &r, 300, 300);
    const builds = app.builds;
    const b = runtime.tree.boundsOfKey("row-b").?;
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 200, .y = b.y + 20 } });
    frame(runtime, &r, 300, 300);
    try std.testing.expectEqual(builds, app.builds);
    const row_b = runtime.tree.get(runtime.tree.idOfKey("row-b").?);
    try std.testing.expect(row_b.data.container.hovered);
    // over the button in row A: row A shows its hover (ancestor of the widget)
    const btn = runtime.tree.boundsOfKey("btn-a").?;
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = btn.x + 5, .y = btn.y + 5 } });
    frame(runtime, &r, 300, 300);
    try std.testing.expect(!row_b.data.container.hovered);
    try std.testing.expect(runtime.tree.get(runtime.tree.idOfKey("row-a").?).data.container.hovered);
    // a modal over the rows hides their hover
    app.modal = true;
    runtime.invalidate();
    frame(runtime, &r, 300, 300);
    _ = runtime.dispatch(.{ .pointer_move = .{ .x = 200, .y = b.y + 20 } });
    frame(runtime, &r, 300, 300);
    try std.testing.expect(!row_b.data.container.hovered);
}
