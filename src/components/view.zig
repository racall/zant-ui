//! General-purpose container, like React Native's `View` or an HTML `div`.
//!
//! It takes every `ui.BoxOptions` field (same names) with RN / block `div`
//! defaults — a flex column whose children stretch across it and that is not
//! squeezed by its parent — plus a drop shadow, an optional click handler
//! (the whole view becomes clickable, controls inside keep their own clicks)
//! and an inline `style` override applied last.
//!
//!     ui.view(.{
//!         .padding = ui.inset(16),
//!         .gap = 8,
//!         .radius = 8,
//!         .background = theme.color_bg_container,
//!         .on_click = ui.action(self, State.open),
//!         .children = &.{ ui.text("Title", .{}), ui.text("Body", .{}) },
//!     })
const std = @import("std");
const ui = @import("../ui.zig");
const actions = @import("../core/actions.zig");
const geometry = @import("../core/geometry.zig");
const flex = @import("../layout/flex.zig");

pub const ViewOptions = struct {
    key: ?[]const u8 = null,
    /// Main axis (RN `flexDirection`, default `.column`); the CSS-style
    /// alias `flex_direction` takes precedence when set.
    direction: flex.Direction = .column,
    flex_direction: ?flex.Direction = null,
    gap: f32 = 0,
    row_gap: ?f32 = null,
    column_gap: ?f32 = null,
    wrap: flex.Wrap = .nowrap,
    align_content: flex.AlignContent = .start,
    align_self: flex.AlignSelf = .auto,
    order: i32 = 0,
    padding: geometry.Edges = .{},
    margin: geometry.Edges = .{},
    justify: flex.Justify = .start,
    justify_content: ?flex.Justify = null,
    /// Children fill the cross axis, like RN `View` and block `div`.
    align_items: flex.Align = .stretch,
    width: ?f32 = null,
    height: ?f32 = null,
    width_percent: ?f32 = null,
    height_percent: ?f32 = null,
    absolute: bool = false,
    top: ?f32 = null,
    right: ?f32 = null,
    bottom: ?f32 = null,
    left: ?f32 = null,
    min_width: ?f32 = null,
    min_height: ?f32 = null,
    max_width: ?f32 = null,
    max_height: ?f32 = null,
    grow: f32 = 0,
    /// RN default: a view keeps its content size instead of being squeezed.
    shrink: f32 = 0,
    basis: ?f32 = null,
    background: ?u32 = null,
    border_color: ?u32 = null,
    border_width: f32 = 0,
    radius: f32 = 0,
    clip: bool = false,
    /// Vertical scroll container (see `ui.BoxOptions.scroll`).
    scroll: bool = false,
    block_pointer: bool = false,
    selectable: bool = false,
    unselectable: bool = false,
    /// Background / border while hovered, painted without rebuilding.
    hover_background: ?u32 = null,
    hover_border_color: ?u32 = null,
    focus_scope: bool = false,
    show_on_hover: bool = false,
    on_resize: ?actions.Action = null,
    /// Drop shadow; use it sparingly (cards that float, popovers).
    shadow_color: ?u32 = null,
    shadow_blur: f32 = 0,
    shadow_offset_y: f32 = 0,
    /// Makes the whole view clickable (`ui.clickable`).
    on_click: ?actions.Action = null,
    /// Called with `index` instead of `on_click` (rows of a list).
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    /// Pointer cursor while hovering a clickable view.
    cursor: ui.Cursor = .pointing_hand,
    /// Applied last, like an inline `style` (see `ui.styled`).
    style: ?ui.Style = null,
    children: []const ui.Node = &.{},
};

pub fn view(options: ViewOptions) ui.Node {
    const node = if (options.on_click != null or options.index_action != null)
        ui.clickable(.{
            .box = boxOptions(options),
            .on_click = options.on_click,
            .index_action = options.index_action,
            .index = options.index,
            .cursor = options.cursor,
        })
    else
        ui.column(boxOptions(options));

    if (options.style == null and options.shadow_color == null) return node;
    var style = options.style orelse ui.Style{};
    if (options.shadow_color) |color| {
        if (style.shadow_color == null) style.shadow_color = color;
        if (style.shadow_blur == null) style.shadow_blur = options.shadow_blur;
        if (style.shadow_offset_y == null) style.shadow_offset_y = options.shadow_offset_y;
    }
    return ui.styled(node, style);
}

/// The view's container options. Every `BoxOptions` field must exist in
/// `ViewOptions` under the same name, so the two never drift apart.
pub fn boxOptions(options: ViewOptions) ui.BoxOptions {
    var box: ui.BoxOptions = .{};
    inline for (@typeInfo(ui.BoxOptions).@"struct".field_names) |name| {
        @field(box, name) = @field(options, name);
    }
    box.flex_direction = options.flex_direction orelse options.direction;
    return box;
}
