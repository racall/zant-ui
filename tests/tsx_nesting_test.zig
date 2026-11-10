//! Nesting end to end (`tools/tsx/testdata/nesting.tsx`): nested spans and an
//! inline link become one rich text, an icon in a button label, the
//! innermost clickable wins, a modal written in a row covers the window.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const gen = @import("tsx_golden_nesting");

const W = 560;
const H = 640;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, W, H, 1.0);
    renderer.end();
}

fn clickAt(runtime: *Runtime, renderer: *Renderer, x: f32, y: f32) void {
    const p = zant.events.PointerEvent{ .x = x, .y = y };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, renderer);
}

fn click(runtime: *Runtime, renderer: *Renderer, key: []const u8) !void {
    frame(runtime, renderer);
    const r = runtime.tree.boundsOfKey(key) orelse return error.MissingKey;
    clickAt(runtime, renderer, r.x + r.width * 0.5, r.y + r.height * 0.5);
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *gen.App = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx nesting: rich text, icon labels, inner clicks, portals" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = gen.App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{ .window = .{ .width = W, .height = H } });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    // one rich text with seven spans; link span 0 runs the Link's onClick
    const intro = runtime.tree.get(runtime.tree.idOfKey("intro").?);
    const rich = intro.data.rich_text;
    try std.testing.expectEqual(@as(usize, 7), rich.spans.len);
    try std.testing.expectEqual(zant.text.Weight.bold, rich.spans[1].weight);
    try std.testing.expect(rich.spans[3].italic);
    try std.testing.expectEqualStrings("0", rich.spans[5].link);
    const on_link = rich.link_action.?;
    on_link.func(on_link.ctx, "0");
    runtime.invalidate(); // called directly, not through an event
    frame(runtime, &renderer);
    try std.testing.expectEqual(@as(f64, 1), app.clicks);
    const updated = runtime.tree.get(runtime.tree.idOfKey("intro").?).data.rich_text;
    try std.testing.expectEqualStrings(" (1).", updated.spans[6].text);

    // <Icon> inside <Button>
    const save = runtime.tree.get(runtime.tree.idOfKey("save").?).data.button;
    try std.testing.expectEqual(zant.icons.Icon.check, save.icon);
    try std.testing.expectEqualStrings("Save", save.label);

    // the inner button gets its click, the card is not clicked
    try click(runtime, &renderer, "inner");
    try std.testing.expectEqual(@as(f64, 11), app.clicks);
    try std.testing.expectEqual(@as(f64, 0), app.cards);
    // a click on the card outside the button
    const card = runtime.tree.boundsOfKey("card").?;
    clickAt(runtime, &renderer, card.x + 8, card.y + 8);
    try std.testing.expectEqual(@as(f64, 1), app.cards);

    // keyed stateful children keep separate state
    try click(runtime, &renderer, "b");
    try click(runtime, &renderer, "b");
    try std.testing.expectEqualStrings("b: 2", runtime.tree.get(runtime.tree.idOfKey("b").?).data.button.label);
    try std.testing.expectEqualStrings("a: 0", runtime.tree.get(runtime.tree.idOfKey("a").?).data.button.label);

    // the modal is written inside a row but placed at the window root
    try click(runtime, &renderer, "save");
    const dialog = runtime.tree.idOfKey("dialog") orelse return error.MissingModal;
    try std.testing.expectEqual(runtime.tree.root, runtime.tree.get(dialog).parent.?);
    const box = runtime.tree.boundsOfKey("dialog").?;
    try std.testing.expectEqual(@as(f32, W), box.width);
}
