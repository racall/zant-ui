//! Imperative TSX statements driven through real actions and the CPU renderer, without a window.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const App = @import("tsx_golden_loops").App;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, runtime.config.window.width, runtime.config.window.height, 1.0);
    renderer.end();
}

fn click(runtime: *Runtime, renderer: *Renderer, key: []const u8) !void {
    frame(runtime, renderer);
    const r = runtime.tree.boundsOfKey(key) orelse return error.MissingKey;
    try std.testing.expect(r.y >= 0 and r.y + r.height <= runtime.config.window.height);
    const p = zant.events.PointerEvent{ .x = r.x + r.width * 0.5, .y = r.y + r.height * 0.5 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, renderer);
}

fn text(runtime: *Runtime, key: []const u8) []const u8 {
    const id = runtime.tree.idOfKey(key) orelse return "";
    return runtime.tree.get(id).data.text.content;
}

fn render(ctx: *anyopaque) ui.Node {
    const app: *App = @ptrCast(@alignCast(ctx));
    return app.render();
}

test "tsx loops: computed results, control targets and callback returns across themes and sizes" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    for ([_]zant.tokens.Theme{ zant.tokens.antLight(), zant.tokens.antDark() }) |theme| {
        try check(600, 620, theme);
        try check(900, 800, theme);
    }
}

fn check(width: u32, height: u32, theme: zant.tokens.Theme) !void {
    var app = App{};
    defer zant.tsx.deinitFields(&app);
    const runtime = try Runtime.init(std.testing.allocator, &app, render, .{
        .window = .{ .width = @floatFromInt(width), .height = @floatFromInt(height) },
        .theme = theme,
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(width, height);
    defer renderer.deinit();

    frame(runtime, &renderer);
    try std.testing.expect(runtime.needs_build);
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("15", text(runtime, "mounted"));
    try std.testing.expectEqualStrings("2", text(runtime, "filtered"));
    try std.testing.expectEqualStrings("15", text(runtime, "props"));
    try std.testing.expect(!runtime.needs_build);

    const cases = [_][2][]const u8{
        .{ "classic", "8" },
        .{ "each", "29" },
        .{ "conditions", "9" },
        .{ "select", "33" },
        .{ "select", "103" },
        .{ "sum", "8" },
    };
    for (cases) |case| {
        try click(runtime, &renderer, case[0]);
        try std.testing.expectEqualStrings(case[1], text(runtime, "result"));
        if (std.mem.eql(u8, case[0], "each")) {
            try std.testing.expectEqualStrings("2", text(runtime, "visits"));
        }
        if (std.mem.eql(u8, case[1], "33")) {
            try std.testing.expectEqualStrings("3", text(runtime, "visits"));
        }
    }
    // No case matched: the default in the middle runs after all case expressions.
    try std.testing.expectEqualStrings("4", text(runtime, "visits"));
    try click(runtime, &renderer, "items");
    try std.testing.expect(runtime.needs_build);
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("6", text(runtime, "mounted"));
    try std.testing.expectEqualStrings("6", text(runtime, "props"));
    try std.testing.expect(!runtime.needs_build);
    try click(runtime, &renderer, "each");
    try std.testing.expectEqualStrings("17", text(runtime, "result"));
}
