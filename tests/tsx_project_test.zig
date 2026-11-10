//! A two-file TSX project end to end (`examples/tsx/notes/`): App.tsx imports
//! the NoteCard component and the Note type from NoteCard.tsx; callbacks
//! cross the generated modules.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const Notes = @import("notes_tsx").Notes;

const W = 520;
const H = 560;

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1.0).?;
    runtime.renderFrame(canvas, W, H, 1.0);
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

test "tsx project: components and types imported from another file" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var notes = Notes{};
    defer notes.notes.deinit();
    defer notes.text.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &notes, render, .{
        .window = .{ .width = W, .height = H },
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);

    try click(runtime, &renderer, "new");
    for ([_][]const u8{ "Milk", "Bread" }) |title| {
        _ = runtime.dispatch(.{ .text_input = title });
        _ = runtime.dispatch(.{ .key_down = .{ .key = .enter } });
        frame(runtime, &renderer);
    }
    try std.testing.expectEqual(@as(usize, 2), notes.notes.get().len);
    try std.testing.expectEqualStrings("Bread", notes.notes.get()[1].title);

    // NoteCard (NoteCard.tsx) → onPin={pin} (App.tsx method)
    try click(runtime, &renderer, "pin-1");
    try std.testing.expect(notes.notes.get()[0].pinned);
    // NoteCard → onDelete={(id) => ...} (App.tsx arrow)
    try click(runtime, &renderer, "delete-2");
    try std.testing.expectEqual(@as(usize, 1), notes.notes.get().len);
    try std.testing.expect(runtime.tree.boundsOfKey("delete-2") == null);
}
