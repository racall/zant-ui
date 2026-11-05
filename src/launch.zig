const std = @import("std");
const ui = @import("ui.zig");
const tokens = @import("theme/tokens.zig");
const Runtime = @import("ui_runtime.zig").Runtime;
const events = @import("core/events.zig");

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
/// Optional hooks, detected on the state type:
///
/// - `pub fn theme(self: *State) tokens.Theme` — evaluated before every
///   rebuild; return `tokens.antDark()` for dark mode.
/// - `pub fn onKey(self: *State, key: events.KeyEvent) bool` — application
///   shortcuts (⌘S, Esc, ...), seen before the focused widget. Return true to
///   consume the key.
/// - `pub fn onCloseRequest(self: *State) bool` — asked before the window
///   closes or the app quits (close button, ⌘W, ⌘Q). Return false to keep
///   it open, e.g. after asking about unsaved changes.
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
        fn theme(ctx: *anyopaque) tokens.Theme {
            const value: *State = @ptrCast(@alignCast(ctx));
            return value.theme();
        }
        fn key(ctx: *anyopaque, event: events.KeyEvent) bool {
            const value: *State = @ptrCast(@alignCast(ctx));
            return value.onKey(event);
        }
        fn close(ctx: *anyopaque) bool {
            const value: *State = @ptrCast(@alignCast(ctx));
            return value.onCloseRequest();
        }
    };

    const runtime = try Runtime.init(allocator, state, Adapter.render, .{
        .title = options.title,
        .window = options.window,
        .theme = options.theme,
        .theme_fn = if (@hasDecl(State, "theme")) Adapter.theme else null,
        .key_fn = if (@hasDecl(State, "onKey")) Adapter.key else null,
        .close_fn = if (@hasDecl(State, "onCloseRequest")) Adapter.close else null,
    });
    defer runtime.deinit();
    try runtime.run();
}
