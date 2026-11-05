//! Streamdown's link-safety dialog: shows the URL with Open / Copy / Cancel.
//! Render it as the last child of the window root while a link is pending.
const ui = @import("../../ui.zig");
const actions = @import("../../core/actions.zig");
const theme_mod = @import("../theme.zig");
const Labels = @import("options.zig").Labels;

pub const Props = struct {
    url: []const u8,
    theme: theme_mod.Theme = theme_mod.light(),
    labels: Labels = .{},
    on_open: ?actions.Action = null,
    on_copy: ?actions.Action = null,
    on_close: ?actions.Action = null,
};

pub fn linkSafetyModal(p: Props) ui.Node {
    const t = p.theme;
    const card = ui.column(.{
        .key = "md-link-modal-card",
        .width = 420,
        .padding = ui.inset(22),
        .gap = 12,
        .radius = 12,
        .background = t.surface,
        .border_color = t.border,
        .border_width = 1,
        .align_items = .stretch,
        .children = &.{
            ui.text(p.labels.link_title, .{ .size = 16, .weight = .semibold, .color = t.foreground }),
            ui.paragraph(p.labels.link_message, .{ .size = 13, .color = t.muted_foreground }),
            ui.styled(ui.paragraph(p.url, .{ .size = 12, .color = t.foreground }), .{
                .background = t.muted,
                .radius = 6,
                .padding = .{ .left = 10, .right = 10, .top = 8, .bottom = 8 },
            }),
            ui.row(.{ .gap = 8, .justify = .end, .width_percent = 100, .children = &.{
                ui.button(.{ .key = "md-link-cancel", .text = p.labels.cancel, .variant = .text, .size = .small, .on_click = p.on_close }),
                ui.button(.{ .key = "md-link-copy", .text = p.labels.copy_link, .size = .small, .on_click = p.on_copy }),
                ui.button(.{ .key = "md-link-open", .text = p.labels.open_link, .variant = .primary, .size = .small, .on_click = p.on_open }),
            } }),
        },
    });
    return ui.styled(ui.column(.{
        .key = "md-link-modal",
        .justify = .center,
        .align_items = .center,
        .background = t.overlay,
        .block_pointer = true,
        .children = &.{ui.styled(card, .{ .shadow_color = 0x33000000, .shadow_blur = 18, .shadow_offset_y = 8, .motion = .zoom_big })},
    }), .{ .absolute = true, .left = 0, .top = 0, .width_percent = 100, .height_percent = 100, .motion = .fade });
}
