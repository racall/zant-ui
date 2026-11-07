//! Modal dialog in Ant Design's style (antd `Modal`): a dimmed mask over the
//! window with a centred card — title, close button, content and an
//! optional footer. The mask fades and the card zooms (antd motion).
//!
//! Render it as the last child of the window root while it is open; give
//! each modal its own `key`. Esc handling stays with the app (call the same
//! close action from its key handler).
//!
//!     if (state.help_open) children.append(ui.modal(.{
//!         .title = "键盘快捷键",
//!         .on_close = ui.action(state, State.closeHelp),
//!         .content = ui.shortcutList(.{ .groups = &groups }),
//!     }));
const std = @import("std");
const ui = @import("../ui.zig");
const actions = @import("../core/actions.zig");

pub const ModalOptions = struct {
    key: []const u8 = "modal",
    title: []const u8 = "",
    content: ui.Node,
    /// Buttons row (antd: right-aligned, primary last). Null for none.
    footer: ?ui.Node = null,
    /// Card width (antd default 520). Null sizes it to the content.
    width: ?f32 = 520,
    /// Close button, mask click (when `mask_closable`) and the app's Esc.
    on_close: ?actions.Action = null,
    closable: bool = true,
    mask_closable: bool = true,
    /// Mask colour (antd colorBgMask: rgba(0,0,0,.45)).
    mask: u32 = 0x73000000,
};

pub fn modal(o: ModalOptions) ui.Node {
    const theme = ui.currentTheme();
    const arena = ui.frameAllocator();
    const k = struct {
        fn f(base: []const u8, comptime suffix: []const u8) []const u8 {
            return std.fmt.allocPrint(ui.frameAllocator(), "{s}-" ++ suffix, .{base}) catch base;
        }
    }.f;

    var body: std.ArrayListUnmanaged(ui.Node) = .empty;
    if (o.title.len > 0 or o.closable) {
        var head: std.ArrayListUnmanaged(ui.Node) = .empty;
        head.append(arena, ui.styled(ui.text(o.title, .{ .size = 16, .weight = .semibold, .color = theme.color_text, .max_lines = 1 }), .{ .grow = 1, .shrink = 1, .min_width = 0 })) catch {};
        if (o.closable) head.append(arena, ui.iconButton(.{
            .key = k(o.key, "close"),
            .icon = .close,
            .size = 16,
            .box = 28,
            .color = theme.color_text_secondary,
            .tooltip = "Esc",
            .on_click = o.on_close,
        })) catch {};
        body.append(arena, ui.row(.{ .key = k(o.key, "head"), .align_items = .center, .gap = 12, .children = head.items })) catch {};
    }
    body.append(arena, o.content) catch {};
    if (o.footer) |footer| body.append(arena, ui.row(.{ .key = k(o.key, "foot"), .justify = .end, .gap = 8, .children = arena.dupe(ui.Node, &.{footer}) catch &.{} })) catch {};

    // antd: 8px radius, 20px/24px padding, elevated surface, soft shadow
    const card = ui.styled(ui.column(.{
        .key = k(o.key, "card"),
        .width = o.width,
        .padding = .{ .left = 24, .right = 24, .top = 20, .bottom = 24 },
        .gap = 16,
        .align_items = .stretch,
        .children = body.items,
    }), .{
        .background = theme.color_bg_elevated,
        .radius = theme.border_radius_lg,
        .shadow_color = 0x29000000,
        .shadow_blur = 24,
        .shadow_offset_y = 8,
        .block_pointer = true,
        .motion = .zoom_big,
    });
    const mask = ui.dismissLayer(.{
        .key = k(o.key, "mask"),
        .on_dismiss = if (o.mask_closable) o.on_close else null,
        .background = o.mask,
    });
    // the centring layer holds nothing clickable: clicks beside the card
    // reach the mask
    const center = ui.styled(ui.column(.{
        .key = k(o.key, "center"),
        .justify = .center,
        .align_items = .center,
        .padding = ui.inset(24),
        .children = arena.dupe(ui.Node, &.{card}) catch &.{},
    }), .{ .absolute = true, .left = 0, .top = 0, .width_percent = 100, .height_percent = 100 });
    return ui.styled(ui.column(.{
        .key = o.key,
        // Tab stays inside the dialog while it is open
        .focus_scope = true,
        .children = arena.dupe(ui.Node, &.{ mask, center }) catch &.{},
    }), .{
        .absolute = true,
        .left = 0,
        .top = 0,
        .width_percent = 100,
        .height_percent = 100,
        .motion = .fade,
    });
}
