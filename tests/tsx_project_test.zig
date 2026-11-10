//! A TSX project end to end (`examples/tsx/notes/`): App.tsx imports
//! the NoteCard component and the Note type from NoteCard.tsx; callbacks
//! cross the generated modules; format.zig supplies the summary text.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const Notes = @import("notes_tsx").Notes;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, runtime.config.window.width, runtime.config.window.height, 1.0);
    renderer.end();
}

fn click(runtime: *Runtime, renderer: *Renderer, key: []const u8) !void {
    frame(runtime, renderer);
    const r = runtime.tree.boundsOfKey(key) orelse return error.MissingKey;
    const p = zant.events.PointerEvent{ .x = r.x + r.width * 0.5, .y = r.y + r.height * 0.5 };
    _ = runtime.dispatch(.{ .pointer_move = p });
    _ = runtime.dispatch(.{ .pointer_down = p });
    _ = runtime.dispatch(.{ .pointer_up = p });
    frame(runtime, renderer);
}

fn render(ctx: *anyopaque) ui.Node {
    const self: *Notes = @ptrCast(@alignCast(ctx));
    return self.render();
}

test "tsx project: components, types and Zig functions across sizes and themes" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    for ([_]zant.tokens.Theme{ zant.tokens.antLight(), zant.tokens.antDark() }) |theme| {
        try checkNotes(360, 440, theme);
        try checkNotes(720, 640, theme);
    }
}

fn checkNotes(width: u32, height: u32, theme: zant.tokens.Theme) !void {
    var notes = Notes{};
    defer notes.notes.deinit();
    defer notes.text.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &notes, render, .{
        .window = .{ .width = @floatFromInt(width), .height = @floatFromInt(height) },
        .theme = theme,
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(width, height);
    defer renderer.deinit();
    frame(runtime, &renderer);

    try std.testing.expect(textOf(runtime, "summary") == null);
    try click(runtime, &renderer, "new");
    for ([_][]const u8{ "Milk", "Bread" }) |title| {
        _ = runtime.dispatch(.{ .text_input = title });
        _ = runtime.dispatch(.{ .key_down = .{ .key = .enter } });
        frame(runtime, &renderer);
    }
    try std.testing.expectEqual(@as(usize, 2), notes.notes.get().len);
    try std.testing.expectEqualStrings("Bread", notes.notes.get()[1].title);
    try std.testing.expectEqualStrings("2 notes · 0 pinned", textOf(runtime, "summary").?);
    const summary = runtime.tree.boundsOfKey("summary").?;
    const input = runtime.tree.boundsOfKey("new").?;
    try std.testing.expect(summary.x >= 0 and summary.x + summary.width <= @as(f32, @floatFromInt(width)));
    try std.testing.expect(summary.y + summary.height <= input.y);
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive or node.exiting or node.data != .text) continue;
        if (!std.mem.eql(u8, node.key orelse continue, "summary")) continue;
        try std.testing.expectEqual(theme.color_text_secondary, node.data.text.style.color);
    }

    // NoteCard (NoteCard.tsx) → onPin={pin} (App.tsx method)
    try click(runtime, &renderer, "pin-1");
    try std.testing.expect(notes.notes.get()[0].pinned);
    try std.testing.expectEqualStrings("2 notes · 1 pinned", textOf(runtime, "summary").?);
    // NoteCard → onDelete={(id) => ...} (App.tsx arrow)
    try click(runtime, &renderer, "delete-2");
    try std.testing.expectEqual(@as(usize, 1), notes.notes.get().len);
    try std.testing.expect(runtime.tree.boundsOfKey("delete-2") == null);
    try std.testing.expectEqualStrings("1 note · 1 pinned", textOf(runtime, "summary").?);
}

fn textOf(runtime: *Runtime, key: []const u8) ?[]const u8 {
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive or node.exiting) continue;
        if (!std.mem.eql(u8, node.key orelse continue, key)) continue;
        if (node.data == .text) return node.data.text.content;
    }
    return null;
}
