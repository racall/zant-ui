const std = @import("std");
const platform = @import("../platform/platform.zig");

pub const PointerButton = enum {
    left,
    right,
    middle,
};

pub const Key = union(enum) {
    character: u21,
    left,
    right,
    up,
    down,
    home,
    end,
    backspace,
    delete,
    escape,
    tab,
    enter,
};

pub const Modifiers = packed struct {
    shift: bool = false,
    control: bool = false,
    alt: bool = false,
    meta: bool = false,
    caps_lock: bool = false,

    /// The platform's command modifier: ⌘ on macOS, Ctrl on Windows.
    pub fn primary(self: Modifiers) bool {
        return @import("keys.zig").primary(self);
    }
};

pub const PointerEvent = struct {
    x: f32,
    y: f32,
    button: PointerButton = .left,
    modifiers: Modifiers = .{},
};

pub const KeyEvent = struct {
    key: Key,
    modifiers: Modifiers = .{},
    repeat: bool = false,
};

pub const Composition = struct {
    text: []const u8,
    selection_start: usize,
    selection_end: usize,
};

pub const Event = union(enum) {
    pointer_move: PointerEvent,
    pointer_down: PointerEvent,
    pointer_up: PointerEvent,
    pointer_enter: PointerEvent,
    pointer_leave: PointerEvent,
    key_down: KeyEvent,
    key_up: KeyEvent,
    text_input: []const u8,
    composition: Composition,
    window_resize: struct { width: f32, height: f32, dpr: f32 },
    window_focus,
    window_blur,
    window_close,
    /// The user asked to close the window or quit; the app may veto it.
    window_close_request,
    scroll: struct { x: f32, y: f32, dx: f32, dy: f32, modifiers: Modifiers },
};

pub fn modifiersFromPlatform(mods: c_int) Modifiers {
    return .{
        .shift = (mods & platform.Mod.shift) != 0,
        .control = (mods & platform.Mod.control) != 0,
        .alt = (mods & platform.Mod.alt) != 0,
        .meta = (mods & platform.Mod.meta) != 0,
        .caps_lock = (mods & platform.Mod.caps_lock) != 0,
    };
}

pub fn keyFromPlatform(key: c_int) Key {
    return switch (key) {
        platform.Key.left => .left,
        platform.Key.right => .right,
        platform.Key.up => .up,
        platform.Key.down => .down,
        platform.Key.home => .home,
        platform.Key.end => .end,
        platform.Key.backspace => .backspace,
        platform.Key.delete => .delete,
        platform.Key.escape => .escape,
        platform.Key.tab => .tab,
        platform.Key.enter => .enter,
        else => .{ .character = if (key >= 0) @intCast(key) else 0 },
    };
}

pub fn translate(raw: *const platform.Event) Event {
    const mods = modifiersFromPlatform(raw.modifiers);
    const pointer = PointerEvent{
        .x = raw.x,
        .y = raw.y,
        .button = switch (raw.button) {
            platform.PointerButton.right => .right,
            platform.PointerButton.middle => .middle,
            else => .left,
        },
        .modifiers = mods,
    };
    return switch (raw.type) {
        platform.EventType.pointer_move => .{ .pointer_move = pointer },
        platform.EventType.pointer_down => .{ .pointer_down = pointer },
        platform.EventType.pointer_up => .{ .pointer_up = pointer },
        platform.EventType.pointer_enter => .{ .pointer_enter = pointer },
        platform.EventType.pointer_leave => .{ .pointer_leave = pointer },
        platform.EventType.key_down => .{ .key_down = .{
            .key = keyFromPlatform(raw.key),
            .modifiers = mods,
            .repeat = raw.is_repeat != 0,
        } },
        platform.EventType.key_up => .{ .key_up = .{
            .key = keyFromPlatform(raw.key),
            .modifiers = mods,
        } },
        platform.EventType.text_input => .{ .text_input = raw.textSlice() },
        platform.EventType.composition => .{ .composition = .{
            .text = raw.textSlice(),
            .selection_start = raw.selection_start,
            .selection_end = raw.selection_end,
        } },
        platform.EventType.window_resize => .{ .window_resize = .{
            .width = @floatFromInt(raw.width),
            .height = @floatFromInt(raw.height),
            .dpr = raw.dpr,
        } },
        platform.EventType.window_focus => .window_focus,
        platform.EventType.window_blur => .window_blur,
        platform.EventType.window_close => .window_close,
        platform.EventType.window_close_request => .window_close_request,
        platform.EventType.scroll => .{ .scroll = .{
            .x = raw.x,
            .y = raw.y,
            .dx = raw.scroll_x,
            .dy = raw.scroll_y,
            .modifiers = mods,
        } },
        else => .window_focus,
    };
}

pub fn characterEquals(key: Key, c: u8) bool {
    return switch (key) {
        .character => |cp| cp == c,
        else => false,
    };
}
