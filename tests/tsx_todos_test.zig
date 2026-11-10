//! A generated todo component driven through native runtime events and frame resets.
const std = @import("std");
const zant = @import("zant");
const ui = zant.ui;
const Runtime = zant.ui_runtime.Runtime;
const Renderer = zant.renderer.Renderer;
const Todos = @import("tsx_golden_todos").Todos;
const W = 640;
const H = 800;

const Adapter = struct {
    fn render(pointer: *anyopaque) ui.Node {
        const state: *Todos = @ptrCast(@alignCast(pointer));
        return state.render();
    }
};

fn frame(runtime: *Runtime, renderer: *Renderer) void {
    const canvas = renderer.begin(1).?;
    runtime.renderFrame(canvas, W, H, 1);
    renderer.end();
}

fn click(runtime: *Runtime, renderer: *Renderer, key: []const u8) !void {
    frame(runtime, renderer);
    const bounds = runtime.tree.boundsOfKey(key) orelse return error.MissingKey;
    const point = zant.events.PointerEvent{
        .x = bounds.x + bounds.width / 2,
        .y = bounds.y + bounds.height / 2,
    };
    _ = runtime.dispatch(.{ .pointer_move = point });
    _ = runtime.dispatch(.{ .pointer_down = point });
    _ = runtime.dispatch(.{ .pointer_up = point });
    frame(runtime, renderer);
}

fn add(runtime: *Runtime, renderer: *Renderer, title: []const u8) !void {
    try click(runtime, renderer, "draft");
    _ = runtime.dispatch(.{ .text_input = title });
    frame(runtime, renderer);
    _ = runtime.dispatch(.{ .key_down = .{ .key = .enter } });
    frame(runtime, renderer);
}

test "generated todos: add, toggle, remove, filters and retired state arenas" {
    zant.motion.enabled = false;
    defer zant.motion.enabled = true;
    var state = Todos{};
    state.todos.allocator = std.testing.allocator;
    state.name.allocator = std.testing.allocator;
    defer state.todos.deinit();
    defer state.name.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &state, Adapter.render, .{
        .window = .{ .width = W, .height = H },
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();

    try add(runtime, &renderer, "First");
    try add(runtime, &renderer, "Second");
    try std.testing.expectEqual(@as(usize, 2), state.todos.get().len);
    try std.testing.expectEqualStrings("First", state.todos.get()[0].title);
    try std.testing.expectEqualStrings("Second", state.todos.get()[1].title);
    try click(runtime, &renderer, "toggle-1");
    try std.testing.expect(state.todos.get()[0].done);
    try std.testing.expect(!state.todos.get()[1].done);
    try click(runtime, &renderer, "active");
    try std.testing.expect(runtime.tree.boundsOfKey("toggle-1") == null);
    try std.testing.expect(runtime.tree.boundsOfKey("toggle-2") != null);
    try click(runtime, &renderer, "done");
    try std.testing.expect(runtime.tree.boundsOfKey("toggle-1") != null);
    try std.testing.expect(runtime.tree.boundsOfKey("toggle-2") == null);
    try click(runtime, &renderer, "all");
    try click(runtime, &renderer, "remove-2");
    try std.testing.expectEqual(@as(usize, 1), state.todos.get().len);
    try click(runtime, &renderer, "clear");
    try std.testing.expectEqual(@as(usize, 0), state.todos.get().len);

    // Every render sweeps retired arenas. The state allocator checks all copies
    // are released, and frame captures are reclaimed by the runtime frame arena.
    for (0..1000) |_| {
        const id = state.nextId;
        try add(runtime, &renderer, "Cycle");
        var buffer: [64]u8 = undefined;
        const key = try std.fmt.bufPrint(&buffer, "remove-{d}", .{id});
        try click(runtime, &renderer, key);
        try std.testing.expectEqual(@as(usize, 0), state.todos.get().len);
        try std.testing.expectEqual(@as(usize, 0), state.todos.retired.items.len);
    }
}

fn textOf(runtime: *Runtime, key: []const u8) ?[]const u8 {
    for (runtime.tree.nodes.items) |node| {
        if (!node.alive or node.exiting) continue;
        if (!std.mem.eql(u8, node.key orelse continue, key)) continue;
        if (node.data == .text) return node.data.text.content;
    }
    return null;
}

test "generated array loops compute results and capture a render local" {
    const Arrays = @import("tsx_golden_arrays").Arrays;
    const Render = struct {
        fn render(pointer: *anyopaque) ui.Node {
            const state: *Arrays = @ptrCast(@alignCast(pointer));
            return state.render();
        }
    };
    var state = Arrays{};
    state.boxes.allocator = std.testing.allocator;
    defer state.boxes.deinit();
    const runtime = try Runtime.init(std.testing.allocator, &state, Render.render, .{
        .window = .{ .width = W, .height = H },
    });
    defer runtime.deinit();
    var renderer = try Renderer.createRaster(W, H);
    defer renderer.deinit();
    frame(runtime, &renderer);
    try std.testing.expectEqualStrings("Plans 3", textOf(runtime, "banner").?);
    try std.testing.expectEqualStrings("Pro / Team Pro,Team First mode=type", textOf(runtime, "strings").?);
    try std.testing.expectEqualStrings("2 true true 1 true", textOf(runtime, "results").?);
    try std.testing.expectEqualStrings("4 7 false", textOf(runtime, "lengths").?);
    try std.testing.expectEqualStrings("10 2 3 3", textOf(runtime, "numbers").?);
    try std.testing.expectEqualStrings("4 1", textOf(runtime, "bounds").?);
    try std.testing.expectEqualStrings("4 4", textOf(runtime, "callbacks").?);
    try std.testing.expectEqualStrings("1 99", textOf(runtime, "nullable").?);
    try std.testing.expectEqualStrings("2 4", textOf(runtime, "peer").?);
    try click(runtime, &renderer, "capture");
    try std.testing.expectEqual(@as(usize, 1), state.boxes.get().len);
    try click(runtime, &renderer, "update");
    try std.testing.expectEqualStrings("mode=type", state.boxes.get()[0].title);
}
