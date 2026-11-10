//! Todo app benchmark: the TSX version (`todos.tsx`, React-style immutable
//! updates) against the hand-written Zig one (`todos_manual.zig`, in-place
//! updates), driven through identical input.
const std = @import("std");
const zant = @import("zant");
const harness = @import("bench_harness.zig");
const Stats = harness.Stats;
const Harness = harness.Harness;
const TsxTodos = @import("todos_tsx").Todos;
const ManualTodos = @import("todos_manual.zig").Todos;

const items = 1000;

pub fn run(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer) !void {
    try scenario(gpa, renderer, TsxTodos, "tsx");
    try scenario(gpa, renderer, ManualTodos, "manual");
}

fn scenario(gpa: std.mem.Allocator, renderer: *zant.renderer.Renderer, comptime App: type, label: []const u8) !void {
    var app = App{};
    var h = try Harness.init(gpa, renderer, &app);
    defer h.deinit();
    var name_buf: [32]u8 = undefined;

    // add: type a title into the input and press Enter, `items` times
    var add: Stats = .{};
    defer add.samples.deinit(gpa);
    _ = try h.click("new");
    for (0..items) |i| {
        _ = h.rt.dispatch(.{ .text_input = try std.fmt.bufPrint(&name_buf, "Task {d}", .{i}) });
        _ = h.rt.dispatch(.{ .key_down = .{ .key = .enter } });
        add.add(gpa, h.frame());
    }
    // toggle: click the first 20 checkboxes in turn (they are on screen)
    var toggle: Stats = .{};
    defer toggle.samples.deinit(gpa);
    for (0..500) |i| {
        toggle.add(gpa, try h.click(try std.fmt.bufPrint(&name_buf, "check-{d}", .{i % 20 + 1})));
    }
    // filter: All / Active / Done
    var filter: Stats = .{};
    defer filter.samples.deinit(gpa);
    const segments = [_][]const u8{ "filter-1-hit", "filter-2-hit", "filter-0-hit" };
    for (0..60) |i| filter.add(gpa, try h.click(segments[i % segments.len]));

    var name: [32]u8 = undefined;
    add.report(try std.fmt.bufPrint(&name, "todos-add-{s}", .{label}));
    toggle.report(try std.fmt.bufPrint(&name, "todos-toggle-{s}", .{label}));
    filter.report(try std.fmt.bufPrint(&name, "todos-filter-{s}", .{label}));
}
