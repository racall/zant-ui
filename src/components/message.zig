//! Global message in Ant Design's style (antd `message`): a small pill
//! centred near the top of the window that drops in and fades out. It holds
//! nothing clickable, so clicks pass through to the page.
//!
//! The app decides how long it stays: render it while a deadline is ahead
//! and call `ui.wakeAfter(remaining)` so it disappears on time.
const ui = @import("../ui.zig");
const icons = @import("../core/icons.zig");

pub const MessageKind = enum { success, info, warning, err };

pub const MessageOptions = struct {
    key: []const u8 = "message",
    text: []const u8,
    kind: MessageKind = .success,
    /// Distance from the top of the window.
    top: f32 = 16,
    /// Icon colour override (default: the kind's token colour).
    icon_color: ?u32 = null,
};

pub fn message(o: MessageOptions) ui.Node {
    const theme = ui.currentTheme();
    const icon: icons.Icon = switch (o.kind) {
        .success => .check,
        .info => .info,
        .warning, .err => .alert,
    };
    const color = o.icon_color orelse switch (o.kind) {
        .success => theme.color_success,
        .info => theme.color_primary,
        .warning => theme.color_warning,
        .err => theme.color_error,
    };
    // antd: elevated surface, 8px radius, 9px × 12px padding, soft shadow
    const pill = ui.styled(ui.row(.{
        .key = "message-pill",
        .gap = 8,
        .padding = .{ .left = 12, .right = 12, .top = 9, .bottom = 9 },
        .align_items = .center,
        .children = &.{
            ui.icon(.{ .name = icon, .size = 14, .color = color }),
            ui.text(o.text, .{ .size = 13, .color = theme.color_text }),
        },
    }), .{
        .background = theme.color_bg_elevated,
        .border_color = theme.color_split,
        .border_width = 1,
        .radius = theme.border_radius_lg,
        .shadow_color = 0x1f000000,
        .shadow_blur = 16,
        .shadow_offset_y = 6,
    });
    return ui.styled(ui.row(.{ .key = o.key, .justify = .center, .children = &.{pill} }), .{
        .absolute = true,
        .left = 0,
        .top = o.top,
        .width_percent = 100,
        .motion = .move_up,
    });
}
