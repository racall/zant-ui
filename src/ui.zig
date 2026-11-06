const std = @import("std");
const geometry = @import("core/geometry.zig");
const node_mod = @import("core/node.zig");
const actions = @import("core/actions.zig");
const events = @import("core/events.zig");
const icons_mod = @import("core/icons.zig");
const flex = @import("layout/flex.zig");
const tokens = @import("theme/tokens.zig");
const text_mod = @import("renderer/text.zig");
const pdf_mod = @import("platform/pdf.zig");
const platform_mod = @import("platform/platform.zig");
const motion = @import("motion/motion.zig");

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
    icon_size: f32 = 18,
    on_click: ?actions.Action = null,
    on_bool: ?actions.BoolAction = null,
    on_text: ?actions.TextAction = null,
    on_undo: ?actions.Action = null,
    on_redo: ?actions.Action = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    tooltip: ?[]const u8 = null,
    image_fit: node_mod.ImageFit = .cover,
    options: []const []const u8 = &.{},
    selected: usize = 0,
    children: []const Node = &.{},
    rich_spans: []const node_mod.RichSpan = &.{},
    /// Rich text line height multiplier (0 = font default).
    rich_line_height: f32 = 2.0,
    /// Rich text link activation; receives the clicked span's `link`.
    on_link: ?actions.TextAction = null,
    /// Controlled input selection `(anchor, cursor)`; applied when it changes.
    selection: ?[2]usize = null,
    /// Input selection change notifications `(anchor, cursor)`.
    on_selection: ?actions.RangeAction = null,
    /// Input key hook (runs before the input's own editing keys).
    on_key: ?actions.InputKeyAction = null,
    /// Centred text column width for wide multiline inputs (0 = full width).
    content_max_width: f32 = 0,
    /// Vector drawing (`ui.vector`).
    drawing: ?*const node_mod.vector.Drawing = null,
    /// Memoized subtree: when the hash equals the mounted node's, the
    /// reconciler keeps that subtree without calling `memo_build`.
    memo_hash: ?u64 = null,
    memo_build: ?MemoBuild = null,
    /// Enter / exit and layout transitions (`ui.styled(.., .{ .motion = .. })`).
    motion: node_mod.MotionStyle = .{},
};

pub const MemoBuild = struct {
    ctx: *const anyopaque,
    func: *const fn (ctx: *const anyopaque) Node,
};

threadlocal var frame_arena = std.heap.ArenaAllocator.init(std.heap.c_allocator);
threadlocal var frame_active = false;

/// Theme used by `ui.*` builders for their default colours. The runtime sets
/// it before every build from the active theme (light/dark).
threadlocal var build_theme: tokens.Theme = .{};

pub fn setBuildTheme(theme: tokens.Theme) void {
    build_theme = theme;
}

threadlocal var build_tree: ?*const @import("core/tree.zig").Tree = null;

/// Retained tree of the runtime being built (set by the runtime).
pub fn setBuildTree(tree: ?*const @import("core/tree.zig").Tree) void {
    build_tree = tree;
}

/// Window-space box (logical px) of the node with `key` as laid out in the
/// previous frame, or null (first frame, key not shown). Use it to place a
/// popover next to its trigger from the root container:
///
///     if (ui.keyBounds("menu-btn")) |b| ui.styled(menu, .{ .absolute = true, .left = b.x, .top = b.bottom() + 4 })
pub fn keyBounds(key: []const u8) ?geometry.Rect {
    const tree = build_tree orelse return null;
    return tree.boundsOfKey(key);
}

/// Active theme while building a frame.
pub fn currentTheme() tokens.Theme {
    return build_theme;
}

threadlocal var frame_now_ms: i64 = 0;
threadlocal var pending_wake_ms: ?u32 = null;
threadlocal var pending_edited: ?bool = null;

/// Declares whether the document has unsaved changes (call it from
/// `render`). On macOS this shows the dot in the window's close button,
/// like every document app; the runtime applies it after the build.
threadlocal var build_window_style: platform_mod.WindowStyle = .native;

/// Window style of the app being built (set by the runtime before render).
pub fn setBuildWindowStyle(style: platform_mod.WindowStyle) void {
    build_window_style = style;
}

/// Space an app-drawn title bar (`.hidden_titlebar`) must keep free for the
/// system's window controls: the traffic lights on the left on macOS, the
/// caption buttons on the right on Windows. Zero for other window styles.
///
///     const insets = ui.titlebarInsets();
///     ui.row(.{ .padding = .{ .left = 16 + insets.left, .right = 16 + insets.right }, ... })
pub fn titlebarInsets() platform_mod.TitlebarInsets {
    return platform_mod.titlebarInsets(build_window_style);
}

/// Copy of a container with `extra` appended to its children (overlays the
/// runtime adds on top of the app's root, e.g. Windows caption buttons).
pub fn appendChildren(node: Node, extra: []const Node) Node {
    if (extra.len == 0) return node;
    const copy = alloc(Element);
    copy.* = node.element.*;
    const children = allocSlice(Node, node.element.children.len + extra.len);
    @memcpy(children[0..node.element.children.len], node.element.children);
    @memcpy(children[node.element.children.len..], extra);
    copy.children = children;
    return .{ .element = copy };
}

pub fn documentEdited(edited: bool) void {
    pending_edited = edited;
}

/// The value declared by the last build, if any (runtime).
pub fn takeDocumentEdited() ?bool {
    const v = pending_edited;
    pending_edited = null;
    return v;
}

pub const CommandKind = enum { focus, scroll_into_view };

/// Imperative request queued by `focus` / `scrollIntoView`; the runtime
/// runs it after the next layout.
pub const Command = struct {
    kind: CommandKind,
    key_buf: [96]u8 = undefined,
    key_len: u8 = 0,

    pub fn key(self: *const Command) []const u8 {
        return self.key_buf[0..self.key_len];
    }
};

pub const max_commands = 8;
threadlocal var command_queue: [max_commands]Command = undefined;
threadlocal var command_count: usize = 0;

fn queueCommand(kind: CommandKind, node_key: []const u8) void {
    if (command_count >= max_commands or node_key.len > 96) return;
    var cmd: Command = .{ .kind = kind, .key_len = @intCast(node_key.len) };
    @memcpy(cmd.key_buf[0..node_key.len], node_key);
    command_queue[command_count] = cmd;
    command_count += 1;
}

/// Focuses the focusable node with `key` after the next layout (call it
/// from an action, e.g. to focus a search field when it opens).
pub fn focus(node_key: []const u8) void {
    queueCommand(.focus, node_key);
}

/// Scrolls the nearest scroll container so the node with `key` is at its
/// top (animated unless motion is off).
pub fn scrollIntoView(node_key: []const u8) void {
    queueCommand(.scroll_into_view, node_key);
}

/// Moves the queued commands into `out` (runtime).
pub fn takeCommands(out: *[max_commands]Command) []Command {
    const n = command_count;
    @memcpy(out[0..n], command_queue[0..n]);
    command_count = 0;
    return out[0..n];
}

/// Requests a rebuild after `ms` milliseconds (call it from `render`; e.g.
/// to hide a toast). Valid for the build that requested it.
pub fn wakeAfter(ms: u32) void {
    pending_wake_ms = if (pending_wake_ms) |cur| @min(cur, ms) else ms;
}

/// The wake-up requested by the last build (runtime).
pub fn takeWake() ?u32 {
    const v = pending_wake_ms;
    pending_wake_ms = null;
    return v;
}

/// Sets the timestamp `nowMs` reports (the runtime samples it per frame).
pub fn setNow(ms: i64) void {
    frame_now_ms = ms;
}

/// Timestamp (ms, monotonic) of the current frame or event. Use it for
/// time-based UI state (toasts, timers) together with `wakeAfter`.
pub fn nowMs() i64 {
    return frame_now_ms;
}

pub fn beginFrame() void {
    _ = frame_arena.reset(.retain_capacity);
    pending_wake_ms = null;
    pending_edited = null;
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

/// Allocator whose memory lives until the next `beginFrame`.
///
/// Use it for small per-frame callback contexts (e.g. a copy button that needs
/// the text it copies). Callbacks fire before the next frame is built, so the
/// memory stays valid for every event dispatched against this frame.
pub fn frameAllocator() std.mem.Allocator {
    return frame_arena.allocator();
}

/// Copies `value` into the frame arena.
pub fn frameString(value: []const u8) []const u8 {
    return copyString(value);
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

pub fn rangeAction(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), usize, usize) void) actions.RangeAction {
    const Adapter = struct {
        fn call(raw: *anyopaque, start: usize, end: usize) void {
            func(@ptrCast(@alignCast(raw)), start, end);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub fn inputKeyAction(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), events.KeyEvent, []const u8, usize, usize) bool) actions.InputKeyAction {
    const Adapter = struct {
        fn call(raw: *anyopaque, key: events.KeyEvent, value: []const u8, anchor: usize, cursor: usize) bool {
            return func(@ptrCast(@alignCast(raw)), key, value, anchor, cursor);
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
    shadow_color: ?u32 = null,
    shadow_blur: ?f32 = null,
    shadow_offset_y: ?f32 = null,
    clip: ?bool = null,
    block_pointer: ?bool = null,
    text_color: ?u32 = null,
    font_size: ?f32 = null,
    font_weight: ?text_mod.Weight = null,
    text_align: ?text_mod.Align = null,
    line_height: ?f32 = null,
    wrap: ?bool = null,
    /// Enter / exit transition (antd presets, see `docs/motion.md`). Give
    /// the node a `key` so a re-added node revives instead of duplicating.
    motion: ?motion.Presence = null,
    /// Transform origin for zoom / slide presets.
    motion_origin: ?motion.Origin = null,
    /// Animate position / size changes (leaf containers).
    layout_transition: ?motion.Transition = null,
    /// Absolute children: follow the n-th in-flow sibling's box.
    anchor: ?u16 = null,
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
    if (style.shadow_color) |value| copy.visual.shadow_color = value;
    if (style.shadow_blur) |value| copy.visual.shadow_blur = value;
    if (style.shadow_offset_y) |value| copy.visual.shadow_offset_y = value;
    if (style.clip) |value| copy.visual.clip = value;
    if (style.block_pointer) |value| copy.visual.block_pointer = value;
    if (style.motion) |value| copy.motion.presence = value;
    if (style.motion_origin) |value| copy.motion.origin = value;
    if (style.layout_transition) |value| copy.motion.layout_transition = value;
    if (style.anchor) |value| copy.layout.anchor = value;
    if (style.color orelse style.text_color) |value| {
        copy.text_style.color = value;
        copy.text_color = value;
    }
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
            spans[i].link = copyString(span.link);
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
        .scroll = box.scroll,
    };
}

fn visualFrom(box: BoxOptions) node_mod.Visual {
    return .{
        .background = box.background,
        .border_color = box.border_color,
        .border_width = box.border_width,
        .radius = box.radius,
        .clip = box.clip or box.scroll,
        .block_pointer = box.block_pointer,
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
    /// Vertical scroll container: children keep their natural height, overflow
    /// is clipped and the wheel/trackpad scrolls the content. Needs a definite
    /// height (`height`, `height_percent`, `grow` inside a sized parent, ...).
    scroll: bool = false,
    /// Swallow clicks on empty areas (modal backdrops, popover panels).
    block_pointer: bool = false,
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
        .text_style = textStyle(options, build_theme.color_text, false),
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
        .color = options.color orelse build_theme.color_text_secondary,
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
    icon_size: f32 = 18,
    on_click: ?actions.Action = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    /// false: clicking does not take keyboard focus (toolbar buttons that
    /// act on the focused input).
    focusable: bool = true,
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
        .focusable = options.focusable,
        .layout = .{ .height = height, .width = options.width, .shrink = 0 },
        .visual = .{ .radius = build_theme.border_radius },
        .text_style = .{ .font_size = options.font_size orelse switch (options.size) {
            .small => 12,
            .middle => 14,
            .large => 16,
        }, .text_align = options.text_align },
        .text_color = options.text_color,
        .label_padding = options.label_padding,
        .icon = options.icon,
        .icon_size = options.icon_size,
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
    on_undo: ?actions.Action = null,
    on_redo: ?actions.Action = null,
    /// Controlled selection `(anchor, cursor)` in UTF-8 bytes. Applied whenever
    /// the value differs from the previously applied one.
    selection: ?[2]usize = null,
    on_selection: ?actions.RangeAction = null,
    /// Key hook (list continuation, shortcuts); return true to consume.
    on_key: ?actions.InputKeyAction = null,
    /// Multiline: keep the text in a centred column of at most this width
    /// while the input (and its scroll bar) spans the full width.
    content_max_width: ?f32 = null,
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
        .visual = .{ .radius = build_theme.border_radius },
        .text_style = .{ .font_size = options.font_size, .line_height = options.line_height },
        .on_text = options.on_change,
        .on_click = options.on_submit,
        .on_undo = options.on_undo,
        .on_redo = options.on_redo,
        .selection = options.selection,
        .on_selection = options.on_selection,
        .on_key = options.on_key,
        .content_max_width = options.content_max_width orelse 0,
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
    // ink bar: follows the active tab (antd: 300ms ease)
    const row_children = allocSlice(Node, options.items.len + 1);
    @memcpy(row_children[0..options.items.len], tab_children);
    row_children[options.items.len] = styled(column(.{
        .key = std.fmt.allocPrint(frame_arena.allocator(), "{s}-ink", .{base}) catch base,
        .children = &.{},
    }), .{
        .absolute = true,
        .anchor = @intCast(active),
        .bottom = 0,
        .height = 2,
        .background = build_theme.color_primary,
        .layout_transition = .{ .duration_ms = 300, .easing = .ease },
    });
    const children = allocSlice(Node, 3);
    children[0] = row(.{ .gap = 4, .children = row_children });
    children[1] = divider(.{ .margin = .{ .top = 4, .bottom = 4 } });
    children[2] = options.items[active].content;
    return column(.{ .key = options.key, .gap = 0, .children = children });
}

pub const TooltipOptions = struct {
    text: []const u8,
    child: Node,
};

/// Returns `node` with `key` set (used to key memoized subtrees).
pub fn withKey(node: Node, key: []const u8) Node {
    const copy = alloc(Element);
    copy.* = node.element.*;
    copy.key = copyString(key);
    return .{ .element = copy };
}

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
        .visual = .{ .radius = build_theme.border_radius },
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

pub const Icon = icons_mod.Icon;

pub const IconOptions = struct {
    key: ?[]const u8 = null,
    name: Icon,
    size: f32 = 18,
    color: ?u32 = null,
    /// Stroke width in px (default scales with size: 1.5px at 18px).
    stroke: ?f32 = null,
};

/// A line icon from the Zant icon set (24×24 grid, uniform stroke).
pub fn icon(options: IconOptions) Node {
    const arena = frame_arena.allocator();
    const d = arena.create(node_mod.vector.Drawing) catch @panic("Zant UI: out of memory while building a frame");
    d.* = icons_mod.drawing(arena, options.name, options.size, options.color orelse build_theme.color_text, options.stroke) catch
        @panic("Zant UI: out of memory while building a frame");
    return vector(.{ .key = options.key, .drawing = d });
}

pub const IconButtonOptions = struct {
    key: ?[]const u8 = null,
    icon: Icon,
    /// Icon size; the button is `box` × `box`.
    size: f32 = 18,
    box: f32 = 32,
    color: ?u32 = null,
    /// Draws `active_background` behind the icon (toggled state).
    active: bool = false,
    active_background: ?u32 = null,
    disabled: bool = false,
    /// Toolbar buttons should not steal focus from the input they act on.
    focusable: bool = false,
    tooltip: ?[]const u8 = null,
    on_click: ?actions.Action = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
};

/// Square icon-only button with hover feedback.
pub fn iconButton(options: IconButtonOptions) Node {
    var node = styled(button(.{
        .key = options.key,
        .text = "",
        .variant = .text,
        .size = .small,
        .icon = options.icon,
        .icon_size = options.size,
        .text_color = options.color,
        .disabled = options.disabled,
        .focusable = options.focusable,
        .on_click = options.on_click,
        .index_action = options.index_action,
        .index = options.index,
    }), .{
        .width = options.box,
        .height = options.box,
        .padding = .{},
        .radius = options.box * 0.28,
        .background = if (options.active) options.active_background orelse build_theme.color_fill_secondary else null,
    });
    if (options.tooltip) |t| node = tooltip(.{ .text = t, .child = node });
    return node;
}

pub const DismissLayerOptions = struct {
    key: ?[]const u8 = "dismiss-layer",
    on_dismiss: ?actions.Action = null,
    /// Optional dimming colour (transparent by default).
    background: ?u32 = null,
};

/// Full-size invisible click catcher placed right before a popover/menu in
/// the same container: a click anywhere outside the popover calls
/// `on_dismiss`. Paints nothing unless `background` is set.
pub fn dismissLayer(options: DismissLayerOptions) Node {
    return styled(button(.{
        .key = options.key,
        .text = "",
        .variant = .plain,
        .focusable = false,
        .on_click = options.on_dismiss,
    }), .{
        .absolute = true,
        .left = 0,
        .top = 0,
        .width_percent = 100,
        .height_percent = 100,
        .radius = 0,
        .background = options.background,
    });
}

pub const VectorOptions = struct {
    key: ?[]const u8 = null,
    drawing: *const node_mod.vector.Drawing,
    /// Fixed box width (default: the drawing's width). The drawing is
    /// left-aligned; wrap in a row to center it.
    width: ?f32 = null,
};

/// A retained vector drawing (math formulas, diagrams). Its size is the
/// drawing's extent; the drawing is copied into the node on mount/change.
pub fn vector(options: VectorOptions) Node {
    const copy = alloc(node_mod.vector.Drawing);
    copy.* = options.drawing.*;
    return make(.{
        .kind = .vector,
        .key = options.key,
        .drawing = copy,
        .layout = .{ .width = options.width orelse options.drawing.width, .height = options.drawing.height(), .shrink = 0 },
    });
}

pub const MemoOptions = struct {
    key: ?[]const u8 = null,
    /// Must change whenever anything the builder reads changes.
    hash: u64,
};

/// React.memo for Zant: `build(ctx)` runs only when no mounted node with this
/// position/key has the same `hash`; otherwise the existing subtree (layout,
/// text shaping, callbacks) is kept untouched. `build` must return a
/// container (`ui.column`/`ui.row`), and `ctx` plus every callback context
/// inside the subtree must outlive the frame (no `frameAllocator` data).
pub fn memo(options: MemoOptions, ctx: anytype, comptime build: fn (@TypeOf(ctx)) Node) Node {
    const Ctx = @TypeOf(ctx);
    const Adapter = struct {
        fn call(raw: *const anyopaque) Node {
            const typed: Ctx = @ptrCast(@alignCast(@constCast(raw)));
            return build(typed);
        }
    };
    return make(.{
        .kind = .container,
        .key = options.key,
        .memo_hash = options.hash,
        .memo_build = .{ .ctx = @ptrCast(ctx), .func = Adapter.call },
    });
}

pub const RichTextOptions = struct {
    key: ?[]const u8 = null,
    spans: []const node_mod.RichSpan = &.{},
    width: ?f32 = null,
    height: ?f32 = null,
    /// Line height multiplier; 0 uses the font's natural line height.
    line_height: f32 = 2.0,
    /// Called with `RichSpan.link` when a linked span is clicked.
    on_link: ?actions.TextAction = null,
};

pub fn richText(options: RichTextOptions) Node {
    return make(.{
        .kind = .rich_text,
        .key = options.key,
        .rich_spans = options.spans,
        .rich_line_height = options.line_height,
        .on_link = options.on_link,
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

/// Native PDF preview backed by PDFKit (macOS) or PDFium (Windows). Interaction is inspired by
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
        .visual = .{ .background = options.color orelse build_theme.color_border_secondary },
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
        .color = options.color orelse build_theme.color_text,
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
            .background = options.background orelse build_theme.color_fill_secondary,
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
            .color = build_theme.color_text,
            .wrap = false,
        });
        header_children[1] = column(.{ .grow = 1, .basis = 0 });
        header_children[2] = text(options.extra orelse "", .{
            .size = 12,
            .color = build_theme.color_text_secondary,
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
            .background = build_theme.color_bg_container,
            .border_color = if (options.bordered) build_theme.color_border_secondary else null,
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
        .info => build_theme.color_primary,
        .success => build_theme.color_success,
        .warning => build_theme.color_warning,
        .error_state => build_theme.color_error,
    };
    const count: usize = if (options.description.len > 0) 2 else 1;
    const children = allocSlice(Node, count);
    children[0] = text(options.title, .{
        .size = 14,
        .weight = .medium,
        .color = build_theme.color_text,
        .wrap = true,
    });
    if (count == 2) {
        children[1] = text(options.description, .{
            .size = 13,
            .color = build_theme.color_text_secondary,
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
        // light primary tint by default; on a custom background white text
        // (antd), so the label never takes the background's colour
        .color = options.color orelse if (options.background != null) 0xffffffff else build_theme.color_primary,
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
            .background = options.background orelse build_theme.color_primary_bg,
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
            .background = options.color orelse build_theme.color_primary,
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
            .background = build_theme.color_fill_secondary,
            .radius = options.height * 0.5,
            .clip = true,
        },
        .children = children,
    });
}

// ── composite components (Ant Design style) ─────────────────────────────

const window_controls_mod = @import("components/window_controls.zig");
pub const WindowControlsOptions = window_controls_mod.WindowControlsOptions;
/// Windows caption buttons (added by the runtime for `.hidden_titlebar`).
pub const windowControls = window_controls_mod.windowControls;
pub const caption_maximize_key = window_controls_mod.caption_maximize_key;
const menu_mod = @import("components/menu.zig");
const segmented_mod = @import("components/segmented.zig");
pub const MenuItem = menu_mod.MenuItem;
pub const MenuOptions = menu_mod.MenuOptions;
/// antd Dropdown menu panel (see components/menu.zig).
pub const dropdownMenu = menu_mod.dropdownMenu;
pub const SegmentedOption = segmented_mod.SegmentedOption;
pub const SegmentedOptions = segmented_mod.SegmentedOptions;
/// antd Segmented control.
pub const segmented = segmented_mod.segmented;
const modal_mod = @import("components/modal.zig");
const shortcut_list_mod = @import("components/shortcut_list.zig");
pub const ModalOptions = modal_mod.ModalOptions;
/// antd Modal: mask + centred card with title, close button and footer.
pub const modal = modal_mod.modal;
pub const Shortcut = shortcut_list_mod.Shortcut;
pub const ShortcutGroup = shortcut_list_mod.ShortcutGroup;
pub const ShortcutListOptions = shortcut_list_mod.ShortcutListOptions;
/// Grouped keyboard shortcut reference in balanced columns.
pub const shortcutList = shortcut_list_mod.shortcutList;
const message_mod = @import("components/message.zig");
pub const MessageKind = message_mod.MessageKind;
pub const MessageOptions = message_mod.MessageOptions;
/// antd message: a transient pill near the top of the window.
pub const message = message_mod.message;
