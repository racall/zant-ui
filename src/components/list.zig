//! Virtual list (`ui.list`): only the items in and near the viewport are
//! built and laid out, so a list of thousands of rows — or a chat with long
//! messages — opens and scrolls at the cost of one screen.
//!
//! Modelled on GPUI's `list` / `uniform_list` (crates/gpui/src/elements):
//! - item heights are measured when items are laid out and remembered;
//!   unmeasured items count with an estimate (the running average);
//! - the scroll position is logical — item index + offset into it — so a
//!   corrected estimate above the viewport never moves what is on screen;
//! - `overdraw` px above and below the viewport are built too, so
//!   scrolling never shows a gap;
//! - `.bottom` alignment + `follow_tail` for chats: new content stays in
//!   view until the user scrolls up, and following resumes at the end.
//!
//! The app owns a `ListState` (like `markdown.Cache`) and describes items
//! with a builder called only for visible indices:
//!
//!     var chat_list: ui.ListState = .init(gpa);
//!     ...
//!     ui.list(&self.chat_list, .{
//!         .key = "chat",
//!         .count = self.messages.len,
//!         .item = ui.listItem(self, State.message),   // fn (*State, usize) ui.Node
//!         .alignment = .bottom,
//!         .follow_tail = true,
//!     })
const std = @import("std");
const ui = @import("../ui.zig");
const geometry = @import("../core/geometry.zig");
const actions = @import("../core/actions.zig");
const events = @import("../core/events.zig");
const tokens = @import("../theme/tokens.zig");

pub const Alignment = enum {
    /// Items start at the top (most lists).
    top,
    /// A short list sits at the bottom of the viewport (chats).
    bottom,
};

pub const ItemBuilder = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, index: usize) ui.Node,
};

/// Wraps `fn (ctx, index) ui.Node` as a list item builder.
pub fn itemBuilder(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), usize) ui.Node) ItemBuilder {
    const Adapter = struct {
        fn call(raw: *anyopaque, index: usize) ui.Node {
            return func(@ptrCast(@alignCast(raw)), index);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub const ItemId = struct {
    ctx: *anyopaque,
    func: *const fn (ctx: *anyopaque, index: usize) u64,
};

/// Wraps `fn (ctx, index) u64` (stable item ids, see `ListOptions.item_id`).
pub fn itemId(ctx: anytype, comptime func: *const fn (@TypeOf(ctx), usize) u64) ItemId {
    const Adapter = struct {
        fn call(raw: *anyopaque, index: usize) u64 {
            return func(@ptrCast(@alignCast(raw)), index);
        }
    };
    return .{ .ctx = @ptrCast(ctx), .func = Adapter.call };
}

pub const ListOptions = struct {
    /// Key of the list's viewport (node keys of items derive from it).
    key: []const u8 = "list",
    count: usize,
    item: ItemBuilder,
    /// Stable ids for item keys. Without it items are keyed by index, which
    /// is right when items are only appended (chats, logs); give ids when
    /// items are inserted or removed in the middle.
    item_id: ?ItemId = null,
    alignment: Alignment = .top,
    /// Keep the end in view as items grow or are added, until the user
    /// scrolls away from it; scrolling back to the end resumes following.
    follow_tail: bool = false,
    /// Extra px built above and below the viewport.
    overdraw: f32 = 600,
    /// Every item has this height (no measuring; `uniform_list`).
    item_height: ?f32 = null,
    /// Height assumed for items not laid out yet (until some are measured,
    /// after which their average is used).
    estimated_height: f32 = 120,
    gap: f32 = 0,
    padding: geometry.Edges = .{},
    background: ?u32 = null,
    /// Viewport size: fixed, or grow inside a sized parent (default).
    width: ?f32 = null,
    height: ?f32 = null,
    grow: f32 = 1,
    /// Text in the items can be selected and copied (`BoxOptions.selectable`).
    selectable: bool = false,
    /// Take keyboard focus on click: arrows, Page Up / Down, Home / End.
    focusable: bool = true,
    scrollbar: bool = true,
    /// Called after the user scrolls (wheel, keys, scroll bar).
    on_scroll: ?actions.Action = null,
};

/// Scroll position, heights and follow state of one list (app-owned).
pub const ListState = struct {
    allocator: std.mem.Allocator,
    /// Measured height per item (< 0: not measured yet).
    heights: std.ArrayListUnmanaged(f32) = .empty,
    /// Fenwick trees over measured heights and measured counts (prefix sums).
    fen_sum: std.ArrayListUnmanaged(f64) = .empty,
    fen_cnt: std.ArrayListUnmanaged(u32) = .empty,
    measured_total: f64 = 0,
    measured_count: usize = 0,
    /// Viewport top = top of item `anchor` + `offset` (offset may be negative
    /// within the top padding).
    anchor: usize = 0,
    offset: f32 = 0,
    following: bool = true,
    viewport: geometry.Size = .{},
    rendered_first: usize = 0,
    rendered_end: usize = 0,
    uniform: ?f32 = null,
    estimate_hint: f32 = 120,
    gap: f32 = 0,
    padding: geometry.Edges = .{},
    pending_scroll: ?struct { index: usize, position: ScrollPosition } = null,
    bar_drag: ?struct { start: f32, top: f32 } = null,
    options_key: []const u8 = "",
    item_id: ?ItemId = null,
    on_scroll: ?actions.Action = null,

    pub const ScrollPosition = enum { start, center, end };

    pub fn init(allocator: std.mem.Allocator) ListState {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *ListState) void {
        self.heights.deinit(self.allocator);
        self.fen_sum.deinit(self.allocator);
        self.fen_cnt.deinit(self.allocator);
    }

    pub fn count(self: *const ListState) usize {
        return self.heights.items.len;
    }

    // ---- heights ------------------------------------------------------

    fn rebuildFenwick(self: *ListState) void {
        const n = self.heights.items.len;
        self.fen_sum.resize(self.allocator, n) catch return;
        self.fen_cnt.resize(self.allocator, n) catch return;
        @memset(self.fen_sum.items, 0);
        @memset(self.fen_cnt.items, 0);
        self.measured_total = 0;
        self.measured_count = 0;
        for (self.heights.items, 0..) |h, i| if (h >= 0) self.fenAdd(i, h, 1);
    }

    fn fenAdd(self: *ListState, index: usize, dh: f64, dc: i32) void {
        self.measured_total += dh;
        self.measured_count = @intCast(@as(i64, @intCast(self.measured_count)) + dc);
        var i = index + 1;
        while (i <= self.fen_sum.items.len) : (i += i & (~i + 1)) {
            self.fen_sum.items[i - 1] += dh;
            self.fen_cnt.items[i - 1] = @intCast(@as(i64, self.fen_cnt.items[i - 1]) + dc);
        }
    }

    /// (sum of measured heights, number measured) of items [0, n).
    fn fenPrefix(self: *const ListState, n: usize) struct { sum: f64, cnt: usize } {
        var sum: f64 = 0;
        var cnt: usize = 0;
        var i = @min(n, self.fen_sum.items.len);
        while (i > 0) : (i -= i & (~i + 1)) {
            sum += self.fen_sum.items[i - 1];
            cnt += self.fen_cnt.items[i - 1];
        }
        return .{ .sum = sum, .cnt = cnt };
    }

    /// Height assumed for unmeasured items.
    pub fn estimate(self: *const ListState) f32 {
        if (self.uniform) |h| return h;
        if (self.measured_count == 0) return self.estimate_hint;
        return @floatCast(self.measured_total / @as(f64, @floatFromInt(self.measured_count)));
    }

    pub fn itemHeight(self: *const ListState, index: usize) f32 {
        if (self.uniform) |h| return h;
        const h = self.heights.items[index];
        return if (h >= 0) h else self.estimate();
    }

    fn setMeasured(self: *ListState, index: usize, h: f32) void {
        if (index >= self.heights.items.len) return;
        const old = self.heights.items[index];
        if (old >= 0 and @abs(old - h) < 0.25) return;
        if (old >= 0) self.fenAdd(index, h - old, 0) else self.fenAdd(index, h, 1);
        self.heights.items[index] = h;
    }

    /// Top of item `index` in content coordinates (padding included).
    pub fn itemTop(self: *const ListState, index: usize) f32 {
        const n = @min(index, self.count());
        if (self.uniform) |h| return self.padding.top + @as(f32, @floatFromInt(n)) * (h + self.gap);
        const p = self.fenPrefix(n);
        const est = self.estimate();
        return self.padding.top + @as(f32, @floatCast(p.sum)) + @as(f32, @floatFromInt(n - p.cnt)) * est + @as(f32, @floatFromInt(n)) * self.gap;
    }

    pub fn contentHeight(self: *const ListState) f32 {
        const n = self.count();
        if (n == 0) return self.padding.vertical();
        return self.itemTop(n) - self.gap + self.padding.bottom;
    }

    /// Item at content y (binary search on the prefix sums).
    fn indexAt(self: *const ListState, y: f32) usize {
        const n = self.count();
        if (n == 0) return 0;
        var lo: usize = 0;
        var hi: usize = n;
        while (lo + 1 < hi) {
            const mid = (lo + hi) / 2;
            if (self.itemTop(mid) <= y) lo = mid else hi = mid;
        }
        return lo;
    }

    // ---- mutations ------------------------------------------------------

    /// Matches the item count (appends unmeasured items, drops trailing ones).
    pub fn setCount(self: *ListState, n: usize) void {
        const old = self.count();
        if (n == old) return;
        if (n > old) {
            self.heights.appendNTimes(self.allocator, -1, n - old) catch return;
        } else {
            self.heights.shrinkRetainingCapacity(n);
            if (self.anchor >= n) {
                self.anchor = if (n > 0) n - 1 else 0;
                self.offset = 0;
            }
        }
        self.rebuildFenwick();
    }

    /// `removed` items at `at` were replaced by `inserted` new ones.
    pub fn splice(self: *ListState, at: usize, removed: usize, inserted: usize) void {
        const n = self.count();
        const start = @min(at, n);
        const end = @min(n, start + removed);
        const fresh = self.allocator.alloc(f32, inserted) catch return;
        defer self.allocator.free(fresh);
        @memset(fresh, -1);
        self.heights.replaceRange(self.allocator, start, end - start, fresh) catch return;
        if (self.anchor >= end) {
            self.anchor = self.anchor - (end - start) + inserted;
        } else if (self.anchor >= start) {
            self.anchor = start;
            self.offset = 0;
        }
        if (self.anchor >= self.count() and self.count() > 0) self.anchor = self.count() - 1;
        self.rebuildFenwick();
    }

    /// Item `index` must be measured again (its content changed).
    pub fn invalidate(self: *ListState, index: usize) void {
        if (index >= self.count() or self.heights.items[index] < 0) return;
        self.fenAdd(index, -self.heights.items[index], -1);
        self.heights.items[index] = -1;
    }

    /// Every item must be measured again (e.g. the width or font changed).
    pub fn invalidateAll(self: *ListState) void {
        @memset(self.heights.items, -1);
        self.rebuildFenwick();
    }

    // ---- scrolling ------------------------------------------------------

    pub fn scrollTop(self: *const ListState) f32 {
        if (self.count() == 0) return 0;
        return self.itemTop(self.anchor) + self.offset;
    }

    fn maxScrollTop(self: *const ListState) f32 {
        return @max(0, self.contentHeight() - self.viewport.height);
    }

    fn setScrollTop(self: *ListState, top: f32) void {
        const t = std.math.clamp(top, 0, self.maxScrollTop());
        if (self.count() == 0) {
            self.anchor = 0;
            self.offset = 0;
            return;
        }
        self.anchor = self.indexAt(t);
        self.offset = t - self.itemTop(self.anchor);
    }

    pub fn isAtEnd(self: *const ListState) bool {
        return self.scrollTop() >= self.maxScrollTop() - 1;
    }

    /// Scrolls by `dy` px (positive = towards the end).
    pub fn scrollBy(self: *ListState, dy: f32) void {
        self.setScrollTop(self.scrollTop() + dy);
        self.following = self.isAtEnd();
    }

    /// Brings item `index` into view at the next build.
    pub fn scrollToItem(self: *ListState, index: usize, position: ScrollPosition) void {
        self.pending_scroll = .{ .index = index, .position = position };
        self.following = false;
    }

    /// Scrolls to the end and resumes following the tail.
    pub fn scrollToEnd(self: *ListState) void {
        self.following = true;
        self.pending_scroll = null;
    }

    /// Indices built in the last frame `[first, end)` (visible + overdraw).
    pub fn renderedRange(self: *const ListState) [2]usize {
        return .{ self.rendered_first, self.rendered_end };
    }

    fn scrolled(self: *ListState) void {
        if (self.on_scroll) |a| a.func(a.ctx);
    }

    fn itemKey(self: *const ListState, index: usize) []const u8 {
        if (self.item_id) |f| return std.fmt.allocPrint(ui.frameAllocator(), "{s}-k{d}", .{ self.options_key, f.func(f.ctx, index) }) catch "";
        return std.fmt.allocPrint(ui.frameAllocator(), "{s}-i{d}", .{ self.options_key, index }) catch "";
    }

    // ---- events ---------------------------------------------------------

    fn onPointer(self: *ListState, info: ui.PointerInfo) bool {
        switch (info.phase) {
            .scroll => {
                if (@abs(info.dy) < 0.01) return false;
                self.scrollBy(-info.dy);
                self.scrolled();
                return true;
            },
            else => return false,
        }
    }

    fn onKey(self: *ListState, k: events.KeyEvent) bool {
        const page = @max(40, self.viewport.height * 0.9);
        switch (k.key) {
            .up => self.scrollBy(-40),
            .down => self.scrollBy(40),
            .page_up => self.scrollBy(-page),
            .page_down => self.scrollBy(page),
            .home => self.setScrollTop(0),
            .end => self.scrollToEnd(),
            .character => |c| {
                if (c != ' ') return false;
                self.scrollBy(if (k.modifiers.shift) -page else page);
            },
            else => return false,
        }
        if (k.key == .home) self.following = false;
        self.scrolled();
        return true;
    }

    fn onBar(self: *ListState, info: ui.PointerInfo) bool {
        const content = self.contentHeight();
        if (content <= self.viewport.height) return false;
        switch (info.phase) {
            .down => self.bar_drag = .{ .start = info.window_y, .top = self.scrollTop() },
            .drag => {
                const d = self.bar_drag orelse return false;
                self.setScrollTop(d.top + (info.window_y - d.start) * content / @max(1, self.viewport.height));
                self.following = self.isAtEnd();
                self.scrolled();
            },
            .up => self.bar_drag = null,
            else => return false,
        }
        return true;
    }

    /// An item changed size (its `on_resize`): the rebuild that follows
    /// in the same frame reads the new heights.
    fn onItemResize(self: *ListState) void {
        _ = self;
    }

    fn onViewportResize(self: *ListState) void {
        _ = self;
    }
};

/// The virtual list (see the module documentation).
pub fn list(state: *ListState, options: ListOptions) ui.Node {
    const arena = ui.frameAllocator();
    const theme = ui.currentTheme();
    state.options_key = options.key;
    state.item_id = options.item_id;
    state.on_scroll = options.on_scroll;
    state.uniform = options.item_height;
    state.estimate_hint = options.estimated_height;
    state.gap = options.gap;
    state.padding = options.padding;
    state.setCount(options.count);
    if (!options.follow_tail) state.following = false;

    // the viewport as laid out last time (on_resize rebuilds when it changes)
    if (ui.keyBounds(options.key)) |b| {
        if (@abs(b.width - state.viewport.width) > 0.5 and state.viewport.width > 0 and state.uniform == null) {
            // heights depend on the width: measure again as items show
            state.invalidateAll();
        }
        state.viewport = .{ .width = b.width, .height = b.height };
    }
    // measure the items built last time
    if (state.uniform == null) {
        var i = state.rendered_first;
        while (i < state.rendered_end and i < state.count()) : (i += 1) {
            if (ui.keyBounds(state.itemKey(i))) |b| state.setMeasured(i, b.height);
        }
    }

    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    const n = state.count();
    const vp_h = state.viewport.height;
    if (vp_h > 0 and n > 0) {
        if (state.pending_scroll) |p| {
            state.pending_scroll = null;
            const idx = @min(p.index, n - 1);
            const top = state.itemTop(idx);
            const h = state.itemHeight(idx);
            state.setScrollTop(switch (p.position) {
                .start => top - state.padding.top,
                .center => top + h / 2 - vp_h / 2,
                .end => top + h - vp_h + state.padding.bottom,
            });
        }
        if (state.following) state.setScrollTop(state.maxScrollTop());
        // keep the anchor inside the scroll range (heights may have changed)
        state.setScrollTop(state.scrollTop());

        const content_h = state.contentHeight();
        // a short list sits at the bottom for `.bottom` alignment
        const shift: f32 = if (content_h < vp_h and options.alignment == .bottom) vp_h - content_h else 0;
        const top = state.scrollTop();
        const want_first = state.indexAt(@max(0, top - options.overdraw));
        var want_end = state.indexAt(top + vp_h + options.overdraw) + 1;
        if (want_end > n) want_end = n;
        // Estimates of unmeasured items can be far off (a 120 px guess for a
        // 1500 px message), so build at most a few of them per pass, growing
        // outward from the anchor; the same-frame layout passes measure them
        // and the next pass fills the rest of the range (GPUI measures items
        // one by one the same way).
        const max_unmeasured: usize = 3;
        var unmeasured: usize = 0;
        const anchor = @min(state.anchor, n - 1);
        var first = @max(want_first, @min(anchor, want_end - 1));
        var end = first + 1;
        if (state.uniform == null and state.heights.items[first] < 0) unmeasured += 1;
        while (true) {
            var grew = false;
            if (end < want_end and (state.uniform != null or state.heights.items[end] >= 0 or unmeasured < max_unmeasured)) {
                if (state.uniform == null and state.heights.items[end] < 0) unmeasured += 1;
                end += 1;
                grew = true;
            }
            if (first > want_first and (state.uniform != null or state.heights.items[first - 1] >= 0 or unmeasured < max_unmeasured)) {
                first -= 1;
                if (state.uniform == null and state.heights.items[first] < 0) unmeasured += 1;
                grew = true;
            }
            if (!grew) break;
        }
        // the rest of the range comes in the next frame
        if (first > want_first or end < want_end) ui.wakeAfter(1);
        state.rendered_first = first;
        state.rendered_end = end;

        const items = arena.alloc(ui.Node, end - first) catch return ui.column(.{});
        for (first..end) |i| {
            const built = options.item.func(options.item.ctx, i);
            items[i - first] = ui.column(.{
                .key = state.itemKey(i),
                .width_percent = 100,
                .align_items = .stretch,
                .height = options.item_height,
                .on_resize = if (options.item_height == null) ui.action(state, ListState.onItemResize) else null,
                .children = arena.dupe(ui.Node, &.{built}) catch &.{},
            });
        }
        // the built items as one column, placed so the anchor lines up
        const y = shift + state.itemTop(first) - top;
        children.append(arena, ui.styled(ui.column(.{
            .key = std.fmt.allocPrint(arena, "{s}-content", .{options.key}) catch "",
            .gap = options.gap,
            .padding = .{ .left = options.padding.left, .right = options.padding.right },
            .align_items = .stretch,
            .children = items,
        }), .{ .absolute = true, .left = 0, .right = 0, .top = y })) catch {};

        if (options.scrollbar and content_h > vp_h + 0.5) {
            const track = vp_h - 8;
            const thumb = @max(28, track * vp_h / content_h);
            const max_top = @max(1, content_h - vp_h);
            const thumb_y = 4 + (track - thumb) * std.math.clamp(top / max_top, 0, 1);
            const color = tokens.withAlpha(theme.color_text, if (state.bar_drag != null) 0x70 else 0x40);
            children.append(arena, ui.pointerArea(.{
                .box = .{ .key = std.fmt.allocPrint(arena, "{s}-bar", .{options.key}) catch "", .absolute = true, .right = 0, .top = thumb_y, .width = 12, .height = thumb, .align_items = .center, .children = arena.dupe(ui.Node, &.{
                    ui.styled(ui.column(.{}), .{ .width = 6, .height = thumb, .radius = 3, .background = color }),
                }) catch &.{} },
                .on_pointer = ui.pointerAction(state, ListState.onBar),
            })) catch {};
        }
    } else {
        state.rendered_first = 0;
        state.rendered_end = 0;
    }

    return ui.pointerArea(.{
        .box = .{
            .key = options.key,
            .width = options.width,
            .height = options.height,
            .grow = if (options.height == null) options.grow else 0,
            .shrink = 1,
            .basis = if (options.height == null) 0 else null,
            .width_percent = if (options.width == null) 100 else null,
            .clip = true,
            .background = options.background,
            .selectable = options.selectable,
            .on_resize = ui.action(state, ListState.onViewportResize),
            .children = children.items,
        },
        .on_pointer = ui.pointerAction(state, ListState.onPointer),
        .capture_scroll = true,
        // the list takes the wheel only: presses reach the items (text
        // selection, buttons) and focus the list for its keys
        .clicks = false,
        .focusable = options.focusable,
        .on_key = ui.keyAction(state, ListState.onKey),
    });
}
