const std = @import("std");
const Tree = @import("core/tree.zig").Tree;
const NodeId = @import("core/node.zig").NodeId;
const theme_mod = @import("theme/tokens.zig");
const text = @import("renderer/text.zig");
const geometry = @import("core/geometry.zig");
const node_mod = @import("core/node.zig");

const Theme = theme_mod.Theme;
const Edges = geometry.Edges;

pub const GalleryState = struct {
    allocator: std.mem.Allocator,
    tree: *Tree,
    theme: Theme,
    counter: u32 = 0,
    counter_text: NodeId = 0,
    value_text: NodeId = 0,
    increment_button: NodeId = 0,
    default_input: NodeId = 0,
    placeholder_input: NodeId = 0,

    pub fn build(self: *GalleryState, allocator: std.mem.Allocator, tree: *Tree, theme: Theme) !void {
        self.* = .{ .allocator = allocator, .tree = tree, .theme = theme };
        const root = tree.get(tree.root);
        root.visual.background = theme.color_bg_base;
        root.style.direction = .column;
        root.style.align_items = .stretch;
        root.style.gap = 0;

        const header = try tree.addContainer(tree.root, .{
            .direction = .column,
            .justify = .center,
            .gap = 4,
            .height = 90,
            .padding = .{ .left = 34, .right = 34, .top = 18, .bottom = 14 },
        });
        const header_node = tree.get(header);
        header_node.visual.background = theme.color_bg_container;

        _ = try tree.addText(header, "Zant UI — Component Gallery", .{
            .font_size = 26,
            .weight = .semibold,
            .color = theme.color_text,
            .wrap = false,
        });
        _ = try tree.addText(header, "Native Desktop UI powered by Zig + Skia", .{
            .font_size = 14,
            .color = theme.color_text_secondary,
            .wrap = false,
        });

        const divider = try tree.addContainer(tree.root, .{ .height = 1 });
        tree.get(divider).visual.background = theme.color_border_secondary;

        const body = try tree.addContainer(tree.root, .{
            .direction = .row,
            .align_items = .start,
            .gap = 20,
            .grow = 1,
            .padding = .{ .left = 32, .right = 32, .top = 18, .bottom = 18 },
        });

        const left = try tree.addContainer(body, .{
            .direction = .column,
            .align_items = .stretch,
            .gap = 14,
            .basis = 0,
            .grow = 1,
        });
        const right = try tree.addContainer(body, .{
            .direction = .column,
            .align_items = .stretch,
            .gap = 14,
            .width = 322,
        });

        try self.buildFlexCard(left);
        try self.buildButtonsCard(left);
        try self.buildInteractiveCard(left);
        try self.buildTypographyCard(right);
        try self.buildInputsCard(right);

        tree.markLayoutDirty();
    }

    fn addCard(self: *GalleryState, parent: NodeId, title: []const u8) !NodeId {
        const card = try self.tree.addContainer(parent, .{
            .direction = .column,
            .align_items = .stretch,
            .gap = 10,
            .padding = .symmetric(16, 14),
        });
        const node = self.tree.get(card);
        node.visual.background = self.theme.color_bg_container;
        node.visual.border_color = self.theme.color_border_secondary;
        node.visual.border_width = 1;
        node.visual.radius = 10;
        _ = try self.tree.addText(card, title, .{
            .font_size = 16,
            .weight = .semibold,
            .color = self.theme.color_text,
            .wrap = false,
        });
        return card;
    }

    fn addDescription(self: *GalleryState, parent: NodeId, value: []const u8) !void {
        _ = try self.tree.addText(parent, value, .{
            .font_size = 12,
            .color = self.theme.color_text_secondary,
            .wrap = true,
        });
    }

    fn addBox(self: *GalleryState, parent: NodeId, color: u32, width: ?f32, grow: f32, height: f32) !NodeId {
        const box = try self.tree.addContainer(parent, .{
            .width = width,
            .height = height,
            .grow = grow,
        });
        const node = self.tree.get(box);
        node.visual.background = color;
        node.visual.radius = 4;
        return box;
    }

    fn buildFlexCard(self: *GalleryState, parent: NodeId) !void {
        const card = try self.addCard(parent, "Flex Layout");
        try self.addDescription(card, "Flex rows and columns with gaps, padding, grow and wrap.");

        const row = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .center,
            .gap = 8,
            .height = 44,
        });
        _ = try self.addBox(row, self.theme.color_primary, null, 1, 30);
        _ = try self.addBox(row, 0xff91caff, 60, 0, 30);
        _ = try self.addBox(row, 0xffbae0ff, null, 1, 30);

        const nested = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .center,
            .gap = 6,
            .height = 34,
            .padding = .symmetric(6, 4),
        });
        const nested_node = self.tree.get(nested);
        nested_node.visual.background = self.theme.color_fill_secondary;
        nested_node.visual.radius = 6;
        _ = try self.addBox(nested, 0xffffd591, 44, 0, 18);
        _ = try self.addBox(nested, self.theme.color_primary, null, 1, 10);
        _ = try self.addBox(nested, 0xfffff1b8, 30, 0, 18);
    }

    fn buildTypographyCard(self: *GalleryState, parent: NodeId) !void {
        const card = try self.addCard(parent, "Typography");
        _ = try self.tree.addText(card, "Heading", .{
            .font_size = 22,
            .weight = .semibold,
            .color = self.theme.color_text,
            .wrap = false,
        });
        _ = try self.tree.addText(card, "Body text is rendered with Skia paragraph shaping and system font fallback.", .{
            .font_size = 14,
            .color = self.theme.color_text,
            .wrap = true,
        });
        _ = try self.tree.addText(card, "Secondary text · 12 px", .{
            .font_size = 12,
            .color = self.theme.color_text_secondary,
            .wrap = false,
        });
        _ = try self.tree.addText(card, "中文排版与英文混排，自动字体回退。", .{
            .font_size = 14,
            .color = self.theme.color_text,
            .wrap = true,
        });
        _ = try self.tree.addText(card, "This paragraph wraps onto multiple lines and its measured height participates in flex layout.", .{
            .font_size = 13,
            .color = self.theme.color_text_secondary,
            .wrap = true,
            .line_height = 1.5,
        });
    }

    fn buildButtonsCard(self: *GalleryState, parent: NodeId) !void {
        const card = try self.addCard(parent, "Buttons");
        const row1 = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .center,
            .gap = 8,
        });
        _ = try self.tree.addButton(row1, "Primary", .primary, .middle, null);
        _ = try self.tree.addButton(row1, "Default", .default, .middle, null);
        const disabled = try self.tree.addButton(row1, "Disabled", .default, .middle, null);
        self.tree.setNodeEnabled(disabled, false);

        const row2 = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .center,
            .gap = 8,
        });
        _ = try self.tree.addButton(row2, "Text", .text, .middle, null);
        _ = try self.tree.addButton(row2, "Link", .link, .middle, null);
        _ = try self.tree.addButton(row2, "Small", .primary, .small, null);
        _ = try self.tree.addButton(row2, "Middle", .default, .middle, null);
        _ = try self.tree.addButton(row2, "Large", .link, .large, null);

        _ = try self.tree.addText(card, "Tab to focus · Space / Enter to activate a button", .{
            .font_size = 12,
            .color = self.theme.color_text_secondary,
            .wrap = true,
        });
    }

    fn buildInputsCard(self: *GalleryState, parent: NodeId) !void {
        const card = try self.addCard(parent, "Inputs");

        const row1 = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .start,
            .gap = 12,
        });
        self.default_input = try self.addInputColumn(row1, "Default", "Zant UI", "Type here", false, true, false);
        self.placeholder_input = try self.addInputColumn(row1, "Placeholder", "", "Type something…", false, true, false);

        const row2 = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .start,
            .gap = 12,
        });
        _ = try self.addInputColumn(row2, "Disabled", "Disabled value", "", true, false, false);
        _ = try self.addInputColumn(row2, "Initial content", "中文 English mix", "", false, true, false);

        self.value_text = try self.tree.addText(card, "Value: —", .{
            .font_size = 12,
            .color = self.theme.color_primary,
            .wrap = true,
            .max_lines = 1,
        });
        self.tree.get(self.value_text).style.height = 18;
    }

    fn addInputColumn(
        self: *GalleryState,
        parent: NodeId,
        label: []const u8,
        value: []const u8,
        placeholder: []const u8,
        disabled: bool,
        editable: bool,
        read_only: bool,
    ) !NodeId {
        const column = try self.tree.addContainer(parent, .{
            .direction = .column,
            .align_items = .stretch,
            .gap = 5,
            .basis = 0,
            .grow = 1,
        });
        _ = try self.tree.addText(column, label, .{
            .font_size = 12,
            .color = self.theme.color_text_secondary,
            .wrap = false,
        });
        const input_id = try self.tree.addInput(column, value, placeholder, .{
            .ctx = @ptrCast(self),
            .func = onInputChanged,
        });
        const node = self.tree.get(input_id);
        node.style.height = 32;
        node.style.width = null;
        node.data.input.read_only = read_only;
        if (!editable) node.data.input.read_only = true;
        if (disabled) self.tree.setNodeEnabled(input_id, false);
        return input_id;
    }

    fn buildInteractiveCard(self: *GalleryState, parent: NodeId) !void {
        const card = try self.addCard(parent, "Interactive Demo");
        const row = try self.tree.addContainer(card, .{
            .direction = .row,
            .align_items = .center,
            .gap = 14,
        });
        self.increment_button = try self.tree.addButton(row, "Increment", .primary, .middle, .{
            .ctx = @ptrCast(self),
            .func = onIncrement,
        });
        self.counter_text = try self.tree.addText(row, "Clicked: 0", .{
            .font_size = 14,
            .weight = .medium,
            .color = self.theme.color_text,
            .wrap = false,
        });
    }
};

fn onIncrement(ctx: *anyopaque, _: NodeId) void {
    const self: *GalleryState = @ptrCast(@alignCast(ctx));
    self.counter += 1;
    var buffer: [64]u8 = undefined;
    const value = std.fmt.bufPrint(&buffer, "Clicked: {d}", .{self.counter}) catch return;
    self.tree.setText(self.counter_text, value) catch return;
    self.tree.markPaintDirty();
}

fn onInputChanged(ctx: *anyopaque, node_id: NodeId) void {
    const self: *GalleryState = @ptrCast(@alignCast(ctx));
    const node = self.tree.get(node_id);
    const value = node.data.input.text.items;
    const display = if (value.len == 0) "Value: —" else std.fmt.allocPrint(self.allocator, "Value: {s}", .{value}) catch return;
    defer if (value.len != 0) self.allocator.free(display);
    self.tree.setTextNoLayout(self.value_text, display) catch return;
    self.tree.markPaintDirty();
}
