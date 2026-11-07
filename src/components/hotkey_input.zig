//! Shortcut recorder (a settings field for a global hotkey): shows the
//! current combination as key caps; click it, press a new combination, done.
//! Recording state lives in the app (`recording`), like every Zant control.
//!
//!     ui.hotkeyInput(.{
//!         .key = "panel-hotkey",
//!         .value = self.hotkey,
//!         .recording = self.recording,
//!         .on_start = ui.action(self, State.startRecording),   // unregister the hotkey meanwhile
//!         .on_record = ui.hotkeyAction(self, State.setHotkey),
//!         .on_cancel = ui.action(self, State.stopRecording),
//!         .on_clear = ui.action(self, State.clearHotkey),
//!     })
//!
//! While recording: a combination with Ctrl / Alt (⌥) / ⌘ (Win) records it,
//! Esc cancels, Backspace / Delete clears. Unregister the app's own hotkey
//! in `on_start`: a registered hotkey never reaches the window.
const std = @import("std");
const ui = @import("../ui.zig");
const actions = @import("../core/actions.zig");
const events = @import("../core/events.zig");
const desktop = @import("../desktop.zig");

pub const HotkeyAction = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, shortcut: desktop.Shortcut) void,
};

/// Wraps `fn (ctx, zant.desktop.Shortcut) void`.
pub fn hotkeyAction(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), desktop.Shortcut) void) HotkeyAction {
    const Adapter = struct {
        fn call(raw: *anyopaque, shortcut: desktop.Shortcut) void {
            func(@ptrCast(@alignCast(raw)), shortcut);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub const HotkeyInputOptions = struct {
    key: []const u8 = "hotkey-input",
    value: ?desktop.Shortcut = null,
    recording: bool = false,
    width: ?f32 = null,
    height: f32 = 32,
    placeholder: []const u8 = "点击设置快捷键",
    recording_text: []const u8 = "按下组合键…",
    /// Clicked (or Enter / Space while focused): start recording.
    on_start: ?actions.Action = null,
    on_record: ?HotkeyAction = null,
    /// Esc while recording.
    on_cancel: ?actions.Action = null,
    /// The clear button, or Backspace / Delete while recording.
    on_clear: ?actions.Action = null,
};

const Handler = struct {
    o: HotkeyInputOptions,

    fn onPointer(self: *Handler, info: ui.PointerInfo) bool {
        if (info.phase != .up or info.button != .left) return false;
        if (info.x < 0 or info.y < 0 or info.x > info.width or info.y > info.height) return false;
        if (self.o.recording) return false;
        ui.focus(self.o.key);
        if (self.o.on_start) |a| a.func(a.ctx);
        return true;
    }

    fn onKey(self: *Handler, k: events.KeyEvent) bool {
        if (!self.o.recording) {
            const start = switch (k.key) {
                .enter => true,
                .character => |c| c == ' ',
                else => false,
            };
            if (start) if (self.o.on_start) |a| a.func(a.ctx);
            return start;
        }
        const plain = !(k.modifiers.control or k.modifiers.alt or k.modifiers.meta or k.modifiers.shift);
        switch (k.key) {
            .escape => if (plain) {
                if (self.o.on_cancel) |a| a.func(a.ctx);
                return true;
            },
            .backspace, .delete => if (plain) {
                if (self.o.on_clear) |a| a.func(a.ctx);
                return true;
            },
            else => {},
        }
        if (desktop.Shortcut.fromKeyEvent(k)) |shortcut| {
            if (self.o.on_record) |a| a.func(a.ctx, shortcut);
        }
        // while recording every key belongs to the recorder
        return true;
    }
};

fn keyCap(label: []const u8, theme: anytype) ui.Node {
    return ui.row(.{
        .padding = ui.insetXY(6, 1),
        .radius = 4,
        .background = theme.color_fill_tertiary,
        .border_color = theme.color_border_secondary,
        .border_width = 1,
        .children = ui.frameAllocator().dupe(ui.Node, &.{ui.text(label, .{ .size = 12, .weight = .medium, .color = theme.color_text })}) catch &.{},
    });
}

pub fn hotkeyInput(o: HotkeyInputOptions) ui.Node {
    const theme = ui.currentTheme();
    const arena = ui.frameAllocator();
    const handler = arena.create(Handler) catch @panic("Zant UI: out of memory");
    handler.* = .{ .o = o };

    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    if (o.recording) {
        children.append(arena, ui.text(o.recording_text, .{ .size = 13, .color = theme.color_primary })) catch {};
    } else if (o.value) |shortcut| {
        // one cap per key: "⌃⌘V" -> ⌃ ⌘ V, "Ctrl+Alt+V" -> Ctrl Alt V
        const label_buf = arena.alloc(u8, 64) catch @panic("Zant UI: out of memory");
        const label = shortcut.label(label_buf);
        var cap_nodes: std.ArrayListUnmanaged(ui.Node) = .empty;
        if (desktop.is_macos) {
            var it = (std.unicode.Utf8View.init(label) catch unreachable).iterator();
            while (it.nextCodepointSlice()) |cp| cap_nodes.append(arena, keyCap(cp, theme)) catch {};
        } else {
            var parts = std.mem.splitScalar(u8, label, '+');
            while (parts.next()) |part| cap_nodes.append(arena, keyCap(part, theme)) catch {};
        }
        children.append(arena, ui.row(.{ .gap = 4, .align_items = .center, .children = cap_nodes.items })) catch {};
    } else {
        children.append(arena, ui.text(o.placeholder, .{ .size = 13, .color = theme.color_text_disabled })) catch {};
    }
    children.append(arena, ui.row(.{ .grow = 1 })) catch {};
    if (o.value != null and !o.recording) if (o.on_clear) |clear| {
        children.append(arena, ui.iconButton(.{
            .key = std.fmt.allocPrint(arena, "{s}-clear", .{o.key}) catch o.key,
            .icon = .close,
            .size = 14,
            .box = 22,
            .color = theme.color_text_secondary,
            .tooltip = "清除",
            .on_click = clear,
        })) catch {};
    };

    return ui.pointerArea(.{
        .box = .{
            .key = o.key,
            .width = o.width,
            .height = o.height,
            .padding = .{ .left = 8, .right = 4 },
            .radius = 6,
            .align_items = .center,
            .flex_direction = .row,
            .background = if (o.recording) theme.color_primary_bg else theme.color_bg_container,
            .border_color = if (o.recording) theme.color_primary else theme.color_border,
            .border_width = 1,
            .hover_border_color = if (o.recording) null else theme.color_primary_hover,
            .children = children.items,
        },
        .cursor = .pointing_hand,
        .focusable = true,
        .on_pointer = ui.pointerAction(handler, Handler.onPointer),
        .on_key = ui.keyAction(handler, Handler.onKey),
    });
}
