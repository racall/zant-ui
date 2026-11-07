const std = @import("std");
const ui = @import("ui.zig");
const tokens = @import("theme/tokens.zig");
const Runtime = @import("ui_runtime.zig").Runtime;
const events = @import("core/events.zig");
const desktop = @import("desktop.zig");

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
///   it open, e.g. after asking about unsaved changes, or to hide it to the
///   tray instead (`zant.desktop.mainWindow().?.hide()`).
/// - `pub fn onQuitRequest(self: *State) bool` — asked instead of
///   `onCloseRequest` when the user quits the app (⌘Q, Dock menu) rather
///   than closing the window.
/// - `pub fn onAppEvent(self: *State, event: zant.desktop.AppEvent) bool` —
///   tray / status bar clicks and menu items, global hotkeys, Dock clicks
///   (`zant.desktop`). Return true when state changed.
/// - `pub fn onWake(self: *State) void` — on the UI thread after
///   `ui.postWake()` from a background thread: take the results it
///   published (then all windows rebuild).
/// - `pub fn onStart(self: *State) void` — once, after the main window
///   exists and before the first frame: open panels, set the tray icon,
///   register hotkeys, start background work.
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
        fn quit(ctx: *anyopaque) bool {
            const value: *State = @ptrCast(@alignCast(ctx));
            return value.onQuitRequest();
        }
        fn start(ctx: *anyopaque) void {
            const value: *State = @ptrCast(@alignCast(ctx));
            value.onStart();
        }
        fn wake(ctx: *anyopaque) void {
            const value: *State = @ptrCast(@alignCast(ctx));
            value.onWake();
        }
    };

    const runtime = try Runtime.init(allocator, state, Adapter.render, .{
        .title = options.title,
        .window = options.window,
        .theme = options.theme,
        .theme_fn = if (@hasDecl(State, "theme")) Adapter.theme else null,
        .key_fn = if (@hasDecl(State, "onKey")) Adapter.key else null,
        .close_fn = if (@hasDecl(State, "onCloseRequest")) Adapter.close else null,
        .quit_fn = if (@hasDecl(State, "onQuitRequest")) Adapter.quit else null,
        .start_fn = if (@hasDecl(State, "onStart")) Adapter.start else null,
        .wake_fn = if (@hasDecl(State, "onWake")) Adapter.wake else null,
    });
    defer runtime.deinit();
    desktop.setMainRuntime(runtime);
    defer desktop.setMainRuntime(null);
    if (@hasDecl(State, "onAppEvent")) desktop.setAppEventHandler(state, State.onAppEvent);
    try runtime.run();
}
