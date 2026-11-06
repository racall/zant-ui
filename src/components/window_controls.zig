//! Windows caption buttons (minimize, maximize / restore, close) for windows
//! with an app-drawn title bar (`WindowStyle.hidden_titlebar`). The runtime
//! adds them in the top-right corner on Windows, so apps only keep the space
//! free (`ui.titlebarInsets()`); macOS keeps its native traffic lights.
//!
//! Sizes follow Windows 11: 46 px wide buttons filling the title bar height,
//! 10 px glyphs with 1 px strokes, a faint hover fill and the red close
//! button. The maximize button reports itself to the window (key
//! `caption_maximize_key`) so Windows 11 shows its snap layouts flyout.
const ui = @import("../ui.zig");
const actions = @import("../core/actions.zig");
const platform = @import("../platform/platform.zig");

pub const caption_maximize_key = "zant-caption-max";

pub const WindowControlsOptions = struct {
    /// Title bar height (the buttons fill it).
    height: f32 = 32,
    maximized: bool = false,
    /// Inactive windows draw dimmer glyphs.
    active: bool = true,
    on_minimize: ?actions.Action = null,
    on_maximize: ?actions.Action = null,
    on_close: ?actions.Action = null,
};

fn captionButton(key: []const u8, icon: ui.Icon, variant: @import("../core/node.zig").ButtonType, color: u32, height: f32, action: ?actions.Action) ui.Node {
    return ui.styled(ui.button(.{
        .key = key,
        .variant = variant,
        .icon = icon,
        .icon_size = 10,
        .text_color = color,
        .focusable = false,
        .on_click = action,
    }), .{
        .width = platform.caption_button_width,
        .height = height,
        .radius = 0,
        .padding = .{},
    });
}

pub fn windowControls(options: WindowControlsOptions) ui.Node {
    const theme = ui.currentTheme();
    const color = if (options.active) theme.color_text else theme.color_text_secondary;
    return ui.styled(ui.row(.{
        .key = "zant-caption",
        .children = &.{
            captionButton("zant-caption-min", .window_minimize, .caption, color, options.height, options.on_minimize),
            captionButton(caption_maximize_key, if (options.maximized) .window_restore else .window_maximize, .caption, color, options.height, options.on_maximize),
            captionButton("zant-caption-close", .window_close, .caption_close, color, options.height, options.on_close),
        },
    }), .{ .absolute = true, .top = 0, .right = 0, .height = options.height });
}
