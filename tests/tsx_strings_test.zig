//! Render the generated string-method app and drive an actual input without a window.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const App = @import("tsx_golden_strings").App;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, 520, 520, 1.0);
    renderer.end();
}

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    return runtime.tree.get(id).data.text.content;
}

fn render(ctx: *anyopaque) ui.Node {
    const app: *App = @ptrCast(@alignCast(ctx));
    return app.render();
}

test "tsx strings: generated helpers, chained filters and input events" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = App{};
    defer app.query.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{
        .window = .{ .width = 520, .height = 520 },
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(520, 520);
    defer renderer.deinit();
    frame(runtime, &renderer);
    const eq = std.testing.expectEqualStrings;
    try eq("Alpha 中😀 | Beta | Alphabet", text(runtime, "results"));
    try eq("missing: -1 / 6", text(runtime, "index"));
    try eq("no/no", text(runtime, "checks"));
    try eq("中😀 / bc", text(runtime, "slice"));
    try eq("bc / bc", text(runtime, "defaults"));
    try eq("  x / xaba", text(runtime, "padding"));
    try eq("中中", text(runtime, "repeat"));
    try eq("b-a / b-b", text(runtime, "replace"));
    try eq("中-😀", text(runtime, "split"));
    try eq("中 / 😀 / none", text(runtime, "char"));

    const bounds = runtime.tree.boundsOfKey("search").?;
    const p = zant.events.PointerEvent{ .x = bounds.x + 10, .y = bounds.y + bounds.height / 2 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    _ = runtime.dispatch(.{ .text_input = "  ALpHa  " });
    frame(runtime, &renderer);
    try eq("Alpha 中😀 | Alphabet", text(runtime, "results"));
    try eq("ALPHA", text(runtime, "case"));
    try eq("found: 6 / 6", text(runtime, "index"));

    // Exercise query predicates with an unpadded lowercase value, then an empty result.
    app.query.set("a");
    runtime.invalidate();
    frame(runtime, &renderer);
    try eq("prefix/suffix", text(runtime, "checks"));
    app.query.set("not present");
    runtime.invalidate();
    frame(runtime, &renderer);
    try eq("", text(runtime, "results"));
}
