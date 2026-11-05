const std = @import("std");
const ui = @import("ui.zig");
const tokens = @import("theme/tokens.zig");
const Runtime = @import("ui_runtime.zig").Runtime;

pub const Options = struct {
    title: [:0]const u8 = "Zant UI",
    window: @import("platform/platform.zig").WindowOptions = .{},
    theme: tokens.Theme = .{},
};

/// Minimal application entry point.
///
/// The user only provides a state value, a render function and callbacks.
/// Window creation, event dispatch, reconciliation, Metal/Skia setup and frame
/// scheduling are all handled here.
///
/// For shared/asynchronous state use `ui_runtime.Runtime` + `store.Store`.
pub fn run(
    allocator: std.mem.Allocator,
    initial: anytype,
    comptime render_fn: *const fn (*@TypeOf(initial)) ui.Node,
    options: Options,
) !void {
    const State = @TypeOf(initial);
    const state = try allocator.create(State);
    state.* = initial;
    defer allocator.destroy(state);

    const Adapter = struct {
        fn render(ctx: *anyopaque) ui.Node {
            const value: *State = @ptrCast(@alignCast(ctx));
            return render_fn(value);
        }
    };

    const runtime = try Runtime.init(allocator, state, Adapter.render, .{
        .title = options.title,
        .window = options.window,
        .theme = options.theme,
    });
    defer runtime.deinit();
    try runtime.run();
}
