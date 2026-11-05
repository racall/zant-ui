const std = @import("std");
const geometry = @import("core/geometry.zig");
const node_mod = @import("core/node.zig");
const actions = @import("core/actions.zig");
const flex = @import("layout/flex.zig");
const tokens = @import("theme/tokens.zig");
const text_mod = @import("renderer/text.zig");
const pdf_mod = @import("platform/pdf.zig");
const platform_mod = @import("platform/platform.zig");

pub const Node = struct {
    element: *const Element,
};

pub const Element = struct {
    kind: node_mod.NodeKind,
    key: ?[]const u8 = null,
    layout: node_mod.FlexStyle = .{},
    visual: node_mod.Visual = .{},
    enabled: bool = true,
    focusable: bool = false,
    content: []const u8 = "",
    placeholder: []const u8 = "",
    text_style: text_mod.Style = .{},
    button_type: node_mod.ButtonType = .default,
    button_size: node_mod.ButtonSize = .middle,
    toggle_kind: node_mod.ToggleKind = .checkbox,
    checked: bool = false,
    controlled: bool = false,
    read_only: bool = false,
    password: bool = false,
    multiline: bool = false,
    focus_ring: bool = true,
    text_color: ?u32 = null,
    label_padding: f32 = 0,
    icon: node_mod.ButtonIcon = .none,
    on_click: ?actions.Action = null,
    on_bool: ?actions.BoolAction = null,
    on_text: ?actions.TextAction = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    tooltip: ?[]const u8 = null,
    image_fit: node_mod.ImageFit = .cover,
    options: []const []const u8 = &.{},
    selected: usize = 0,
    children: []const Node = &.{},
    rich_spans: []const node_mod.RichSpan = &.{},
};

threadlocal var frame_arena = std.heap.ArenaAllocator.init(std.heap.c_allocator);
threadlocal var frame_active = false;

const default_theme = tokens.Theme{};

pub fn beginFrame() void {
    _ = frame_arena.reset(.retain_capacity);
    frame_active = true;
}

pub fn endFrame() void {
    frame_active = false;
}

pub fn isFrameActive() bool {
    return frame_active;
}

fn alloc(comptime T: type) *T {
    return frame_arena.allocator().create(T) catch @panic("Zant UI: out of memory while building a frame");
}

fn allocSlice(comptime T: type, length: usize) []T {
    return frame_arena.allocator().alloc(T, length) catch
        @panic("Zant UI: out of memory while building a frame");
}

fn copyString(value: []const u8) []const u8 {
    if (value.len == 0) return "";
    return frame_arena.allocator().dupe(u8, value) catch
        @panic("Zant UI: out of memory while building a frame");
}

fn copyKey(value: ?[]const u8) ?[]const u8 {
    if (value) |key| return copyString(key);
    return null;
}

pub fn action(ctx: anytype, comptime func: *const fn (@TypeOf(ctx)) void) actions.Action {
    const Adapter = struct {
        fn call(raw: *anyopaque) void {
            func(@ptrCast(@alignCast(raw)));
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub fn boolAction(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), bool) void) actions.BoolAction {
    const Adapter = struct {
        fn call(raw: *anyopaque, checked: bool) void {
            func(@ptrCast(@alignCast(raw)), checked);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub fn textAction(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), []const u8) void) actions.TextAction {
    const Adapter = struct {
        fn call(raw: *anyopaque, value: []const u8) void {
            func(@ptrCast(@alignCast(raw)), value);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub fn indexAction(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), usize) void) actions.IndexAction {
    const Adapter = struct {
        fn call(raw: *anyopaque, index: usize) void {
            func(@ptrCast(@alignCast(raw)), index);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub const Style = struct {
    background: ?u32 = null,
    color: ?u32 = null,
    border_color: ?u32 = null,
    border_radius: ?f32 = null,
    border_width: ?f32 = null,
    radius: ?f32 = null,
    padding: ?geometry.Edges = null,
    margin: ?geometry.Edges = null,
    width: ?f32 = null,
    height: ?f32 = null,
    width_percent: ?f32 = null,
    height_percent: ?f32 = null,
    min_width: ?f32 = null,
    min_height: ?f32 = null,
    max_width: ?f32 = null,
    max_height: ?f32 = null,
    grow: ?f32 = null,
    shrink: ?f32 = null,
    basis: ?f32 = null,
    order: ?i32 = null,
    align_self: ?flex.AlignSelf = null,
    absolute: ?bool = null,
    top: ?f32 = null,
    right: ?f32 = null,
    bottom: ?f32 = null,
    left: ?f32 = null,
    clip: ?bool = null,
    text_color: ?u32 = null,
    font_size: ?f32 = null,
    font_weight: ?text_mod.Weight = null,
    text_align: ?text_mod.Align = null,
    line_height: ?f32 = null,
    wrap: ?bool = null,
};

/// Override common visual/layout/text properties of any component.
///
///     ui.styled(ui.button(.{ .text = "Save" }), .{
///         .background = 0xff111111,
///         .radius = 2,
///         .height = 40,
///     })
pub fn styled(node: Node, style: Style) Node {
    const copy = alloc(Element);
    copy.* = node.element.*;
    if (style.background) |value| copy.visual.background = value;
    if (style.border_color) |value| copy.visual.border_color = value;
    if (style.border_width) |value| copy.visual.border_width = value;
    if (style.border_radius orelse style.radius) |value| copy.visual.radius = value;
    if (style.padding) |value| copy.layout.padding = value;
    if (style.margin) |value| copy.layout.margin = value;
    if (style.width) |value| copy.layout.width = value;
    if (style.height) |value| copy.layout.height = value;
    if (style.min_width) |value| copy.layout.min_width = value;
    if (style.min_height) |value| copy.layout.min_height = value;
    if (style.max_width) |value| copy.layout.max_width = value;
    if (style.max_height) |value| copy.layout.max_height = value;
    if (style.grow) |value| copy.layout.grow = value;
    if (style.shrink) |value| copy.layout.shrink = value;
    if (style.basis) |value| copy.layout.basis = value;
    if (style.width_percent) |value| copy.layout.width_percent = value;
    if (style.height_percent) |value| copy.layout.height_percent = value;
    if (style.absolute) |value| copy.layout.absolute = value;
    if (style.top) |value| copy.layout.top = value;
    if (style.right) |value| copy.layout.right = value;
    if (style.bottom) |value| copy.layout.bottom = value;
    if (style.left) |value| copy.layout.left = value;
    if (style.order) |value| copy.layout.order = value;
    if (style.align_self) |value| copy.layout.align_self = value;
    if (style.absolute) |value| copy.layout.absolute = value;
    if (style.top) |value| copy.layout.top = value;
    if (style.right) |value| copy.layout.right = value;
    if (style.bottom) |value| copy.layout.bottom = value;
    if (style.left) |value| copy.layout.left = value;
    if (style.clip) |value| copy.visual.clip = value;
    if (style.color orelse style.text_color) |value| copy.text_style.color = value;
    if (style.font_size) |value| copy.text_style.font_size = value;
    if (style.font_weight) |value| copy.text_style.weight = value;
    if (style.text_align) |value| copy.text_style.text_align = value;
    if (style.line_height) |value| copy.text_style.line_height = value;
    if (style.wrap) |value| copy.text_style.wrap = value;
    return .{ .element = copy };
}

pub fn inset(value: f32) geometry.Edges {
    return geometry.Edges.all(value);
}

pub fn insetXY(horizontal: f32, vertical: f32) geometry.Edges {
    return geometry.Edges.symmetric(horizontal, vertical);
}

fn make(element: Element) Node {
    const copy = alloc(Element);
    copy.* = element;
    copy.key = copyKey(element.key);
    copy.content = copyString(element.content);
    copy.placeholder = copyString(element.placeholder);
    copy.tooltip = if (element.tooltip) |value| copyString(value) else null;
    if (element.options.len > 0) {
        const options = allocSlice([]const u8, element.options.len);
        for (element.options, 0..) |option, i| options[i] = copyString(option);
        copy.options = options;
    } else {
        copy.options = &.{};
    }
    if (element.children.len > 0) {
        const children = allocSlice(Node, element.children.len);
        @memcpy(children, element.children);
        copy.children = children;
    } else {
        copy.children = &.{};
    }
    if (element.rich_spans.len > 0) {
        const spans = allocSlice(node_mod.RichSpan, element.rich_spans.len);
        for (element.rich_spans, 0..) |span, i| {
            spans[i] = span;
            spans[i].text = copyString(span.text);
        }
        copy.rich_spans = spans;
    } else {
        copy.rich_spans = &.{};
    }
    return .{ .element = copy };
}

fn layoutFrom(box: BoxOptions, direction: flex.Direction) node_mod.FlexStyle {
    return .{
        .direction = box.flex_direction orelse direction,
        .justify = box.justify_content orelse box.justify,
        .align_items = box.align_items,
        .gap = box.gap,
        .row_gap = box.row_gap,
        .column_gap = box.column_gap,
        .wrap = box.wrap,
        .align_content = box.align_content,
        .align_self = box.align_self,
        .order = box.order,
        .padding = box.padding,
        .margin = box.margin,
        .width = box.width,
        .height = box.height,
        .width_percent = box.width_percent,
        .height_percent = box.height_percent,
        .absolute = box.absolute,
        .top = box.top,
        .right = box.right,
        .bottom = box.bottom,
        .left = box.left,
        .min_width = box.min_width,
        .min_height = box.min_height,
        .max_width = box.max_width,
        .max_height = box.max_height,
        .grow = box.grow,
        .shrink = box.shrink,
        .basis = box.basis,
    };
}

fn visualFrom(box: BoxOptions) node_mod.Visual {
    return .{
        .background = box.background,
        .border_color = box.border_color,
        .border_width = box.border_width,
        .radius = box.radius,
        .clip = box.clip,
    };
}

pub const BoxOptions = struct {
    key: ?[]const u8 = null,
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
    align_items: flex.Align = .start,
    flex_direction: ?flex.Direction = null,
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
    shrink: f32 = 0,
    basis: ?f32 = null,
    background: ?u32 = null,
    border_color: ?u32 = null,
    border_width: f32 = 0,
    radius: f32 = 0,
    clip: bool = false,
    children: []const Node = &.{},
};

pub fn column(options: BoxOptions) Node {
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = layoutFrom(options, .column),
        .visual = visualFrom(options),
        .children = options.children,
    });
}

pub fn row(options: BoxOptions) Node {
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = layoutFrom(options, .row),
        .visual = visualFrom(options),
        .children = options.children,
    });
}

pub const SpaceOptions = struct {
    key: ?[]const u8 = null,
    direction: flex.Direction = .row,
    gap: f32 = 8,
    align_items: flex.Align = .center,
    justify: flex.Justify = .start,
    width: ?f32 = null,
    height: ?f32 = null,
    children: []const Node = &.{},
};

pub fn space(options: SpaceOptions) Node {
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = options.direction,
            .justify = options.justify,
            .align_items = options.align_items,
            .gap = options.gap,
            .width = options.width,
            .height = options.height,
        },
        .children = options.children,
    });
}

pub const TextOptions = struct {
    key: ?[]const u8 = null,
    size: f32 = 14,
    weight: text_mod.Weight = .regular,
    color: ?u32 = null,
    text_align: text_mod.Align = .left,
    wrap: bool = false,
    line_height: f32 = 0,
    max_lines: u16 = 0,
    family: text_mod.Family = .sans,
};

fn textStyle(options: TextOptions, default_color: u32, wrap_default: bool) text_mod.Style {
    return .{
        .font_size = options.size,
        .weight = options.weight,
        .color = options.color orelse default_color,
        .text_align = options.text_align,
        .wrap = if (options.wrap) true else wrap_default,
        .line_height = options.line_height,
        .max_lines = options.max_lines,
        .family = options.family,
    };
}

pub fn text(value: []const u8, options: TextOptions) Node {
    return make(.{
        .kind = .text,
        .key = options.key,
        .content = value,
        .layout = .{ .shrink = 0 },
        .text_style = textStyle(options, default_theme.color_text, false),
    });
}

pub fn textFmt(comptime fmt: []const u8, args: anytype, options: TextOptions) Node {
    const value = std.fmt.allocPrint(frame_arena.allocator(), fmt, args) catch
        @panic("Zant UI: out of memory while formatting text");
    return text(value, options);
}

pub const HeadingOptions = struct {
    key: ?[]const u8 = null,
    level: u8 = 1,
    color: ?u32 = null,
    weight: text_mod.Weight = .semibold,
    text_align: text_mod.Align = .left,
};

pub fn heading(value: []const u8, options: HeadingOptions) Node {
    const size: f32 = switch (options.level) {
        1 => 30,
        2 => 24,
        3 => 20,
        4 => 16,
        else => 14,
    };
    return text(value, .{
        .key = options.key,
        .size = size,
        .weight = options.weight,
        .color = options.color,
        .text_align = options.text_align,
        .wrap = false,
    });
}

pub fn title(value: []const u8, options: HeadingOptions) Node {
    return heading(value, options);
}

pub const ParagraphOptions = struct {
    key: ?[]const u8 = null,
    size: f32 = 14,
    color: ?u32 = null,
    text_align: text_mod.Align = .left,
    line_height: f32 = 1.5,
};

pub fn paragraph(value: []const u8, options: ParagraphOptions) Node {
    return text(value, .{
        .key = options.key,
        .size = options.size,
        .color = options.color orelse default_theme.color_text_secondary,
        .text_align = options.text_align,
        .wrap = true,
        .line_height = options.line_height,
    });
}

pub const ButtonOptions = struct {
    key: ?[]const u8 = null,
    text: []const u8 = "",
    variant: node_mod.ButtonType = .default,
    size: node_mod.ButtonSize = .middle,
    disabled: bool = false,
    width: ?f32 = null,
    font_size: ?f32 = null,
    text_align: text_mod.Align = .center,
    label_padding: f32 = 0,
    text_color: ?u32 = null,
    icon: node_mod.ButtonIcon = .none,
    on_click: ?actions.Action = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
};

pub fn button(options: ButtonOptions) Node {
    const height: f32 = switch (options.size) {
        .small => 24,
        .middle => 32,
        .large => 40,
    };
    return make(.{
        .kind = .button,
        .key = options.key,
        .content = options.text,
        .button_type = options.variant,
        .button_size = options.size,
        .enabled = !options.disabled,
        .focusable = true,
        .layout = .{ .height = height, .width = options.width, .shrink = 0 },
        .visual = .{ .radius = default_theme.border_radius },
        .text_style = .{ .font_size = options.font_size orelse 14, .text_align = options.text_align },
        .text_color = options.text_color,
        .label_padding = options.label_padding,
        .icon = options.icon,
        .on_click = options.on_click,
        .index_action = options.index_action,
        .index = options.index,
    });
}

pub const LinkOptions = struct {
    key: ?[]const u8 = null,
    text: []const u8 = "",
    on_click: ?actions.Action = null,
};

pub fn link(options: LinkOptions) Node {
    return button(.{
        .key = options.key,
        .text = options.text,
        .variant = .link,
        .on_click = options.on_click,
    });
}

pub const InputOptions = struct {
    key: ?[]const u8 = null,
    value: ?[]const u8 = null,
    placeholder: []const u8 = "",
    disabled: bool = false,
    read_only: bool = false,
    password: bool = false,
    multiline: bool = false,
    focus_ring: bool = true,
    width: ?f32 = null,
    height: f32 = 32,
    font_size: f32 = 14,
    line_height: f32 = 0,
    on_change: ?actions.TextAction = null,
    on_submit: ?actions.Action = null,
};

pub fn input(options: InputOptions) Node {
    return make(.{
        .kind = .input,
        .key = options.key,
        .content = options.value orelse "",
        .placeholder = options.placeholder,
        .controlled = options.value != null,
        .read_only = options.read_only,
        .password = options.password,
        .multiline = options.multiline,
        .focus_ring = options.focus_ring,
        .enabled = !options.disabled,
        .focusable = true,
        .layout = .{ .height = options.height, .width = options.width, .shrink = 0 },
        .visual = .{ .radius = default_theme.border_radius },
        .text_style = .{ .font_size = options.font_size, .line_height = options.line_height },
        .on_text = options.on_change,
        .on_click = options.on_submit,
    });
}

pub const ToggleOptions = struct {
    key: ?[]const u8 = null,
    label: []const u8 = "",
    checked: bool = false,
    disabled: bool = false,
    font_size: f32 = 14,
    on_change: ?actions.BoolAction = null,
};

pub fn checkbox(options: ToggleOptions) Node {
    return make(.{
        .kind = .toggle,
        .key = options.key,
        .content = options.label,
        .toggle_kind = .checkbox,
        .checked = options.checked,
        .enabled = !options.disabled,
        .focusable = true,
        .layout = .{ .height = 24, .shrink = 0 },
        .text_style = .{ .font_size = options.font_size },
        .on_bool = options.on_change,
    });
}

pub fn @"switch"(options: ToggleOptions) Node {
    return make(.{
        .kind = .toggle,
        .key = options.key,
        .content = options.label,
        .toggle_kind = .switch_control,
        .checked = options.checked,
        .enabled = !options.disabled,
        .focusable = true,
        .layout = .{ .height = 24, .shrink = 0 },
        .text_style = .{ .font_size = options.font_size },
        .on_bool = options.on_change,
    });
}

pub const RadioOption = struct {
    label: []const u8,
    disabled: bool = false,
};

pub const RadioGroupOptions = struct {
    key: ?[]const u8 = null,
    options: []const RadioOption = &.{},
    selected: usize = 0,
    direction: flex.Direction = .column,
    gap: f32 = 8,
    on_change: ?actions.IndexAction = null,
};

pub fn radioGroup(options: RadioGroupOptions) Node {
    const children = allocSlice(Node, options.options.len);
    const base = options.key orelse "radio-group";
    for (options.options, 0..) |option, i| {
        const key = std.fmt.allocPrint(frame_arena.allocator(), "{s}-{d}", .{ base, i }) catch
            @panic("Zant UI: out of memory while building a form");
        children[i] = make(.{
            .kind = .toggle,
            .key = key,
            .content = option.label,
            .toggle_kind = .radio,
            .checked = i == options.selected,
            .enabled = !option.disabled,
            .focusable = true,
            .layout = .{ .height = 24, .shrink = 0 },
            .index_action = options.on_change,
            .index = i,
        });
    }
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = options.direction,
            .gap = options.gap,
        },
        .children = children,
    });
}

pub const TabItem = struct {
    label: []const u8,
    content: Node,
};

pub const TabsOptions = struct {
    key: ?[]const u8 = null,
    items: []const TabItem = &.{},
    active: usize = 0,
    on_change: ?actions.IndexAction = null,
};

pub fn tabs(options: TabsOptions) Node {
    if (options.items.len == 0) return column(.{ .key = options.key });
    const active = @min(options.active, options.items.len - 1);
    const tab_children = allocSlice(Node, options.items.len);
    const base = options.key orelse "tab";
    for (options.items, 0..) |item, i| {
        const key = std.fmt.allocPrint(frame_arena.allocator(), "{s}-{d}", .{ base, i }) catch
            @panic("Zant UI: out of memory while building tabs");
        tab_children[i] = button(.{
            .key = key,
            .text = item.label,
            .variant = if (i == active) .link else .text,
            .index_action = options.on_change,
            .index = i,
        });
    }
    const children = allocSlice(Node, 3);
    children[0] = row(.{ .gap = 4, .children = tab_children });
    children[1] = divider(.{ .margin = .{ .top = 4, .bottom = 4 } });
    children[2] = options.items[active].content;
    return column(.{ .key = options.key, .gap = 0, .children = children });
}

pub const TooltipOptions = struct {
    text: []const u8,
    child: Node,
};

pub fn tooltip(options: TooltipOptions) Node {
    const copy = alloc(Element);
    copy.* = options.child.element.*;
    copy.tooltip = copyString(options.text);
    return .{ .element = copy };
}

pub const ImageOptions = struct {
    key: ?[]const u8 = null,
    src: []const u8 = "",
    width: ?f32 = null,
    height: ?f32 = null,
    fit: node_mod.ImageFit = .cover,
    radius: f32 = 8,
    tooltip: ?[]const u8 = null,
};

pub fn image(options: ImageOptions) Node {
    return make(.{
        .kind = .image,
        .key = options.key,
        .content = options.src,
        .image_fit = options.fit,
        .layout = .{ .width = options.width, .height = options.height, .shrink = 0 },
        .visual = .{ .radius = options.radius, .clip = true },
        .tooltip = options.tooltip,
    });
}

pub const SelectOptions = struct {
    key: ?[]const u8 = null,
    options: []const []const u8 = &.{},
    selected: usize = 0,
    width: ?f32 = null,
    height: f32 = 32,
    disabled: bool = false,
    tooltip: ?[]const u8 = null,
    on_change: ?actions.IndexAction = null,
};

pub fn select(options: SelectOptions) Node {
    return make(.{
        .kind = .select,
        .key = options.key,
        .options = options.options,
        .selected = options.selected,
        .enabled = !options.disabled,
        .focusable = true,
        .layout = .{ .width = options.width, .height = options.height, .shrink = 0 },
        .visual = .{ .radius = default_theme.border_radius },
        .tooltip = options.tooltip,
        .index_action = options.on_change,
        .index = options.selected,
    });
}

const PdfCopyContext = struct {
    src: [:0]const u8,
    page: usize,
};

fn copyPdfText(ctx: *PdfCopyContext) void {
    var buf: [64 * 1024]u8 = undefined;
    if (pdf_mod.pageText(ctx.src, ctx.page, &buf)) |page_text| {
        if (page_text.len > 0) _ = platform_mod.setClipboard(page_text);
    }
}

pub const RichTextOptions = struct {
    key: ?[]const u8 = null,
    spans: []const node_mod.RichSpan = &.{},
    width: ?f32 = null,
    height: ?f32 = null,
};

pub fn richText(options: RichTextOptions) Node {
    return make(.{
        .kind = .rich_text,
        .key = options.key,
        .rich_spans = options.spans,
        .layout = .{
            .width = options.width,
            .height = options.height,
            .shrink = 0,
            .width_percent = if (options.width == null) 100 else null,
        },
    });
}

pub const PdfOptions = struct {
    key: ?[]const u8 = null,
    src: []const u8,
    page: usize = 0,
    width: f32 = 320,
    height: f32 = 420,
    show_toolbar: bool = true,
    background: ?u32 = null,
    on_page_change: ?actions.IndexAction = null,
};

/// Native macOS PDF preview backed by PDFKit. Interaction is inspired by
/// EmbedPDF, but this remains a native Skia UI (no WebView).
pub fn pdfPreview(options: PdfOptions) Node {
    const src_z = std.fmt.allocPrintSentinel(frame_arena.allocator(), "{s}", .{options.src}, 0) catch
        @panic("Zant UI: out of memory while opening PDF");
    const count = pdf_mod.pageCount(src_z);
    const page = if (count == 0) 0 else @min(options.page, count - 1);
    var path_buf: [std.fs.max_path_bytes]u8 = undefined;
    const rendered = if (count > 0) pdf_mod.renderPage(
        src_z,
        page,
        @intFromFloat(@max(1, options.width * 2)),
        @intFromFloat(@max(1, options.height * 2)),
        &path_buf,
    ) else null;

    const image_node: Node = if (rendered) |path|
        image(.{
            .src = path,
            .width = options.width,
            .height = options.height,
            .fit = .contain,
            .radius = 4,
        })
    else
        column(.{
            .width = options.width,
            .height = options.height,
            .background = 0xfff5f5f5,
            .radius = 4,
            .children = &.{text("无法预览 PDF", .{ .size = 13, .color = 0xa6000000 })},
        });

    if (!options.show_toolbar) return image_node;

    const prev_disabled = page == 0 or count == 0;
    const next_disabled = count == 0 or page + 1 >= count;
    const copy_ctx = alloc(PdfCopyContext);
    copy_ctx.* = .{ .src = src_z, .page = page };
    const toolbar = row(.{
        .gap = 6,
        .align_items = .center,
        .children = &.{
            button(.{ .key = "pdf-first", .text = "|‹", .size = .small, .disabled = prev_disabled, .index_action = options.on_page_change, .index = 0 }),
            button(.{ .key = "pdf-prev", .text = "‹", .size = .small, .disabled = prev_disabled, .index_action = options.on_page_change, .index = if (page > 0) page - 1 else 0 }),
            textFmt("{d} / {d}", .{ if (count == 0) 0 else page + 1, count }, .{ .size = 12, .color = 0xa6000000 }),
            button(.{ .key = "pdf-next", .text = "›", .size = .small, .disabled = next_disabled, .index_action = options.on_page_change, .index = page + 1 }),
            button(.{ .key = "pdf-last", .text = "›|", .size = .small, .disabled = next_disabled, .index_action = options.on_page_change, .index = if (count > 0) count - 1 else 0 }),
            button(.{ .key = "pdf-copy", .text = "复制文本", .size = .small, .disabled = count == 0, .on_click = action(copy_ctx, copyPdfText) }),
        },
    });

    return column(.{
        .key = options.key,
        .gap = 8,
        .padding = inset(8),
        .background = options.background,
        .radius = 8,
        .children = &.{ toolbar, image_node },
    });
}

pub const DividerOptions = struct {
    key: ?[]const u8 = null,
    margin: geometry.Edges = .{},
    color: ?u32 = null,
};

pub fn divider(options: DividerOptions) Node {
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{ .height = 1, .margin = options.margin, .shrink = 0 },
        .visual = .{ .background = options.color orelse default_theme.color_border_secondary },
    });
}

pub const TagOptions = struct {
    key: ?[]const u8 = null,
    text: []const u8 = "",
    color: ?u32 = null,
    background: ?u32 = null,
};

pub fn tag(options: TagOptions) Node {
    const children = allocSlice(Node, 1);
    children[0] = text(options.text, .{
        .size = 12,
        .color = options.color orelse default_theme.color_text,
        .wrap = false,
    });
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = .row,
            .align_items = .center,
            .padding = insetXY(8, 2),
            .shrink = 0,
        },
        .visual = .{
            .background = options.background orelse default_theme.color_fill_secondary,
            .radius = 4,
        },
        .children = children,
    });
}

pub const CardOptions = struct {
    key: ?[]const u8 = null,
    title: ?[]const u8 = null,
    extra: ?[]const u8 = null,
    children: []const Node = &.{},
    width: ?f32 = null,
    height: ?f32 = null,
    gap: f32 = 12,
    padding: geometry.Edges = .all(20),
    bordered: bool = true,
};

pub fn card(options: CardOptions) Node {
    const has_header = options.title != null or options.extra != null;
    const total = options.children.len + (if (has_header) @as(usize, 1) else 0);
    const children = allocSlice(Node, total);
    var index: usize = 0;
    if (has_header) {
        const header_children = allocSlice(Node, 3);
        header_children[0] = text(options.title orelse "", .{
            .size = 16,
            .weight = .semibold,
            .color = default_theme.color_text,
            .wrap = false,
        });
        header_children[1] = column(.{ .grow = 1, .basis = 0 });
        header_children[2] = text(options.extra orelse "", .{
            .size = 12,
            .color = default_theme.color_text_secondary,
            .wrap = false,
        });
        children[index] = row(.{ .align_items = .center, .gap = 8, .children = header_children });
        index += 1;
    }
    if (options.children.len > 0) {
        @memcpy(children[index..], options.children);
    }
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = .column,
            .align_items = .stretch,
            .gap = options.gap,
            .padding = options.padding,
            .width = options.width,
            .height = options.height,
            .shrink = 0,
        },
        .visual = .{
            .background = default_theme.color_bg_container,
            .border_color = if (options.bordered) default_theme.color_border_secondary else null,
            .border_width = if (options.bordered) 1 else 0,
            .radius = 10,
        },
        .children = children,
    });
}

pub const AlertType = enum {
    info,
    success,
    warning,
    error_state,
};

pub const AlertOptions = struct {
    key: ?[]const u8 = null,
    type: AlertType = .info,
    title: []const u8 = "",
    description: []const u8 = "",
};

pub fn alert(options: AlertOptions) Node {
    const color: u32 = switch (options.type) {
        .info => default_theme.color_primary,
        .success => default_theme.color_success,
        .warning => default_theme.color_warning,
        .error_state => default_theme.color_error,
    };
    const count: usize = if (options.description.len > 0) 2 else 1;
    const children = allocSlice(Node, count);
    children[0] = text(options.title, .{
        .size = 14,
        .weight = .medium,
        .color = default_theme.color_text,
        .wrap = true,
    });
    if (count == 2) {
        children[1] = text(options.description, .{
            .size = 13,
            .color = default_theme.color_text_secondary,
            .wrap = true,
        });
    }
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = .column,
            .gap = 4,
            .padding = insetXY(14, 10),
        },
        .visual = .{
            .background = tokens.withAlpha(color, 0x0f),
            .border_color = tokens.withAlpha(color, 0x40),
            .border_width = 1,
            .radius = 8,
        },
        .children = children,
    });
}

pub const AvatarOptions = struct {
    key: ?[]const u8 = null,
    text: []const u8 = "?",
    size: f32 = 32,
    background: ?u32 = null,
    color: ?u32 = null,
};

pub fn avatar(options: AvatarOptions) Node {
    const children = allocSlice(Node, 1);
    children[0] = text(options.text, .{
        .size = @max(12, options.size * 0.4),
        .weight = .medium,
        .color = options.color orelse default_theme.color_primary,
        .text_align = .center,
        .wrap = false,
    });
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = .row,
            .align_items = .center,
            .justify = .center,
            .width = options.size,
            .height = options.size,
        },
        .visual = .{
            .background = options.background orelse default_theme.color_primary_bg,
            .radius = options.size * 0.5,
            .clip = true,
        },
        .children = children,
    });
}

pub const ProgressOptions = struct {
    key: ?[]const u8 = null,
    percent: f32 = 0,
    width: f32 = 200,
    height: f32 = 8,
    color: ?u32 = null,
};

pub fn progress(options: ProgressOptions) Node {
    const percent = @max(0, @min(100, options.percent));
    const children = allocSlice(Node, 1);
    children[0] = make(.{
        .kind = .container,
        .layout = .{
            .width = @max(0, options.width * percent / 100.0),
            .height = options.height,
        },
        .visual = .{
            .background = options.color orelse default_theme.color_primary,
            .radius = options.height * 0.5,
        },
    });
    return make(.{
        .kind = .container,
        .key = options.key,
        .layout = .{
            .direction = .row,
            .align_items = .center,
            .width = options.width,
            .height = options.height,
        },
        .visual = .{
            .background = default_theme.color_fill_secondary,
            .radius = options.height * 0.5,
            .clip = true,
        },
        .children = children,
    });
}
