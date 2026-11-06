//! Keyboard shortcut reference: grouped "action — keys" rows laid out in
//! balanced columns (macOS / antd Descriptions style). Typically the content
//! of a `ui.modal` opened with ⌘/.
//!
//!     ui.shortcutList(.{ .groups = &.{
//!         .{ .title = "文件", .items = &.{ .{ .label = "保存", .keys = "⌘S" } } },
//!     } })
const std = @import("std");
const ui = @import("../ui.zig");

pub const Shortcut = struct {
    label: []const u8,
    /// Key glyphs as shown (e.g. "⇧⌘S", "⌘Z  ⇧⌘Z", "Esc").
    keys: []const u8,
};

pub const ShortcutGroup = struct {
    title: []const u8,
    items: []const Shortcut,
};

pub const ShortcutListOptions = struct {
    key: []const u8 = "shortcuts",
    groups: []const ShortcutGroup,
    /// Groups are spread over this many columns, keeping heights balanced.
    columns: usize = 2,
    /// Windows key names ("Ctrl+Shift+S") are wider than macOS glyphs ("⇧⌘S").
    column_width: f32 = if (@import("builtin").os.tag == .macos) 250 else 310,
    column_gap: f32 = 40,
    row_height: f32 = 28,
    font_size: f32 = 13,
};

pub fn shortcutList(o: ShortcutListOptions) ui.Node {
    const theme = ui.currentTheme();
    const arena = ui.frameAllocator();
    const n_cols = @max(1, @min(o.columns, 8));
    var cols: [8]std.ArrayListUnmanaged(ui.Node) = @splat(.empty);
    var heights: [8]usize = @splat(0);
    for (o.groups, 0..) |g, gi| {
        // the shortest column takes the next group (order is kept per column)
        var target: usize = 0;
        for (1..n_cols) |c| {
            if (heights[c] < heights[target]) target = c;
        }
        const first = cols[target].items.len == 0;
        cols[target].append(arena, ui.column(.{
            .key = std.fmt.allocPrint(arena, "{s}-g{d}", .{ o.key, gi }) catch null,
            .padding = .{ .top = if (first) 0 else 14, .bottom = 4 },
            .children = arena.dupe(ui.Node, &.{ui.text(g.title, .{ .size = o.font_size - 1, .color = theme.color_text_secondary })}) catch &.{},
        })) catch {};
        for (g.items) |item| cols[target].append(arena, ui.row(.{
            .height = o.row_height,
            .gap = 16,
            .justify = .space_between,
            .align_items = .center,
            .children = arena.dupe(ui.Node, &.{
                ui.styled(ui.text(item.label, .{ .size = o.font_size, .color = theme.color_text, .max_lines = 1 }), .{ .shrink = 1, .min_width = 0 }),
                ui.text(item.keys, .{ .size = o.font_size, .color = theme.color_text_secondary }),
            }) catch &.{},
        })) catch {};
        heights[target] += g.items.len + 2;
    }
    const columns = arena.alloc(ui.Node, n_cols) catch return ui.column(.{ .key = o.key });
    for (columns, 0..) |*col, c| col.* = ui.column(.{
        .width = o.column_width,
        .align_items = .stretch,
        .children = cols[c].items,
    });
    return ui.row(.{ .key = o.key, .gap = o.column_gap, .align_items = .start, .children = columns });
}
