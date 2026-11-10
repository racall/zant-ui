//! Built-in expressions render through generated Zig without opening a window.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const App = @import("tsx_golden_builtins").App;

fn render(ctx: *anyopaque) ui.Node {
    const app: *App = @ptrCast(@alignCast(ctx));
    return app.render();
}

fn frame(runtime: *Runtime, renderer: *Renderer, width: u32, height: u32) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, @floatFromInt(width), @floatFromInt(height), 1.0);
    renderer.end();
}

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    return runtime.tree.get(id).data.text.content;
}

test "tsx built-ins render JS numeric strings and preserve integer precision" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var app = App{};
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{
        .window = .{ .width = 900, .height = 620 },
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(900, 620);
    defer renderer.deinit();
    const eq = std.testing.expectEqualStrings;
    inline for (.{ zant.tokens.antLight, zant.tokens.antDark }) |theme| {
        runtime.setTheme(theme());
        runtime.invalidate();
        frame(runtime, &renderer, 900, 620);
        try eq("2/3/-2/-2", text(runtime, "rounding"));
        try eq("0.99/10.00/3", text(runtime, "fixed"));
        try eq("9007199254740993/9/-9", text(runtime, "integers"));
        try eq("9007199254740993/9007199254740993/9007199254740993/9007199254740993", text(runtime, "identities"));
        try eq("1/0/0/31/2.5", text(runtime, "conversion"));
        try eq("-31/5/35/125", text(runtime, "parse"));
        try eq("NaN/Infinity/-Infinity/0", text(runtime, "special"));
        try eq("1e+21/1e-7/100000000000000000000/0.000001/2.5", text(runtime, "format"));
        try eq("true/false/false/true", text(runtime, "predicates"));
        try eq("true/true/9007199254740991/2.220446049250313e-16", text(runtime, "constants"));
        try eq("-1,3,-2,8,1,0,3,2,0,1,0,0,0,5/8", text(runtime, "math"));
        try eq("true/true/true", text(runtime, "clock"));
    }
    // Narrow layout still builds the same text; a click updates the numeric state.
    runtime.invalidate();
    frame(runtime, &renderer, 600, 620);
    const bounds = runtime.tree.boundsOfKey("change").?;
    const p = zant.events.PointerEvent{ .x = bounds.x + 10, .y = bounds.y + bounds.height / 2 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, &renderer, 600, 620);
    try eq("1/2/-1/-1", text(runtime, "rounding"));
    try eq("0.99/10.00/1", text(runtime, "fixed"));
}
