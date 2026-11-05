const std = @import("std");
const geometry = @import("geometry.zig");
const text = @import("../renderer/text.zig");
const rich_text = @import("../renderer/rich_text.zig");
const flex = @import("../layout/flex.zig");
const actions = @import("actions.zig");

pub const NodeId = u32;

pub const NodeKind = enum {
    root,
    container,
    text,
    button,
    input,
    toggle,
    image,
    select,
    rich_text,
};

pub const ToggleKind = enum {
    checkbox,
    switch_control,
    radio,
};

pub const DirtyFlags = packed struct {
    layout: bool = false,
    paint: bool = false,

    pub fn any(self: DirtyFlags) bool {
        return self.layout or self.paint;
    }
};

pub const Callback = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, node_id: NodeId) void,
};

pub const FlexStyle = struct {
    direction: flex.Direction = .row,
    justify: flex.Justify = .start,
    align_items: flex.Align = .start,
    gap: f32 = 0,
    row_gap: ?f32 = null,
    column_gap: ?f32 = null,
    wrap: flex.Wrap = .nowrap,
    align_content: flex.AlignContent = .start,
    order: i32 = 0,
    align_self: flex.AlignSelf = .auto,
    padding: geometry.Edges = .{},
    margin: geometry.Edges = .{},
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
    shrink: f32 = 1,
    basis: ?f32 = null,
};

pub const Visual = struct {
    background: ?u32 = null,
    border_color: ?u32 = null,
    border_width: f32 = 0,
    radius: f32 = 0,
    shadow_color: ?u32 = null,
    shadow_blur: f32 = 0,
    shadow_offset_y: f32 = 0,
    clip: bool = false,
};

pub const ContainerData = struct {};

pub const TextData = struct {
    content: []u8,
    style: text.Style,
    layout: ?text.TextLayout = null,
    layout_width: f32 = -1,
    layout_dirty: bool = true,

    pub fn invalidate(self: *TextData) void {
        self.layout_dirty = true;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
        self.layout_width = -1;
    }

    pub fn ensureLayout(self: *TextData, max_width: f32) !*text.TextLayout {
        if (!self.layout_dirty and self.layout != null and @abs(self.layout_width - max_width) < 0.25) {
            return &self.layout.?;
        }
        if (self.layout) |*old| old.deinit();
        self.layout = try text.TextLayout.init(self.content, self.style, max_width);
        self.layout_width = max_width;
        self.layout_dirty = false;
        return &self.layout.?;
    }
};

pub const RichSpan = struct {
    text: []const u8,
    font_size: f32 = 15,
    weight: text.Weight = .regular,
    color: u32 = 0xff292d2a,
    italic: bool = false,
    family: text.Family = .sans,
    background: u32 = 0,
};

pub const RichTextData = struct {
    spans: []RichSpan,
    layout: ?rich_text.Layout = null,
    layout_width: f32 = -1,
    layout_dirty: bool = true,

    pub fn invalidateLayout(self: *RichTextData) void {
        self.layout_dirty = true;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
        self.layout_width = -1;
    }

    pub fn ensureLayout(self: *RichTextData, max_width: f32) !*rich_text.Layout {
        if (!self.layout_dirty and self.layout != null and @abs(self.layout_width - max_width) < 0.25) {
            return &self.layout.?;
        }
        if (self.layout) |*old| old.deinit();
        const native = std.heap.c_allocator.alloc(rich_text.Span, self.spans.len) catch return error.OutOfMemory;
        defer std.heap.c_allocator.free(native);
        for (self.spans, 0..) |span, i| native[i] = .{
            .text = span.text.ptr,
            .length = span.text.len,
            .font_size = span.font_size,
            .weight = @intFromEnum(span.weight),
            .color = span.color,
            .italic = if (span.italic) 1 else 0,
            .family = @intFromEnum(span.family),
            .background = span.background,
        };
        self.layout = try rich_text.Layout.create(native, max_width, 2.0);
        self.layout_width = max_width;
        self.layout_dirty = false;
        return &self.layout.?;
    }
};

pub const ButtonType = enum {
    primary,
    default,
    text,
    link,
};

pub const ButtonIcon = enum { none, search, eye, more };

pub const ButtonSize = enum {
    small,
    middle,
    large,
};

pub const ButtonData = struct {
    label: []u8,
    button_type: ButtonType = .default,
    size: ButtonSize = .middle,
    hovered: bool = false,
    pressed: bool = false,
    focused: bool = false,
    hover_t: f32 = 0,
    press_t: f32 = 0,
    on_click: ?Callback = null,
    action: ?actions.Action = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    layout: ?text.TextLayout = null,
    layout_dirty: bool = true,
    layout_color: u32 = 0,
    font_size: f32 = 14,
    text_align: text.Align = .center,
    label_padding: f32 = 0,
    text_color: ?u32 = null,
    icon: ButtonIcon = .none,

    pub fn invalidateLayout(self: *ButtonData) void {
        self.layout_dirty = true;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
    }

    pub fn ensureLayout(self: *ButtonData, color: u32) !*text.TextLayout {
        if (!self.layout_dirty and self.layout != null and self.layout_color == color) return &self.layout.?;
        if (self.layout) |*old| old.deinit();
        self.layout = try text.TextLayout.init(self.label, .{
            .font_size = self.font_size,
            .weight = .medium,
            .color = color,
            .wrap = false,
            .text_align = .left,
        }, 4096);
        self.layout_dirty = false;
        self.layout_color = color;
        return &self.layout.?;
    }
};

pub const InputData = struct {
    text: std.ArrayListUnmanaged(u8) = .empty,
    placeholder: []u8,
    read_only: bool = false,
    password: bool = false,
    multiline: bool = false,
    focus_ring: bool = true,
    available_width: f32 = 1.0e6,
    focused: bool = false,
    hovered: bool = false,
    cursor: usize = 0,
    anchor: usize = 0,
    scroll_x: f32 = 0,
    scroll_y: f32 = 0,
    line_height: f32 = 0,
    composition: std.ArrayListUnmanaged(u8) = .empty,
    composition_cursor: usize = 0,
    has_composition: bool = false,
    blink_on: bool = true,
    next_blink_ms: i64 = 0,
    on_change: ?Callback = null,
    on_submit: ?Callback = null,
    text_action: ?actions.TextAction = null,
    submit_action: ?actions.Action = null,
    undo_action: ?actions.Action = null,
    redo_action: ?actions.Action = null,
    layout: ?text.TextLayout = null,
    layout_width: f32 = -1,
    layout_dirty: bool = true,
    display_text: std.ArrayListUnmanaged(u8) = .empty,
    placeholder_layout: ?text.TextLayout = null,
    font_size: f32 = 14,
    text_color: u32 = 0xe0000000,
    layout_color: u32 = 0,

    pub fn invalidateLayout(self: *InputData) void {
        self.layout_dirty = true;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
        self.layout_width = -1;
    }

    pub fn deinit(self: *InputData, allocator: std.mem.Allocator) void {
        self.text.deinit(allocator);
        self.composition.deinit(allocator);
        self.display_text.deinit(allocator);
        if (self.layout) |*layout| layout.deinit();
        if (self.placeholder_layout) |*layout| layout.deinit();
        self.layout = null;
        self.placeholder_layout = null;
    }
};

pub const ToggleData = struct {
    label: []u8,
    kind: ToggleKind = .checkbox,
    checked: bool = false,
    hovered: bool = false,
    pressed: bool = false,
    focused: bool = false,
    hover_t: f32 = 0,
    press_t: f32 = 0,
    check_t: f32 = 0,
    font_size: f32 = 14,
    action: ?actions.BoolAction = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    layout: ?text.TextLayout = null,
    layout_dirty: bool = true,

    pub fn invalidateLayout(self: *ToggleData) void {
        self.layout_dirty = true;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
    }

    pub fn ensureLayout(self: *ToggleData, color: u32) !*text.TextLayout {
        if (!self.layout_dirty and self.layout != null) return &self.layout.?;
        if (self.layout) |*old| old.deinit();
        self.layout = try text.TextLayout.init(self.label, .{
            .font_size = self.font_size,
            .weight = .regular,
            .color = color,
            .wrap = false,
        }, 4096);
        self.layout_dirty = false;
        return &self.layout.?;
    }
};

pub const ImageFit = enum(c_int) {
    cover = 0,
    contain = 1,
    fill = 2,
};

pub const ImageData = struct {
    path: []u8,
    handle: ?*anyopaque = null,
    intrinsic_width: c_int = 0,
    intrinsic_height: c_int = 0,
    fit: ImageFit = .cover,
    radius: f32 = 0,
};

pub const SelectOption = struct {
    label: []u8,
};

pub const SelectData = struct {
    options: std.ArrayListUnmanaged(SelectOption) = .empty,
    selected: usize = 0,
    open: bool = false,
    hovered: bool = false,
    focused: bool = false,
    hovered_option: ?usize = null,
    font_size: f32 = 14,
    action: ?actions.IndexAction = null,
    layout: ?text.TextLayout = null,
    option_layouts: std.ArrayListUnmanaged(text.TextLayout) = .empty,
    layout_dirty: bool = true,

    pub fn clearOptions(self: *SelectData, allocator: std.mem.Allocator) void {
        for (self.options.items) |option| allocator.free(option.label);
        self.options.clearRetainingCapacity();
        for (self.option_layouts.items) |*layout| layout.deinit();
        self.option_layouts.clearRetainingCapacity();
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
        self.layout_dirty = true;
    }

    pub fn deinit(self: *SelectData, allocator: std.mem.Allocator) void {
        self.clearOptions(allocator);
        self.options.deinit(allocator);
        self.option_layouts.deinit(std.heap.c_allocator);
    }
};

pub const Data = union(NodeKind) {
    root: void,
    container: ContainerData,
    text: TextData,
    button: ButtonData,
    input: InputData,
    toggle: ToggleData,
    image: ImageData,
    select: SelectData,
    rich_text: RichTextData,
};

pub const Node = struct {
    id: NodeId,
    parent: ?NodeId = null,
    children: std.ArrayListUnmanaged(NodeId) = .empty,
    style: FlexStyle = .{},
    visual: Visual = .{},
    key: ?[]u8 = null,
    tooltip: ?[]u8 = null,
    bounds: geometry.Rect = .{},
    dirty: DirtyFlags = .{},
    visible: bool = true,
    enabled: bool = true,
    focusable: bool = false,
    data: Data,

    pub fn kind(self: *const Node) NodeKind {
        return std.meta.activeTag(self.data);
    }

    pub fn deinit(self: *Node, allocator: std.mem.Allocator) void {
        if (self.key) |key| allocator.free(key);
        self.key = null;
        if (self.tooltip) |tooltip| allocator.free(tooltip);
        self.tooltip = null;
        switch (self.data) {
            .text => |*value| {
                if (value.layout) |*layout| layout.deinit();
                allocator.free(value.content);
            },
            .button => |*value| {
                if (value.layout) |*layout| layout.deinit();
                allocator.free(value.label);
            },
            .input => |*value| {
                allocator.free(value.placeholder);
                value.deinit(allocator);
            },
            .toggle => |*value| {
                if (value.layout) |*layout| layout.deinit();
                allocator.free(value.label);
            },
            .image => |*value| {
                @import("../components/image.zig").destroy(value);
                allocator.free(value.path);
            },
            .select => |*value| value.deinit(allocator),
            .rich_text => |*value| {
                value.invalidateLayout();
                for (value.spans) |span| allocator.free(span.text);
                allocator.free(value.spans);
            },
            else => {},
        }
        self.children.deinit(allocator);
    }

    pub fn isInteractive(self: *const Node) bool {
        if (!self.visible or !self.enabled) return false;
        return switch (self.data) {
            .button => true,
            .input => true,
            .toggle => true,
            .select => true,
            else => false,
        };
    }

    pub fn mark(self: *Node, layout: bool, paint: bool) void {
        if (layout) self.dirty.layout = true;
        if (paint or layout) self.dirty.paint = true;
    }
};
