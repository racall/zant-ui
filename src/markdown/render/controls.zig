//! Small toolbar buttons used by code blocks, tables and diagrams.
const ui = @import("../../ui.zig");
const actions = @import("../../core/actions.zig");
const Ctx = @import("context.zig").Ctx;

/// Space between action buttons: labels mix CJK and Latin words, so a
/// word-sized gap would read as one sentence.
pub const gap: f32 = 10;

pub fn button(ctx: Ctx, key: []const u8, label: []const u8, action: ?actions.Action, enabled: bool) ui.Node {
    return ui.styled(ui.button(.{
        .key = key,
        .text = label,
        .variant = .text,
        .size = .small,
        .font_size = 12,
        .disabled = !enabled,
        .text_color = ctx.options.theme.button_fg,
        .on_click = action,
    }), .{ .height = 24, .radius = 5, .padding = .{ .left = 8, .right = 8 } });
}
