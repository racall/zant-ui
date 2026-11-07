const std = @import("std");
const geometry = @import("geometry.zig");
const text = @import("../renderer/text.zig");
const rich_text = @import("../renderer/rich_text.zig");
const flex = @import("../layout/flex.zig");
const actions = @import("actions.zig");
pub const vector = @import("vector.zig");
pub const motion = @import("../motion/motion.zig");
const Animated = motion.Animated;

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
    vector,
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
    /// Vertical overflow scrolling (see `ui.BoxOptions.scroll`).
    scroll: bool = false,
    /// Absolute children only: take x / width (and y / height unless set)
    /// from the parent's n-th in-flow child (segmented thumb, tab ink bar).
    anchor: ?u16 = null,
};

/// Declarative motion of a node (see `docs/motion.md`).
pub const MotionStyle = struct {
    /// Enter / exit transition.
    presence: motion.Presence = .none,
    /// Transform origin for scale presets.
    origin: motion.Origin = .center,
    /// Animate position / size changes (FLIP); leaf containers only.
    layout_transition: ?motion.Transition = null,
};

/// Per-node animation state (allocated lazily, see `Tree.ensureAnim`).
pub const NodeAnim = struct {
    presence: Animated = .at(1),
    fx: motion.Fx = .{},
    flip: Animated = .at(0),
    flip_active: bool = false,
    /// Bounds were laid out at least once (no FLIP on first layout).
    flip_ready: bool = false,
    /// Parent-relative bounds before the layout pass (x, y, w, h).
    flip_prev: [4]f32 = @splat(0),
    flip_from: [4]f32 = @splat(0),
    /// Current offset of the painted box from the laid-out bounds.
    flip_delta: [4]f32 = @splat(0),
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
    /// Swallows clicks on the node's empty areas so they do not reach what
    /// is painted behind it (modals, popover panels).
    block_pointer: bool = false,
    /// Text inside can be selected with the pointer and copied
    /// (`BoxOptions.selectable`).
    selectable: bool = false,
    /// Text inside is left out of selections (CSS `user-select: none`):
    /// gutters, labels, toolbars inside selectable content.
    unselectable: bool = false,
    /// Tab / Shift+Tab stay inside the topmost focus scope (modals,
    /// popover panels).
    focus_scope: bool = false,
};

/// Pointer handling of a container (`ui.pointerArea`).
pub const PointerArea = struct {
    action: actions.PointerAction,
    /// Wheel / trackpad scroll goes to the handler instead of scroll containers.
    capture_scroll: bool = false,
    /// false: presses are not taken (no capture, no `down` / `drag`), so
    /// they reach what is inside — text selection in a scrolling list.
    clicks: bool = true,
};

pub const ContainerData = struct {
    /// Pointer area handler (`ui.pointerArea`); makes the container interactive.
    pointer: ?PointerArea = null,
    /// Key handler while the container is focused (focusable pointer areas).
    key_action: ?actions.KeyAction = null,
    /// Called after layout when the container's size changed (`on_resize`).
    resize_action: ?actions.Action = null,
    reported_width: f32 = -1,
    reported_height: f32 = -1,
    /// Cursor shown over the container (pointer areas).
    cursor: ?@import("../platform/platform.zig").Cursor = null,
    /// Current vertical scroll offset for `scroll` containers.
    scroll_offset: f32 = 0,
    /// Natural height of the children, measured during layout.
    content_height: f32 = 0,
    /// Animated `scrollIntoView` (cancelled by the user scrolling).
    scroll_anim: Animated = .{},
    scroll_animating: bool = false,

    pub fn maxScroll(self: ContainerData, viewport_height: f32) f32 {
        return @max(0, self.content_height - viewport_height);
    }
};

pub const TextData = struct {
    content: []u8,
    style: text.Style,
    layout: ?text.TextLayout = null,
    layout_width: f32 = -1,
    layout_dirty: bool = true,
    /// Intrinsic width measured at `measure_avail` (layout's min/max-content
    /// pass), kept apart from `layout` so measuring never discards the
    /// laid-out paragraph (that re-shaped every text node every frame).
    measure_avail: f32 = -1,
    measure_width: f32 = 0,
    /// The layout at the previous width. Flex layout measures a node at a
    /// trial width before its final one; keeping both stops them from
    /// evicting each other (that re-shaped every paragraph every frame).
    alt_layout: ?text.TextLayout = null,
    alt_width: f32 = -1,

    /// Width to lay the text out at inside a box `box_width` wide: wrapping
    /// text uses the box; single-line text with `max_lines` too, so it ends
    /// in "…" when squeezed (1px slack avoids rounding into an ellipsis);
    /// other single-line text keeps its natural width.
    pub fn layoutWidth(self: *const TextData, box_width: f32) f32 {
        if (self.style.wrap) return box_width;
        if (self.style.max_lines > 0) return box_width + 1;
        return 1.0e6;
    }

    pub fn invalidate(self: *TextData) void {
        self.layout_dirty = true;
        self.measure_avail = -1;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
        self.layout_width = -1;
        if (self.alt_layout) |*layout| layout.deinit();
        self.alt_layout = null;
        self.alt_width = -1;
    }

    pub fn ensureLayout(self: *TextData, max_width: f32) !*text.TextLayout {
        if (!self.layout_dirty and self.layout != null and @abs(self.layout_width - max_width) < 0.25) {
            return &self.layout.?;
        }
        if (!self.layout_dirty and self.alt_layout != null) {
            // the other cached width, or re-wrap it to this one (no shaping)
            if (@abs(self.alt_width - max_width) >= 0.25) {
                self.alt_layout.?.relayout(max_width);
                self.alt_width = max_width;
            }
            std.mem.swap(?text.TextLayout, &self.layout, &self.alt_layout);
            std.mem.swap(f32, &self.layout_width, &self.alt_width);
            return &self.layout.?;
        }
        const fresh = try text.TextLayout.init(self.content, self.style, max_width);
        if (self.alt_layout) |*old| old.deinit();
        // the current layout stays as the alternate (unless it is stale)
        if (self.layout_dirty) {
            if (self.layout) |*old| old.deinit();
            self.alt_layout = null;
            self.alt_width = -1;
        } else {
            self.alt_layout = self.layout;
            self.alt_width = self.layout_width;
        }
        self.layout = fresh;
        self.layout_width = max_width;
        self.layout_dirty = false;
        return &self.layout.?;
    }

    /// Longest line when laid out `max_width` wide, cached per width.
    pub fn measureWidth(self: *TextData, max_width: f32) f32 {
        if (@abs(self.measure_avail - max_width) < 0.25) return self.measure_width;
        const w = blk: {
            if (self.layout != null and !self.layout_dirty and @abs(self.layout_width - max_width) < 0.25)
                break :blk self.layout.?.metrics.longest_line;
            var tmp = text.TextLayout.init(self.content, self.style, max_width) catch return 0;
            defer tmp.deinit();
            break :blk tmp.metrics.longest_line;
        };
        self.measure_avail = max_width;
        self.measure_width = w;
        return w;
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
    underline: bool = false,
    strike: bool = false,
    /// Inline drawing (e.g. inline math) laid out as a placeholder box whose
    /// baseline sits on the text baseline. `text` is ignored.
    embed: ?*const vector.Drawing = null,
    /// Optional link target. Clicking the span calls the rich text `on_link`.
    link: []const u8 = "",
    /// Text copied for this span when selected text is copied (null: the
    /// span's `text`; "" leaves it out). Embeds use it for their source,
    /// e.g. "$x^2$" for inline math.
    copy: ?[]const u8 = null,
};

pub const RichTextData = struct {
    spans: []RichSpan,
    /// Owns deep copies of span embeds.
    embed_arena: ?*std.heap.ArenaAllocator = null,
    line_height: f32 = 2.0,
    link_action: ?actions.TextAction = null,
    layout: ?rich_text.Layout = null,
    layout_width: f32 = -1,
    layout_dirty: bool = true,
    /// Unwrapped width (layout's max-content pass), cached apart from
    /// `layout` (see `TextData.measure_width`).
    natural_width: ?f32 = null,
    /// Layout at the previous width (see `TextData.alt_layout`).
    alt_layout: ?rich_text.Layout = null,
    alt_width: f32 = -1,

    pub fn invalidateLayout(self: *RichTextData) void {
        self.layout_dirty = true;
        self.natural_width = null;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
        self.layout_width = -1;
        if (self.alt_layout) |*layout| layout.deinit();
        self.alt_layout = null;
        self.alt_width = -1;
    }

    pub fn ensureLayout(self: *RichTextData, max_width: f32) !*rich_text.Layout {
        if (!self.layout_dirty and self.layout != null and @abs(self.layout_width - max_width) < 0.25) {
            return &self.layout.?;
        }
        if (!self.layout_dirty and self.alt_layout != null) {
            // the other cached width, or re-wrap it to this one (no shaping)
            if (@abs(self.alt_width - max_width) >= 0.25) {
                self.alt_layout.?.relayout(max_width);
                self.alt_width = max_width;
            }
            std.mem.swap(?rich_text.Layout, &self.layout, &self.alt_layout);
            std.mem.swap(f32, &self.layout_width, &self.alt_width);
            return &self.layout.?;
        }
        const fresh = try self.createLayout(max_width);
        if (self.alt_layout) |*old| old.deinit();
        if (self.layout_dirty) {
            if (self.layout) |*old| old.deinit();
            self.alt_layout = null;
            self.alt_width = -1;
        } else {
            self.alt_layout = self.layout;
            self.alt_width = self.layout_width;
        }
        self.layout = fresh;
        self.layout_width = max_width;
        self.layout_dirty = false;
        return &self.layout.?;
    }

    /// Width of the paragraph without wrapping (cached).
    pub fn naturalWidth(self: *RichTextData) f32 {
        if (self.natural_width) |w| return w;
        const w = blk: {
            if (self.layout != null and !self.layout_dirty and self.layout_width >= 1.0e5) break :blk self.layout.?.width;
            var tmp = self.createLayout(1.0e6) catch return 0;
            defer tmp.deinit();
            break :blk tmp.width;
        };
        self.natural_width = w;
        return w;
    }

    fn createLayout(self: *RichTextData, max_width: f32) !rich_text.Layout {
        const native = std.heap.c_allocator.alloc(rich_text.Span, self.spans.len) catch return error.OutOfMemory;
        defer std.heap.c_allocator.free(native);
        for (self.spans, 0..) |span, i| native[i] = .{
            .text = span.text.ptr,
            .length = span.text.len,
            .font_size = span.font_size,
            .weight = @backingInt(span.weight),
            .color = span.color,
            .italic = if (span.italic) 1 else 0,
            .family = @backingInt(span.family),
            .background = span.background,
            .decoration = (if (span.underline) @as(c_int, 1) else 0) | (if (span.strike) @as(c_int, 2) else 0),
            .placeholder_width = if (span.embed) |e| @max(e.width, 0.01) else 0,
            .placeholder_ascent = if (span.embed) |e| e.ascent else 0,
            .placeholder_descent = if (span.embed) |e| e.descent else 0,
        };
        return rich_text.Layout.create(native, max_width, self.line_height);
    }

    pub fn hasEmbeds(self: *const RichTextData) bool {
        for (self.spans) |span| if (span.embed != null) return true;
        return false;
    }

    /// Paints the paragraph and its inline embeds at (x, y).
    pub fn paintAt(self: *RichTextData, canvas: ?*anyopaque, layout: *rich_text.Layout, x: f32, y: f32) void {
        layout.paint(canvas, x, y);
        if (!self.hasEmbeds()) return;
        var rects_buf: [128]rich_text.Rect = undefined;
        const rects = layout.placeholderRects(&rects_buf);
        var index: usize = 0;
        for (self.spans) |span| {
            const embed = span.embed orelse continue;
            if (index >= rects.len) break;
            const r = rects[index];
            index += 1;
            // the placeholder box's top is `ascent` above its baseline
            vector.paint(canvas, embed, x + r.x, y + r.y + embed.ascent);
        }
    }

    pub fn deinitEmbeds(self: *RichTextData) void {
        if (self.embed_arena) |arena| {
            const backing = arena.child_allocator;
            arena.deinit();
            backing.destroy(arena);
            self.embed_arena = null;
        }
    }

    pub fn hasLinks(self: *const RichTextData) bool {
        for (self.spans) |span| if (span.link.len > 0) return true;
        return false;
    }

    /// Returns the link of the span under a byte offset of the joined text.
    pub fn linkAtByte(self: *const RichTextData, byte: usize) ?[]const u8 {
        var start: usize = 0;
        for (self.spans) |span| {
            const end = start + span.text.len;
            if (byte >= start and byte < end) return if (span.link.len > 0) span.link else null;
            start = end;
        }
        return null;
    }
};

pub const ButtonType = enum {
    primary,
    default,
    text,
    link,
    /// Paints nothing: an invisible click target (`ui.dismissLayer`).
    plain,
    /// Windows caption button (minimize / maximize): square, no radius,
    /// subtle hover fill (`ui.windowControls`).
    caption,
    /// Windows close caption button: red hover fill, white glyph.
    caption_close,
};

/// Button icons come from the Zant icon set (`core/icons.zig`).
pub const ButtonIcon = @import("icons.zig").Icon;

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
    // transitions (targets follow the booleans above) and their sampled
    // values for paint
    hover: Animated = .{},
    press: Animated = .{},
    /// Click wave (antd `Wave`), 0..1 over 600ms; 1 = idle.
    wave: Animated = .at(1),
    hover_t: f32 = 0,
    press_t: f32 = 0,
    wave_t: f32 = 1,
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
    icon_size: f32 = 18,

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
    hover: Animated = .{},
    focus_anim: Animated = .{},
    hover_t: f32 = 0,
    focus_t: f32 = 0,
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
    selection_action: ?actions.RangeAction = null,
    key_action: ?actions.InputKeyAction = null,
    applied_selection: ?[2]usize = null,
    reported_selection: [2]usize = .{ 0, 0 },
    layout: ?text.TextLayout = null,
    layout_width: f32 = -1,
    layout_dirty: bool = true,
    display_text: std.ArrayListUnmanaged(u8) = .empty,
    placeholder_layout: ?text.TextLayout = null,
    font_size: f32 = 14,
    text_color: u32 = 0xe0000000,
    layout_color: u32 = 0,
    /// Centred text column width for wide multiline inputs (0 = full width).
    content_max_width: f32 = 0,
    /// Column kept while moving the caret up/down through lines.
    preferred_x: ?f32 = null,
    /// Scroll the caret into view on the next paint (set by edits, caret
    /// moves and controlled selection; scrolling itself never clears it).
    reveal_caret: bool = false,

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
    hover: Animated = .{},
    press: Animated = .{},
    /// Fill / border / track / knob position.
    check: Animated = .{},
    /// Checkmark scale (checkbox) / dot scale (radio).
    mark: Animated = .{},
    hover_t: f32 = 0,
    press_t: f32 = 0,
    check_t: f32 = 0,
    mark_t: f32 = 0,
    font_size: f32 = 14,
    action: ?actions.BoolAction = null,
    index_action: ?actions.IndexAction = null,
    index: usize = 0,
    layout: ?text.TextLayout = null,
    layout_dirty: bool = true,
    layout_color: u32 = 0,

    pub fn invalidateLayout(self: *ToggleData) void {
        self.layout_dirty = true;
        if (self.layout) |*layout| layout.deinit();
        self.layout = null;
    }

    pub fn ensureLayout(self: *ToggleData, color: u32) !*text.TextLayout {
        if (!self.layout_dirty and self.layout != null and self.layout_color == color) return &self.layout.?;
        self.layout_color = color;
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
    /// `zant.image.Image.id` of an in-memory source (0 = file `path`).
    source_id: u64 = 0,
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
    hover: Animated = .{},
    focus_anim: Animated = .{},
    open_anim: Animated = .{},
    hover_t: f32 = 0,
    focus_t: f32 = 0,
    open_t: f32 = 0,
    hovered_option: ?usize = null,
    font_size: f32 = 14,
    action: ?actions.IndexAction = null,
    layout: ?text.TextLayout = null,
    option_layouts: std.ArrayListUnmanaged(text.TextLayout) = .empty,
    layout_dirty: bool = true,
    layout_color: u32 = 0,
    option_color: u32 = 0,

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

pub const VectorData = struct {
    arena: *std.heap.ArenaAllocator,
    drawing: *const vector.Drawing,
    hash: u64,
    /// Text copied for the drawing in a text selection ("" = not selectable),
    /// e.g. a formula's LaTeX. Owned by `arena`.
    copy: []const u8 = "",

    pub fn deinit(self: *VectorData) void {
        const backing = self.arena.child_allocator;
        self.arena.deinit();
        backing.destroy(self.arena);
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
    vector: VectorData,
};

pub const Node = struct {
    id: NodeId,
    parent: ?NodeId = null,
    children: std.ArrayListUnmanaged(NodeId) = .empty,
    style: FlexStyle = .{},
    visual: Visual = .{},
    key: ?[]u8 = null,
    tooltip: ?[]u8 = null,
    /// Hash of the last memoized spec (see `ui.memo`); null when not memoized.
    memo_hash: ?u64 = null,
    bounds: geometry.Rect = .{},
    dirty: DirtyFlags = .{},
    /// False for a freed slot (tombstone) waiting in `Tree.free_ids`.
    alive: bool = true,
    /// Listed in `Tree.animating`.
    anim_queued: bool = false,
    motion: MotionStyle = .{},
    anim: ?*NodeAnim = null,
    /// Playing its exit transition: painted, but out of layout, hit
    /// testing and focus; freed when the transition ends.
    exiting: bool = false,
    /// Selected byte range of a text / rich text node (text selection).
    text_sel: ?[2]u32 = null,
    visible: bool = true,
    enabled: bool = true,
    focusable: bool = false,
    data: Data,

    pub fn kind(self: *const Node) NodeKind {
        return std.meta.activeTag(self.data);
    }

    /// Takes part in layout and input (visible and not exiting).
    pub fn inFlow(self: *const Node) bool {
        return self.visible and !self.exiting;
    }

    pub fn deinit(self: *Node, allocator: std.mem.Allocator) void {
        if (self.anim) |anim| allocator.destroy(anim);
        self.anim = null;
        if (self.key) |key| allocator.free(key);
        self.key = null;
        if (self.tooltip) |tooltip| allocator.free(tooltip);
        self.tooltip = null;
        switch (self.data) {
            .text => |*value| {
                value.invalidate();
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
            .vector => |*value| value.deinit(),
            .rich_text => |*value| {
                value.invalidateLayout();
                for (value.spans) |span| {
                    allocator.free(span.text);
                    if (span.link.len > 0) allocator.free(span.link);
                    if (span.copy) |c| allocator.free(c);
                }
                value.deinitEmbeds();
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
            .rich_text => |*data| data.link_action != null and data.hasLinks(),
            .container => |*data| data.pointer != null,
            else => false,
        };
    }

    pub fn mark(self: *Node, layout: bool, paint: bool) void {
        if (layout) self.dirty.layout = true;
        if (paint or layout) self.dirty.paint = true;
    }
};
