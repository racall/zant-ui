//! PDF viewer: continuous vertical scrolling, zoom (fit width / fit page /
//! manual, ⌘/Ctrl + wheel or pinch anchored at the pointer), text selection
//! (drag, double click = word, triple click = line, Shift extends) with
//! copy, search with highlighted matches, links, page navigation and a
//! toolbar.
//!
//! Modelled on EmbedPDF's plugins (viewport, scroll vertical strategy, zoom,
//! render + tiling, selection, search), drawn natively with Zant nodes.
//! The `Viewer` holds the state (the app owns it, like `markdown.Cache`):
//!
//!     var viewer: zant.pdf.Viewer = .init(gpa);
//!     try viewer.open("manual.pdf");
//!     ...
//!     zant.pdf.view(&viewer, .{ .key = "pdf" })   // in render
const std = @import("std");
const ui = @import("../ui.zig");
const tokens = @import("../theme/tokens.zig");
const events = @import("../core/events.zig");
const keys = @import("../core/keys.zig");
const actions = @import("../core/actions.zig");
const platform = @import("../platform/platform.zig");
const dialogs = @import("../platform/dialog.zig");
const document_mod = @import("document.zig");
const text_mod = @import("text.zig");
const renderer_mod = @import("renderer.zig");
const native = @import("native.zig");
const image = @import("../image.zig");
const vector = @import("../core/vector.zig");
pub const annots = @import("annotations.zig");

const Document = document_mod.Document;
const Renderer = renderer_mod.Renderer;
const Rect = text_mod.Rect;

pub const Pos = struct {
    page: usize,
    index: usize,

    fn less(a: Pos, b: Pos) bool {
        return a.page < b.page or (a.page == b.page and a.index < b.index);
    }
};

/// Selected glyphs `start..end` (inclusive, ordered).
pub const Range = struct { start: Pos, end: Pos };

pub const Match = struct { page: usize, from: usize, to: usize };

pub const Fit = enum {
    /// Page width fills the viewport.
    width,
    /// The whole page is visible.
    page,
    /// Manual zoom (`setZoom`, zoom buttons, ⌘/Ctrl + wheel).
    none,
};

pub const Labels = struct {
    search: []const u8 = "Search",
    no_results: []const u8 = "No results",
    previous_page: []const u8 = "Previous page",
    next_page: []const u8 = "Next page",
    zoom_in: []const u8 = "Zoom in",
    zoom_out: []const u8 = "Zoom out",
    fit_width: []const u8 = "Fit width",
    fit_page: []const u8 = "Fit page",
    previous_match: []const u8 = "Previous match",
    next_match: []const u8 = "Next match",
    copy: []const u8 = "Copy",
    cannot_open: []const u8 = "Cannot open this PDF",
    password: []const u8 = "This PDF is password protected",
    highlight: []const u8 = "Highlight",
    underline: []const u8 = "Underline",
    strikeout: []const u8 = "Strikethrough",
    pen: []const u8 = "Pen",
    note: []const u8 = "Note",
    select: []const u8 = "Select",

    pub const zh: Labels = .{
        .search = "搜索",
        .no_results = "无结果",
        .previous_page = "上一页",
        .next_page = "下一页",
        .zoom_in = "放大",
        .zoom_out = "缩小",
        .fit_width = "适合宽度",
        .fit_page = "适合页面",
        .previous_match = "上一个",
        .next_match = "下一个",
        .copy = "复制",
        .cannot_open = "无法打开此 PDF",
        .password = "此 PDF 受密码保护",
        .highlight = "高亮",
        .underline = "下划线",
        .strikeout = "删除线",
        .pen = "画笔",
        .note = "便签",
        .select = "选择",
    };
};

pub const ViewOptions = struct {
    /// Prefix of node keys (several viewers in one window need different keys).
    key: []const u8 = "pdf",
    /// Show the toolbar (page navigation, zoom, search).
    toolbar: bool = true,
    /// Space between pages and around them (px).
    page_gap: f32 = 12,
    padding: f32 = 16,
    /// Viewport background (default: theme colorBgLayout).
    background: ?u32 = null,
    labels: Labels = .{},
    /// External link clicks; null opens the URL with the system browser.
    on_link: ?actions.TextAction = null,
    /// Page, zoom, selection or search results changed (for app UI that
    /// shows them, e.g. a page counter elsewhere).
    on_change: ?actions.Action = null,
    /// Annotation tools in the toolbar (select / highlight / underline /
    /// pen / note).
    annotation_tools: bool = false,
    /// A small menu over selected text: copy, highlight, underline,
    /// strikethrough.
    selection_menu: bool = true,
    /// The annotation list or the selected annotation changed (save the
    /// list; show an editor for `Viewer.selectedAnnotation()` placed with
    /// `ui.keyBounds(Viewer.annotationKey(..))`).
    on_annotation: ?actions.Action = null,
};

/// Zoom steps of the zoom buttons and keyboard shortcuts (EmbedPDF presets).
pub const zoom_steps = [_]f32{ 0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5 };
pub const min_zoom: f32 = 0.25;
pub const max_zoom: f32 = 5;

const DragMode = enum { char, word, line };

/// What a drag on the page does.
pub const Tool = enum {
    /// Select text (and click annotations to select them).
    select,
    /// Drag over text to highlight / underline / strike it out.
    highlight,
    underline,
    strikeout,
    squiggly,
    /// Draw freehand strokes.
    ink,
    /// Click to place a sticky note.
    note,

    fn markupKind(self: Tool) ?annots.Kind {
        return switch (self) {
            .highlight => .highlight,
            .underline => .underline,
            .strikeout => .strikeout,
            .squiggly => .squiggly,
            else => null,
        };
    }
};

pub const Viewer = struct {
    allocator: std.mem.Allocator,
    doc: ?*Document = null,
    renderer: ?*Renderer = null,
    open_error: ?document_mod.OpenError = null,

    zoom: f32 = 1,
    fit: Fit = .width,
    scroll_x: f32 = 0,
    scroll_y: f32 = 0,
    view_w: f32 = 0,
    view_h: f32 = 0,
    /// Page tops in content px at `layout_zoom`.
    tops: std.ArrayListUnmanaged(f32) = .empty,
    layout_zoom: f32 = 0,
    content_w: f32 = 0,
    content_h: f32 = 0,
    max_page_w: f32 = 0,
    current_page: usize = 0,

    /// Selection (anchor = where the drag started).
    anchor: ?Pos = null,
    focus: ?Pos = null,
    drag: ?DragMode = null,
    drag_moved: bool = false,
    down_x: f32 = 0,
    down_y: f32 = 0,
    /// Word / line under the pointer when a multi-click drag started.
    anchor_slice: ?[2]Pos = null,
    pressed_link: ?document_mod.Link = null,
    hover: enum { none, text, link } = .none,

    query: std.ArrayListUnmanaged(u8) = .empty,
    matches: std.ArrayListUnmanaged(Match) = .empty,
    active_match: ?usize = null,
    /// Next page to search (incremental search runs a few pages per build).
    search_page: usize = 0,
    search_done: bool = true,

    page_input: std.ArrayListUnmanaged(u8) = .empty,
    page_input_page: ?usize = null,
    /// Scroll to this page (and y in points) once the layout is known.
    pending_goto: ?struct { page: usize, y: f32 } = null,
    /// Scrollbar drag: pointer y and scroll offset at the start.
    bar_drag: ?struct { start: f32, scroll: f32, horizontal: bool } = null,

    /// Annotations (with undo / redo).
    annotations: annots.Store,
    tool: Tool = .select,
    /// Colour for new annotations of the current tool (null: default).
    tool_color: ?u32 = null,
    selected_annotation: ?u64 = null,
    /// Ink stroke being drawn (page points).
    ink_points: std.ArrayListUnmanaged(annots.Point) = .empty,
    ink_page: usize = 0,
    /// Path of the open document (annotated export reads it again).
    path: std.ArrayListUnmanaged(u8) = .empty,

    options: ViewOptions = .{},
    padding: f32 = 16,
    page_gap: f32 = 12,

    pub fn init(allocator: std.mem.Allocator) Viewer {
        return .{ .allocator = allocator, .annotations = .init(allocator) };
    }

    pub fn deinit(self: *Viewer) void {
        self.close();
        self.annotations.deinit();
        self.ink_points.deinit(self.allocator);
        self.path.deinit(self.allocator);
        self.tops.deinit(self.allocator);
        self.query.deinit(self.allocator);
        self.matches.deinit(self.allocator);
        self.page_input.deinit(self.allocator);
    }

    /// Opens a PDF (closing the current one). On failure the viewer shows
    /// an error state and the error is returned.
    pub fn open(self: *Viewer, path: []const u8) document_mod.OpenError!void {
        return self.openWithPassword(path, null);
    }

    pub fn openWithPassword(self: *Viewer, path: []const u8, password: ?[]const u8) document_mod.OpenError!void {
        self.close();
        const doc = Document.open(self.allocator, path, password) catch |err| {
            self.open_error = err;
            return err;
        };
        self.doc = doc;
        self.path.clearRetainingCapacity();
        self.path.appendSlice(self.allocator, path) catch {};
        self.renderer = Renderer.create(self.allocator, doc) catch null;
        self.open_error = null;
        self.layout_zoom = 0;
    }

    pub fn close(self: *Viewer) void {
        if (self.renderer) |r| r.destroy();
        self.renderer = null;
        if (self.doc) |d| d.close();
        self.doc = null;
        self.anchor = null;
        self.focus = null;
        self.matches.clearRetainingCapacity();
        self.active_match = null;
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.current_page = 0;
        self.tops.clearRetainingCapacity();
        self.open_error = null;
        self.annotations.load(&.{}) catch {};
        self.selected_annotation = null;
        self.ink_points.clearRetainingCapacity();
    }

    pub fn document(self: *Viewer) ?*Document {
        return self.doc;
    }

    pub fn pageCount(self: *const Viewer) usize {
        return if (self.doc) |d| d.pageCount() else 0;
    }

    // ---- zoom ---------------------------------------------------------

    pub fn setFit(self: *Viewer, fit: Fit) void {
        self.fit = fit;
        if (fit != .none) self.applyFit(self.anchorCenter());
    }

    /// Sets a manual zoom (1 = 100%), keeping the viewport centre in place.
    pub fn setZoom(self: *Viewer, zoom: f32) void {
        self.fit = .none;
        self.zoomAt(zoom, self.view_w / 2, self.view_h / 2);
    }

    pub fn zoomIn(self: *Viewer) void {
        for (zoom_steps) |z| if (z > self.zoom + 0.001) return self.setZoom(z);
        self.setZoom(max_zoom);
    }

    pub fn zoomOut(self: *Viewer) void {
        var i: usize = zoom_steps.len;
        while (i > 0) {
            i -= 1;
            if (zoom_steps[i] < self.zoom - 0.001) return self.setZoom(zoom_steps[i]);
        }
        self.setZoom(min_zoom);
    }

    fn fitZoom(self: *const Viewer, fit: Fit) ?f32 {
        const doc = self.doc orelse return null;
        if (self.view_w <= 0 or doc.pageCount() == 0) return null;
        var max_w: f32 = 1;
        var max_h: f32 = 1;
        for (doc.sizes) |s| {
            max_w = @max(max_w, s.width);
            max_h = @max(max_h, s.height);
        }
        const avail_w = @max(40, self.view_w - self.padding * 2);
        const by_width = avail_w / max_w;
        return switch (fit) {
            .width => std.math.clamp(by_width, min_zoom, max_zoom),
            .page => std.math.clamp(@min(by_width, @max(40, self.view_h - self.padding * 2) / max_h), min_zoom, max_zoom),
            .none => null,
        };
    }

    const Anchor = struct { page: usize, px: f32, py: f32, vx: f32, vy: f32 };

    fn anchorCenter(self: *Viewer) ?Anchor {
        return self.anchorAt(self.view_w / 2, self.view_h / 2);
    }

    /// The page point under viewport point (vx, vy), to keep it in place
    /// across a zoom change.
    fn anchorAt(self: *Viewer, vx: f32, vy: f32) ?Anchor {
        const doc = self.doc orelse return null;
        if (self.tops.items.len != doc.pageCount() or self.layout_zoom <= 0) return null;
        const page = self.pageNearY(vy);
        const origin = self.pageOrigin(page);
        return .{ .page = page, .px = (vx - origin[0]) / self.layout_zoom, .py = (vy - origin[1]) / self.layout_zoom, .vx = vx, .vy = vy };
    }

    fn applyFit(self: *Viewer, anchor: ?Anchor) void {
        const z = self.fitZoom(self.fit) orelse return;
        self.setZoomRaw(z, anchor);
    }

    /// Zooms keeping the page point under (vx, vy) fixed (pinch / ⌘ wheel).
    fn zoomAt(self: *Viewer, zoom: f32, vx: f32, vy: f32) void {
        self.setZoomRaw(std.math.clamp(zoom, min_zoom, max_zoom), self.anchorAt(vx, vy));
    }

    fn setZoomRaw(self: *Viewer, zoom: f32, anchor: ?Anchor) void {
        if (@abs(zoom - self.zoom) < 0.0001 and self.layout_zoom == zoom) return;
        self.zoom = zoom;
        self.relayout();
        if (anchor) |a| {
            // place the anchored page point back under the same viewport point
            const origin = self.pageOrigin(a.page);
            self.scroll_x += origin[0] + a.px * self.zoom - a.vx;
            self.scroll_y += origin[1] + a.py * self.zoom - a.vy;
        }
        self.clampScroll();
        self.updateCurrentPage();
    }

    // ---- layout -------------------------------------------------------

    fn relayout(self: *Viewer) void {
        const doc = self.doc orelse return;
        self.tops.resize(self.allocator, doc.pageCount()) catch return;
        var y = self.padding;
        var max_w: f32 = 0;
        for (doc.sizes, 0..) |s, i| {
            self.tops.items[i] = y;
            y += s.height * self.zoom + self.page_gap;
            max_w = @max(max_w, s.width * self.zoom);
        }
        self.max_page_w = max_w;
        self.content_h = if (doc.pageCount() > 0) y - self.page_gap + self.padding else 0;
        self.content_w = max_w + self.padding * 2;
        self.layout_zoom = self.zoom;
    }

    fn maxScrollX(self: *const Viewer) f32 {
        return @max(0, self.content_w - self.view_w);
    }

    fn maxScrollY(self: *const Viewer) f32 {
        return @max(0, self.content_h - self.view_h);
    }

    fn clampScroll(self: *Viewer) void {
        self.scroll_x = std.math.clamp(self.scroll_x, 0, self.maxScrollX());
        self.scroll_y = std.math.clamp(self.scroll_y, 0, self.maxScrollY());
    }

    fn pageSizePx(self: *const Viewer, page: usize) [2]f32 {
        const s = self.doc.?.pageSize(page);
        return .{ s.width * self.zoom, s.height * self.zoom };
    }

    /// Top-left of a page in viewport coordinates.
    fn pageOrigin(self: *const Viewer, page: usize) [2]f32 {
        const size = self.pageSizePx(page);
        const x = if (self.content_w <= self.view_w)
            (self.view_w - size[0]) / 2
        else
            self.padding + (self.max_page_w - size[0]) / 2 - self.scroll_x;
        return .{ x, self.tops.items[page] - self.scroll_y };
    }

    /// Page whose vertical span (with half the gap) contains viewport y.
    fn pageNearY(self: *const Viewer, vy: f32) usize {
        const y = vy + self.scroll_y;
        const n = self.tops.items.len;
        if (n == 0) return 0;
        var lo: usize = 0;
        var hi: usize = n;
        while (lo + 1 < hi) {
            const mid = (lo + hi) / 2;
            if (self.tops.items[mid] - self.page_gap / 2 <= y) lo = mid else hi = mid;
        }
        return lo;
    }

    fn updateCurrentPage(self: *Viewer) void {
        if (self.tops.items.len == 0) return;
        const page = self.pageNearY(self.view_h / 3);
        if (page != self.current_page) {
            self.current_page = page;
            self.page_input_page = null;
        }
    }

    // ---- navigation ---------------------------------------------------

    /// Scrolls to a page (0-based); `y` is a point offset from its top.
    pub fn goToPage(self: *Viewer, page: usize, y: f32) void {
        const doc = self.doc orelse return;
        if (doc.pageCount() == 0) return;
        const p = @min(page, doc.pageCount() - 1);
        if (self.tops.items.len != doc.pageCount() or self.view_h <= 0) {
            self.pending_goto = .{ .page = p, .y = y };
            return;
        }
        self.scroll_y = self.tops.items[p] + y * self.zoom - self.padding;
        self.clampScroll();
        self.current_page = p;
        self.page_input_page = null;
    }

    pub fn nextPage(self: *Viewer) void {
        if (self.current_page + 1 < self.pageCount()) self.goToPage(self.current_page + 1, 0);
    }

    pub fn previousPage(self: *Viewer) void {
        if (self.current_page > 0) self.goToPage(self.current_page - 1, 0);
    }

    fn scrollBy(self: *Viewer, dx: f32, dy: f32) void {
        self.scroll_x += dx;
        self.scroll_y += dy;
        self.clampScroll();
        self.updateCurrentPage();
    }

    // ---- selection ----------------------------------------------------

    /// The selected glyph range, ordered (null when nothing is selected).
    pub fn selection(self: *const Viewer) ?Range {
        const a = self.anchor orelse return null;
        const f = self.focus orelse return null;
        return if (Pos.less(f, a)) .{ .start = f, .end = a } else .{ .start = a, .end = f };
    }

    /// Selects glyphs `start..end` (inclusive; any order).
    pub fn setSelection(self: *Viewer, start: Pos, end: Pos) void {
        self.anchor = start;
        self.focus = end;
        self.anchor_slice = null;
    }

    pub fn clearSelection(self: *Viewer) void {
        self.anchor = null;
        self.focus = null;
        self.anchor_slice = null;
    }

    /// Selects every page's text.
    pub fn selectAll(self: *Viewer) void {
        const doc = self.doc orelse return;
        const n = doc.pageCount();
        if (n == 0) return;
        var last = n;
        while (last > 0) {
            last -= 1;
            const t = doc.pageText(last) orelse continue;
            if (t.count() == 0) continue;
            self.anchor = .{ .page = 0, .index = 0 };
            self.focus = .{ .page = last, .index = t.count() - 1 };
            self.anchor_slice = null;
            return;
        }
    }

    /// UTF-8 text of the selection (pages separated by a line break).
    pub fn selectedText(self: *Viewer, allocator: std.mem.Allocator) ![]u8 {
        var out: std.ArrayListUnmanaged(u8) = .empty;
        errdefer out.deinit(allocator);
        const doc = self.doc orelse return out.toOwnedSlice(allocator);
        const range = self.selection() orelse return out.toOwnedSlice(allocator);
        var page = range.start.page;
        while (page <= range.end.page) : (page += 1) {
            const t = doc.pageText(page) orelse continue;
            if (t.count() == 0) continue;
            const from = if (page == range.start.page) range.start.index else 0;
            const to = if (page == range.end.page) range.end.index else t.count() - 1;
            if (page != range.start.page and out.items.len > 0 and out.items[out.items.len - 1] != '\n') try out.append(allocator, '\n');
            try t.appendText(allocator, &out, from, @min(to, t.count() - 1));
        }
        return out.toOwnedSlice(allocator);
    }

    /// Copies the selection to the clipboard; false when nothing is selected.
    pub fn copySelection(self: *Viewer) bool {
        const value = self.selectedText(self.allocator) catch return false;
        defer self.allocator.free(value);
        if (value.len == 0) return false;
        return platform.setClipboard(value);
    }

    /// Glyph position under a viewport point: exact / tolerant hit on the
    /// page under the point, else the nearest glyph of the nearest page.
    fn posAt(self: *Viewer, vx: f32, vy: f32, exact: bool) ?Pos {
        const doc = self.doc orelse return null;
        if (self.tops.items.len == 0) return null;
        const page = self.pageNearY(vy);
        const origin = self.pageOrigin(page);
        const px = (vx - origin[0]) / self.zoom;
        const py = (vy - origin[1]) / self.zoom;
        const t = doc.pageText(page) orelse return null;
        if (t.glyphAt(px, py, 1.5)) |i| return .{ .page = page, .index = i };
        if (exact) return null;
        if (t.nearestGlyph(px, py)) |i| return .{ .page = page, .index = i };
        return null;
    }

    fn linkAt(self: *Viewer, vx: f32, vy: f32) ?document_mod.Link {
        const doc = self.doc orelse return null;
        if (self.tops.items.len == 0) return null;
        const page = self.pageNearY(vy);
        const origin = self.pageOrigin(page);
        const px = (vx - origin[0]) / self.zoom;
        const py = (vy - origin[1]) / self.zoom;
        for (doc.pageLinks(page)) |l| if (l.rect.contains(px, py)) return l;
        return null;
    }

    fn sliceFor(self: *Viewer, pos: Pos, mode: DragMode) [2]Pos {
        const t = self.doc.?.pageText(pos.page) orelse return .{ pos, pos };
        const s = switch (mode) {
            .char => return .{ pos, pos },
            .word => t.wordAt(pos.index),
            .line => t.lineAt(pos.index),
        } orelse return .{ pos, pos };
        return .{ .{ .page = pos.page, .index = s.from }, .{ .page = pos.page, .index = s.to } };
    }

    fn extendTo(self: *Viewer, pos: Pos) void {
        const mode = self.drag orelse .char;
        if (mode == .char or self.anchor_slice == null) {
            self.focus = pos;
            return;
        }
        // word / line granularity: grow from the anchored word or line
        const a = self.anchor_slice.?;
        const b = self.sliceFor(pos, mode);
        if (Pos.less(b[0], a[0])) {
            self.anchor = a[1];
            self.focus = b[0];
        } else {
            self.anchor = a[0];
            self.focus = if (Pos.less(b[1], a[1])) a[1] else b[1];
        }
    }

    // ---- search -------------------------------------------------------

    /// Starts a search (empty clears it). Results arrive over the next
    /// builds (a few pages per frame); `matches` lists them.
    pub fn search(self: *Viewer, query: []const u8) void {
        if (std.mem.eql(u8, self.query.items, query)) return;
        self.query.clearRetainingCapacity();
        self.query.appendSlice(self.allocator, query) catch return;
        self.matches.clearRetainingCapacity();
        self.active_match = null;
        self.search_page = 0;
        self.search_done = query.len == 0;
    }

    pub fn searchResults(self: *const Viewer) []const Match {
        return self.matches.items;
    }

    pub fn nextMatch(self: *Viewer) void {
        if (self.matches.items.len == 0) return;
        self.active_match = if (self.active_match) |m| (m + 1) % self.matches.items.len else self.firstMatchFromCurrentPage();
        self.revealMatch();
    }

    pub fn previousMatch(self: *Viewer) void {
        if (self.matches.items.len == 0) return;
        const n = self.matches.items.len;
        self.active_match = if (self.active_match) |m| (m + n - 1) % n else self.firstMatchFromCurrentPage();
        self.revealMatch();
    }

    fn firstMatchFromCurrentPage(self: *const Viewer) usize {
        for (self.matches.items, 0..) |m, i| if (m.page >= self.current_page) return i;
        return 0;
    }

    /// Searches pages for at most ~6 ms; true when more pages remain.
    fn continueSearch(self: *Viewer) bool {
        const doc = self.doc orelse return false;
        if (self.search_done) return false;
        const start = platform.monotonicMs();
        const had = self.matches.items.len;
        while (self.search_page < doc.pageCount()) {
            if (doc.pageText(self.search_page)) |t| {
                var found: std.ArrayListUnmanaged(text_mod.Slice) = .empty;
                defer found.deinit(self.allocator);
                t.search(self.allocator, self.query.items, &found) catch {};
                for (found.items) |s| self.matches.append(self.allocator, .{ .page = self.search_page, .from = s.from, .to = s.to }) catch {};
            }
            self.search_page += 1;
            if (platform.monotonicMs() - start > 6) break;
        }
        if (self.search_page >= doc.pageCount()) self.search_done = true;
        // jump to the first result as soon as there is one
        if (had == 0 and self.matches.items.len > 0 and self.active_match == null) {
            self.active_match = self.firstMatchFromCurrentPage();
            self.revealMatch();
        }
        return !self.search_done;
    }

    fn revealMatch(self: *Viewer) void {
        const doc = self.doc orelse return;
        const m = self.matches.items[self.active_match orelse return];
        const t = doc.pageText(m.page) orelse return;
        const rects = t.rects(self.allocator, m.from, m.to) catch return;
        defer self.allocator.free(rects);
        if (rects.len == 0 or self.tops.items.len == 0) return self.goToPage(m.page, 0);
        const r = rects[0];
        const origin = self.pageOrigin(m.page);
        const top = origin[1] + r.y * self.zoom;
        const left = origin[0] + r.x * self.zoom;
        // keep it visible with some room above (centre when far away)
        if (top < 24 or top + r.height * self.zoom > self.view_h - 24) self.scroll_y += top - self.view_h / 3;
        if (left < 16 or left + r.width * self.zoom > self.view_w - 16) self.scroll_x += left - self.view_w / 3;
        self.clampScroll();
        self.updateCurrentPage();
    }

    // ---- annotations --------------------------------------------------

    /// Switches the pointer tool (clears the text selection).
    pub fn setTool(self: *Viewer, tool: Tool) void {
        self.tool = tool;
        self.clearSelection();
        self.ink_points.clearRetainingCapacity();
        if (tool != .select) self.selectAnnotation(null);
    }

    pub fn selectedAnnotation(self: *const Viewer) ?*const annots.Annotation {
        return self.annotations.get(self.selected_annotation orelse return null);
    }

    /// Node key of an annotation's box (for `ui.keyBounds`, e.g. to place
    /// a comment editor next to the selected annotation).
    pub fn annotationKey(buf: []u8, key_prefix: []const u8, id: u64) []const u8 {
        return std.fmt.bufPrint(buf, "{s}-a{d}", .{ key_prefix, id }) catch "";
    }

    pub fn selectAnnotation(self: *Viewer, id: ?u64) void {
        if (std.meta.eql(self.selected_annotation, id)) return;
        self.selected_annotation = id;
        self.annotationChanged();
    }

    fn annotationChanged(self: *Viewer) void {
        if (self.options.on_annotation) |a| a.func(a.ctx);
    }

    pub fn defaultColor(kind: annots.Kind) u32 {
        return switch (kind) {
            .highlight => annots.colors.yellow,
            .underline, .squiggly => 0xff1677ff,
            .strikeout, .ink => annots.colors.red,
            .note => annots.colors.note,
        };
    }

    /// Turns the text selection into markup annotations (one per page);
    /// returns the id of the last one.
    pub fn annotateSelection(self: *Viewer, kind: annots.Kind, color: ?u32) ?u64 {
        const doc = self.doc orelse return null;
        const range = self.selection() orelse return null;
        if (!kind.isMarkup()) return null;
        var last: ?u64 = null;
        var page = range.start.page;
        while (page <= range.end.page) : (page += 1) {
            const t = doc.pageText(page) orelse continue;
            if (t.count() == 0) continue;
            const from = if (page == range.start.page) range.start.index else 0;
            const to = @min(if (page == range.end.page) range.end.index else t.count() - 1, t.count() - 1);
            if (from > to) continue;
            const rects = t.rects(self.allocator, from, to) catch continue;
            defer self.allocator.free(rects);
            if (rects.len == 0) continue;
            var text_buf: std.ArrayListUnmanaged(u8) = .empty;
            defer text_buf.deinit(self.allocator);
            t.appendText(self.allocator, &text_buf, from, to) catch {};
            last = self.annotations.add(.{
                .id = 0,
                .page = page,
                .kind = kind,
                .color = color orelse defaultColor(kind),
                .rects = rects,
                .text = text_buf.items,
            }) catch null;
        }
        if (last != null) {
            self.clearSelection();
            self.annotationChanged();
        }
        return last;
    }

    pub fn removeAnnotation(self: *Viewer, id: u64) void {
        if (self.annotations.remove(id)) {
            if (self.selected_annotation == id) self.selected_annotation = null;
            self.annotationChanged();
        }
    }

    pub fn setAnnotationContents(self: *Viewer, id: u64, contents: []const u8) void {
        self.annotations.setContents(id, contents) catch return;
        self.annotationChanged();
    }

    pub fn setAnnotationColor(self: *Viewer, id: u64, color: u32) void {
        self.annotations.setColor(id, color) catch return;
        self.annotationChanged();
    }

    pub fn undo(self: *Viewer) bool {
        if (!self.annotations.undo()) return false;
        if (self.selected_annotation) |id| if (self.annotations.get(id) == null) {
            self.selected_annotation = null;
        };
        self.annotationChanged();
        return true;
    }

    pub fn redo(self: *Viewer) bool {
        if (!self.annotations.redo()) return false;
        self.annotationChanged();
        return true;
    }

    /// Writes a copy of the open PDF with the annotations to `out_path`
    /// (standard PDF annotations other viewers show and edit).
    pub fn saveAnnotated(self: *Viewer, out_path: []const u8) !void {
        if (self.doc == null or self.path.items.len == 0) return error.NoDocument;
        const a = self.allocator;
        var arena_state = std.heap.ArenaAllocator.init(a);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        const list = self.annotations.all();
        const out = try arena.alloc(native.Annotation, list.len);
        for (list, out) |ann, *o| {
            const rects = try arena.alloc(f32, ann.rects.len * 4);
            for (ann.rects, 0..) |r, i| {
                rects[i * 4 + 0] = r.x;
                rects[i * 4 + 1] = r.y;
                rects[i * 4 + 2] = r.width;
                rects[i * 4 + 3] = r.height;
            }
            var npoints: usize = 0;
            for (ann.strokes) |st| npoints += st.len;
            const points = try arena.alloc(f32, npoints * 2);
            const lengths = try arena.alloc(c_int, ann.strokes.len);
            var k: usize = 0;
            for (ann.strokes, 0..) |st, si| {
                lengths[si] = @intCast(st.len);
                for (st) |pt| {
                    points[k * 2] = pt[0];
                    points[k * 2 + 1] = pt[1];
                    k += 1;
                }
            }
            o.* = .{
                .page = @intCast(ann.page),
                .kind = @backingInt(ann.kind),
                .color = ann.color,
                .width = ann.width,
                .rects = rects.ptr,
                .rect_count = @intCast(ann.rects.len),
                .points = points.ptr,
                .stroke_lengths = lengths.ptr,
                .stroke_count = @intCast(ann.strokes.len),
                .contents = (try arena.dupeSentinel(u8, ann.contents, 0)).ptr,
            };
        }
        const src = try arena.dupeSentinel(u8, self.path.items, 0);
        const dst = try arena.dupeSentinel(u8, out_path, 0);
        if (native.zant_pdf_save_annotated(src.ptr, null, dst.ptr, out.ptr, @intCast(out.len)) != 0) return error.SaveFailed;
    }

    const PagePoint = struct { page: usize, x: f32, y: f32 };

    /// Page and page point under a viewport point (nearest page by y).
    fn pagePoint(self: *const Viewer, vx: f32, vy: f32) ?PagePoint {
        if (self.tops.items.len == 0) return null;
        const page = self.pageNearY(vy);
        const origin = self.pageOrigin(page);
        return .{ .page = page, .x = (vx - origin[0]) / self.zoom, .y = (vy - origin[1]) / self.zoom };
    }

    fn annotationAt(self: *const Viewer, vx: f32, vy: f32) ?u64 {
        const pp = self.pagePoint(vx, vy) orelse return null;
        const list = self.annotations.all();
        var i = list.len;
        while (i > 0) {
            i -= 1;
            const a = &list[i];
            if (a.page == pp.page and a.hit(pp.x, pp.y)) return a.id;
        }
        return null;
    }

    fn finishInk(self: *Viewer) void {
        defer self.ink_points.clearRetainingCapacity();
        if (self.ink_points.items.len == 0) return;
        const stroke: []const annots.Point = self.ink_points.items;
        const strokes = [_][]const annots.Point{stroke};
        const color = self.tool_color orelse defaultColor(.ink);
        _ = self.annotations.add(.{ .id = 0, .page = self.ink_page, .kind = .ink, .color = color, .strokes = &strokes, .width = 2 }) catch return;
        self.annotationChanged();
    }

    // ---- events -------------------------------------------------------

    fn changed(self: *Viewer) void {
        if (self.options.on_change) |a| a.func(a.ctx);
    }

    fn onPointer(self: *Viewer, info: ui.PointerInfo) bool {
        if (self.doc == null) return false;
        switch (info.phase) {
            .scroll => {
                if (info.modifiers.control or info.modifiers.meta) {
                    // ⌘ / Ctrl + wheel and pinch (Control + wheel, 250 per
                    // unit of magnification) zoom around the pointer; one
                    // wheel notch (~60) is about 1.27×
                    const factor = @exp(std.math.clamp(info.dy, -120, 120) * 0.004);
                    self.fit = .none;
                    self.zoomAt(self.zoom * factor, info.x, info.y);
                } else {
                    self.scrollBy(-info.dx, -info.dy);
                }
                self.changed();
                return true;
            },
            .hover => {
                if (self.tool == .ink or self.tool == .note) return false;
                const next: @TypeOf(self.hover) = if (self.tool == .select and self.annotationAt(info.x, info.y) != null)
                    .link
                else if (self.tool == .select and self.linkAt(info.x, info.y) != null)
                    .link
                else if (self.posAt(info.x, info.y, true) != null) .text else .none;
                if (next == self.hover) return false;
                self.hover = next;
                return true;
            },
            .leave => {
                if (self.hover == .none) return false;
                self.hover = .none;
                return true;
            },
            .down => {
                self.down_x = info.x;
                self.down_y = info.y;
                self.drag_moved = false;
                self.pressed_link = null;
                switch (self.tool) {
                    .ink => {
                        const pp = self.pagePoint(info.x, info.y) orelse return false;
                        self.ink_page = pp.page;
                        self.ink_points.clearRetainingCapacity();
                        self.ink_points.append(self.allocator, .{ pp.x, pp.y }) catch {};
                        return true;
                    },
                    .note => {
                        const pp = self.pagePoint(info.x, info.y) orelse return false;
                        const size: f32 = 18;
                        const rect = [_]annots.Rect{.{ .x = pp.x - size / 2, .y = pp.y - size / 2, .width = size, .height = size }};
                        const id = self.annotations.add(.{ .id = 0, .page = pp.page, .kind = .note, .color = self.tool_color orelse defaultColor(.note), .rects = &rect }) catch return false;
                        self.selected_annotation = id;
                        self.annotationChanged();
                        return true;
                    },
                    .select => if (!info.modifiers.shift and info.click_count == 1) {
                        if (self.annotationAt(info.x, info.y)) |id| {
                            self.clearSelection();
                            self.drag = null;
                            self.selectAnnotation(id);
                            return true;
                        }
                        self.selectAnnotation(null);
                    },
                    else => {},
                }
                if (!info.modifiers.shift and info.click_count == 1) {
                    if (self.linkAt(info.x, info.y)) |l| {
                        self.pressed_link = l;
                        return false;
                    }
                }
                const mode: DragMode = switch (info.click_count) {
                    1 => .char,
                    2 => .word,
                    else => .line,
                };
                self.drag = mode;
                const pos = self.posAt(info.x, info.y, false);
                if (info.modifiers.shift and self.anchor != null) {
                    if (pos) |p| self.focus = p;
                    self.drag_moved = true;
                    return true;
                }
                if (pos) |p| {
                    if (mode == .char) {
                        self.anchor = p;
                        self.focus = p;
                        self.anchor_slice = null;
                    } else {
                        const s = self.sliceFor(p, mode);
                        self.anchor = s[0];
                        self.focus = s[1];
                        self.anchor_slice = s;
                        self.drag_moved = true;
                    }
                } else self.clearSelection();
                return true;
            },
            .drag => {
                if (self.tool == .ink) {
                    if (self.ink_points.items.len == 0) return false;
                    const origin = self.pageOrigin(self.ink_page);
                    const size = self.doc.?.pageSize(self.ink_page);
                    const x = std.math.clamp((info.x - origin[0]) / self.zoom, 0, size.width);
                    const y = std.math.clamp((info.y - origin[1]) / self.zoom, 0, size.height);
                    const last = self.ink_points.items[self.ink_points.items.len - 1];
                    // skip sub-pixel moves (keeps strokes light)
                    if (@abs(last[0] - x) + @abs(last[1] - y) < 0.75 / self.zoom) return false;
                    self.ink_points.append(self.allocator, .{ x, y }) catch {};
                    return true;
                }
                if (self.pressed_link != null) return false;
                if (self.drag == null) return false;
                if (@abs(info.x - self.down_x) + @abs(info.y - self.down_y) > 3) self.drag_moved = true;
                // auto-scroll when dragging past the viewport edges
                if (info.y < 0) self.scrollBy(0, info.y * 0.5) else if (info.y > info.height) self.scrollBy(0, (info.y - info.height) * 0.5);
                if (self.posAt(std.math.clamp(info.x, 0, info.width), std.math.clamp(info.y, 0, info.height), false)) |p| self.extendTo(p);
                return true;
            },
            .up => {
                if (self.pressed_link) |l| {
                    self.pressed_link = null;
                    if (@abs(info.x - self.down_x) + @abs(info.y - self.down_y) <= 4) self.activateLink(l);
                    return true;
                }
                if (self.tool == .ink) {
                    self.finishInk();
                    return true;
                }
                const was_dragging = self.drag != null;
                self.drag = null;
                // a click without a drag clears the selection (and selects nothing)
                if (was_dragging and !self.drag_moved) self.clearSelection();
                // markup tools annotate what was dragged over
                if (self.tool.markupKind()) |kind| if (self.selection() != null and self.drag_moved) {
                    _ = self.annotateSelection(kind, self.tool_color);
                };
                self.changed();
                return true;
            },
        }
    }

    fn activateLink(self: *Viewer, l: document_mod.Link) void {
        if (l.dest_page) |p| {
            self.goToPage(p, l.dest_y orelse 0);
            self.changed();
            return;
        }
        if (l.uri.len == 0) return;
        if (self.options.on_link) |a| a.func(a.ctx, l.uri) else _ = dialogs.openUrl(l.uri);
    }

    fn onKey(self: *Viewer, k: events.KeyEvent) bool {
        const primary = keys.primary(k.modifiers);
        if (primary) {
            if (events.characterEquals(k.key, 'z')) {
                _ = if (k.modifiers.shift) self.redo() else self.undo();
                return true;
            }
            if (events.characterEquals(k.key, 'y')) {
                _ = self.redo();
                return true;
            }
            if (events.characterEquals(k.key, 'c')) return self.copySelection() or true;
            if (events.characterEquals(k.key, 'a')) {
                self.selectAll();
                return true;
            }
            if (events.characterEquals(k.key, '=') or events.characterEquals(k.key, '+')) {
                self.zoomIn();
                self.changed();
                return true;
            }
            if (events.characterEquals(k.key, '-')) {
                self.zoomOut();
                self.changed();
                return true;
            }
            if (events.characterEquals(k.key, '0')) {
                self.setFit(.width);
                self.changed();
                return true;
            }
            if (events.characterEquals(k.key, 'f') and self.options.toolbar) {
                var buf: [96]u8 = undefined;
                ui.focus(std.fmt.bufPrint(&buf, "{s}-search", .{self.options.key}) catch return false);
                return true;
            }
            return false;
        }
        switch (k.key) {
            .escape => {
                if (self.selected_annotation != null) {
                    self.selectAnnotation(null);
                    return true;
                }
                if (self.selection() != null) {
                    self.clearSelection();
                    return true;
                }
                if (self.tool != .select) {
                    self.setTool(.select);
                    self.changed();
                    return true;
                }
                return false;
            },
            .backspace, .delete => {
                const id = self.selected_annotation orelse return false;
                self.removeAnnotation(id);
                return true;
            },
            .up => self.scrollBy(0, -48),
            .down => self.scrollBy(0, 48),
            .left => self.scrollBy(-48, 0),
            .right => self.scrollBy(48, 0),
            .home => {
                self.scroll_y = 0;
                self.updateCurrentPage();
            },
            .end => {
                self.scroll_y = self.maxScrollY();
                self.updateCurrentPage();
            },
            .character => |c| {
                if (c != ' ') return false;
                self.scrollBy(0, if (k.modifiers.shift) -self.view_h * 0.9 else self.view_h * 0.9);
            },
            else => return false,
        }
        self.changed();
        return true;
    }

    fn onResize(self: *Viewer) void {
        _ = self; // the rebuild that follows reads the new size
    }

    fn onScrollbar(self: *Viewer, info: ui.PointerInfo, horizontal: bool) bool {
        const track = if (horizontal) self.view_w else self.view_h;
        const content = if (horizontal) self.content_w else self.content_h;
        if (content <= track) return false;
        const pos = if (horizontal) info.window_x else info.window_y;
        switch (info.phase) {
            .down => self.bar_drag = .{ .start = pos, .scroll = if (horizontal) self.scroll_x else self.scroll_y, .horizontal = horizontal },
            .drag => {
                const d = self.bar_drag orelse return false;
                const ratio = content / track;
                if (horizontal) self.scroll_x = d.scroll + (pos - d.start) * ratio else self.scroll_y = d.scroll + (pos - d.start) * ratio;
                self.clampScroll();
                self.updateCurrentPage();
            },
            .up => self.bar_drag = null,
            else => return false,
        }
        return true;
    }

    fn onVBar(self: *Viewer, info: ui.PointerInfo) bool {
        return self.onScrollbar(info, false);
    }

    fn onHBar(self: *Viewer, info: ui.PointerInfo) bool {
        return self.onScrollbar(info, true);
    }

    // toolbar actions
    fn tbPrev(self: *Viewer) void {
        self.previousPage();
        self.changed();
    }
    fn tbNext(self: *Viewer) void {
        self.nextPage();
        self.changed();
    }
    fn tbZoomIn(self: *Viewer) void {
        self.zoomIn();
        self.changed();
    }
    fn tbZoomOut(self: *Viewer) void {
        self.zoomOut();
        self.changed();
    }
    fn tbFit(self: *Viewer) void {
        self.setFit(if (self.fit == .width) .page else .width);
        self.changed();
    }
    fn tbSearch(self: *Viewer, value: []const u8) void {
        self.search(value);
        self.changed();
    }
    fn tbSearchNext(self: *Viewer) void {
        self.nextMatch();
        self.changed();
    }
    fn tbSearchPrev(self: *Viewer) void {
        self.previousMatch();
        self.changed();
    }
    fn tbSearchKey(self: *Viewer, k: events.KeyEvent, value: []const u8, anchor: usize, cursor: usize) bool {
        _ = value;
        _ = anchor;
        _ = cursor;
        switch (k.key) {
            .enter => {
                if (k.modifiers.shift) self.previousMatch() else self.nextMatch();
                self.changed();
                return true;
            },
            .escape => {
                self.search("");
                self.changed();
                return false;
            },
            else => return false,
        }
    }
    fn tbPageInput(self: *Viewer, value: []const u8) void {
        self.page_input.clearRetainingCapacity();
        self.page_input.appendSlice(self.allocator, value) catch {};
        self.page_input_page = self.current_page;
    }
    fn tbPageSubmit(self: *Viewer) void {
        const trimmed = std.mem.trim(u8, self.page_input.items, " ");
        const n = std.fmt.parseInt(usize, trimmed, 10) catch 0;
        if (n >= 1) self.goToPage(n - 1, 0);
        self.page_input_page = null;
        self.changed();
    }
};

// ---- building -----------------------------------------------------------

fn snap(v: f32, scale: f32) f32 {
    return @round(v * scale) / scale;
}

fn frameKey(comptime fmt: []const u8, args: anytype) []const u8 {
    return std.fmt.allocPrint(ui.frameAllocator(), fmt, args) catch "pdf-key";
}

/// The viewer UI. Fills its parent (give the parent a size, or place it in
/// a column with `grow`).
pub fn view(v: *Viewer, options: ViewOptions) ui.Node {
    v.options = options;
    v.padding = options.padding;
    v.page_gap = options.page_gap;
    const theme = ui.currentTheme();
    const arena = ui.frameAllocator();
    const vp_key = frameKey("{s}-vp", .{options.key});

    var children: std.ArrayListUnmanaged(ui.Node) = .empty;
    if (options.toolbar and v.doc != null) {
        children.append(arena, toolbar(v, options, theme)) catch {};
        children.append(arena, ui.styled(ui.column(.{}), .{ .height = 1, .width_percent = 100, .background = theme.color_split })) catch {};
    }

    // viewport size of the last layout (on_resize rebuilds when it changes)
    if (ui.keyBounds(vp_key)) |b| {
        const resized = @abs(b.width - v.view_w) > 0.5 or @abs(b.height - v.view_h) > 0.5;
        const anchor = if (resized) v.anchorAt(0, 0) else null;
        v.view_w = b.width;
        v.view_h = b.height;
        if (resized) {
            if (v.fit != .none) v.applyFit(anchor) else v.clampScroll();
        }
    }
    if (v.doc != null) {
        if (v.layout_zoom == 0) {
            if (v.fit != .none) {
                if (v.fitZoom(v.fit)) |z| v.zoom = z;
            }
            v.relayout();
        } else if (v.layout_zoom != v.zoom) v.relayout();
        if (v.pending_goto) |g| if (v.view_h > 0) {
            v.pending_goto = null;
            v.goToPage(g.page, g.y);
        };
        v.clampScroll();
        // search once the layout is known (results reveal relative to it)
        if (v.view_h > 0 and v.continueSearch()) ui.wakeAfter(1);
    }

    var vp_children: std.ArrayListUnmanaged(ui.Node) = .empty;
    if (v.doc) |doc| {
        if (v.renderer) |r| r.beginFrame();
        if (v.view_h > 0 and v.tops.items.len == doc.pageCount()) {
            buildPages(v, doc, &vp_children, theme, options) catch {};
            buildScrollbars(v, &vp_children, theme, options) catch {};
            if (options.selection_menu and v.tool == .select and v.drag == null) selectionMenu(v, &vp_children, theme, options) catch {};
        }
    } else {
        const message = if (v.open_error) |err| switch (err) {
            error.PasswordRequired => options.labels.password,
            else => options.labels.cannot_open,
        } else "";
        if (message.len > 0) vp_children.append(arena, ui.column(.{
            .width_percent = 100,
            .height_percent = 100,
            .justify = .center,
            .align_items = .center,
            .children = &.{ui.text(message, .{ .size = 14, .color = theme.color_text_secondary })},
        })) catch {};
    }

    const cursor: ui.Cursor = if (v.tool == .ink or v.tool == .note) .crosshair else switch (v.hover) {
        .link => .pointing_hand,
        .text => .ibeam,
        .none => .arrow,
    };
    children.append(arena, ui.pointerArea(.{
        .box = .{
            .key = vp_key,
            .grow = 1,
            .shrink = 1,
            .basis = 0,
            .width_percent = 100,
            .clip = true,
            .background = options.background orelse theme.color_bg_layout,
            .on_resize = ui.action(v, Viewer.onResize),
            .children = vp_children.items,
        },
        .on_pointer = ui.pointerAction(v, Viewer.onPointer),
        .capture_scroll = true,
        .cursor = cursor,
        .focusable = true,
        .on_key = ui.keyAction(v, Viewer.onKey),
    })) catch {};

    return ui.column(.{
        .key = options.key,
        .grow = 1,
        .width_percent = 100,
        .height_percent = 100,
        .align_items = .stretch,
        .children = children.items,
    });
}

fn buildPages(v: *Viewer, doc: *Document, out: *std.ArrayListUnmanaged(ui.Node), theme: tokens.Theme, options: ViewOptions) !void {
    const arena = ui.frameAllocator();
    const scale = ui.displayScale();
    const r = v.renderer;
    const selection = v.selection();
    const sel_color = tokens.withAlpha(theme.color_primary, 0x55);
    const match_color: u32 = 0x66ffd400;
    const active_color: u32 = 0x99ff9100;
    // pages intersecting the viewport, plus one screen of prefetch
    const first = v.pageNearY(-v.view_h * 0.5);
    var page = first;
    while (page < doc.pageCount()) : (page += 1) {
        const origin = v.pageOrigin(page);
        if (origin[1] > v.view_h * 1.5) break;
        const size = v.pageSizePx(page);
        if (origin[1] + size[1] < -v.view_h * 0.5) continue;
        const visible = origin[1] < v.view_h and origin[1] + size[1] > 0;
        const x = snap(origin[0], scale);
        const y = snap(origin[1], scale);
        var layers: std.ArrayListUnmanaged(ui.Node) = .empty;

        // base layer: a small whole-page image (or the closest ready one)
        const page_px_w: u32 = @max(1, @as(u32, @intFromFloat(@round(size[0] * scale))));
        if (r) |rr| {
            const base_w = @min(renderer_mod.base_width, page_px_w);
            const base = rr.tile(.{ .page = @intCast(page), .page_w = base_w, .tx = 0, .ty = 0 });
            const under = rr.anyPageImage(@intCast(page), page_px_w) orelse base;
            if (under) |img| try layers.append(arena, fill(ui.image(.{ .key = frameKey("{s}-p{d}-b", .{ options.key, page }), .image = img, .fit = .fill, .radius = 0 }), 0, 0, size[0], size[1]));
            // sharp tiles for the visible part
            if (visible) try tiles(v, rr, page, page_px_w, size, x, y, &layers, options);
        }
        // annotations, search matches and selection
        for (v.annotations.all()) |*a| if (a.page == page) try annotationLayer(v, a, theme, options, &layers);
        if (v.tool == .ink and v.ink_points.items.len > 0 and v.ink_page == page) {
            const strokes = [_][]const annots.Point{v.ink_points.items};
            try layers.append(arena, inkNode(arena, &strokes, v.tool_color orelse Viewer.defaultColor(.ink), 2, v.zoom, size, null));
        }
        if (doc.pageText(page)) |t| if (t.count() > 0) {
            for (v.matches.items, 0..) |m, i| {
                if (m.page != page) continue;
                const color = if (v.active_match != null and v.active_match.? == i) active_color else match_color;
                try highlight(arena, t, m.from, m.to, v.zoom, color, &layers);
            }
            if (selection) |sel| if (page >= sel.start.page and page <= sel.end.page) {
                const from = if (page == sel.start.page) sel.start.index else 0;
                const to = if (page == sel.end.page) sel.end.index else t.count() - 1;
                if (from <= to) try highlight(arena, t, from, @min(to, t.count() - 1), v.zoom, sel_color, &layers);
            };
        };
        try out.append(arena, ui.styled(ui.column(.{
            .key = frameKey("{s}-p{d}", .{ options.key, page }),
            .children = layers.items,
        }), .{
            .absolute = true,
            .left = x,
            .top = y,
            .width = size[0],
            .height = size[1],
            .background = 0xffffffff,
            .shadow_color = 0x1f000000,
            .shadow_blur = 6,
            .shadow_offset_y = 1,
        }));
    }
}

fn fill(node: ui.Node, x: f32, y: f32, w: f32, h: f32) ui.Node {
    return ui.styled(node, .{ .absolute = true, .left = x, .top = y, .width = w, .height = h });
}

fn tiles(v: *Viewer, r: *Renderer, page: usize, page_px_w: u32, size: [2]f32, x: f32, y: f32, out: *std.ArrayListUnmanaged(ui.Node), options: ViewOptions) !void {
    const arena = ui.frameAllocator();
    const page_px_h = r.pageHeightFor(@intCast(page), page_px_w);
    // device px per logical px of this page image
    const k = @as(f32, @floatFromInt(page_px_w)) / size[0];
    const ts: f32 = @floatFromInt(renderer_mod.tile_size);
    const cols: u32 = (page_px_w + renderer_mod.tile_size - 1) / renderer_mod.tile_size;
    const rows: u32 = (page_px_h + renderer_mod.tile_size - 1) / renderer_mod.tile_size;
    var ty: u32 = 0;
    while (ty < rows) : (ty += 1) {
        const top = @as(f32, @floatFromInt(ty)) * ts / k;
        const h = @min(ts, @as(f32, @floatFromInt(page_px_h - ty * renderer_mod.tile_size))) / k;
        if (y + top > v.view_h or y + top + h < 0) continue;
        var tx: u32 = 0;
        while (tx < cols) : (tx += 1) {
            const left = @as(f32, @floatFromInt(tx)) * ts / k;
            const w = @min(ts, @as(f32, @floatFromInt(page_px_w - tx * renderer_mod.tile_size))) / k;
            if (x + left > v.view_w or x + left + w < 0) continue;
            const img = r.tile(.{ .page = @intCast(page), .page_w = page_px_w, .tx = @intCast(tx), .ty = @intCast(ty) }) orelse continue;
            try out.append(arena, fill(ui.image(.{ .key = frameKey("{s}-p{d}-t{d}-{d}-{d}", .{ options.key, page, page_px_w, tx, ty }), .image = img, .fit = .fill, .radius = 0 }), left, top, w, h));
        }
    }
}

fn highlight(arena: std.mem.Allocator, t: *const text_mod.PageText, from: usize, to: usize, zoom: f32, color: u32, out: *std.ArrayListUnmanaged(ui.Node)) !void {
    const rects = try t.rects(arena, from, to);
    for (rects) |rc| try out.append(arena, ui.styled(ui.column(.{}), .{
        .absolute = true,
        .left = rc.x * zoom,
        .top = rc.y * zoom,
        .width = rc.width * zoom,
        .height = rc.height * zoom,
        .background = color,
        .radius = 1,
    }));
}

fn buildScrollbars(v: *Viewer, out: *std.ArrayListUnmanaged(ui.Node), theme: tokens.Theme, options: ViewOptions) !void {
    const arena = ui.frameAllocator();
    const thumb_color = tokens.withAlpha(theme.color_text, if (v.bar_drag != null) 0x70 else 0x40);
    if (v.content_h > v.view_h + 0.5) {
        const track = v.view_h - 8;
        const thumb = @max(28, track * v.view_h / v.content_h);
        const top = 4 + (track - thumb) * (v.scroll_y / v.maxScrollY());
        try out.append(arena, ui.pointerArea(.{
            .box = .{ .key = frameKey("{s}-vbar", .{options.key}), .absolute = true, .right = 0, .top = top, .width = 12, .height = thumb, .align_items = .center, .children = &.{
                ui.styled(ui.column(.{}), .{ .width = 6, .height = thumb, .radius = 3, .background = thumb_color }),
            } },
            .on_pointer = ui.pointerAction(v, Viewer.onVBar),
        }));
    }
    if (v.content_w > v.view_w + 0.5) {
        const track = v.view_w - 8;
        const thumb = @max(28, track * v.view_w / v.content_w);
        const left = 4 + (track - thumb) * (v.scroll_x / v.maxScrollX());
        try out.append(arena, ui.pointerArea(.{
            .box = .{ .key = frameKey("{s}-hbar", .{options.key}), .absolute = true, .bottom = 0, .left = left, .width = thumb, .height = 12, .justify = .center, .flex_direction = .column, .children = &.{
                ui.styled(ui.column(.{}), .{ .width = thumb, .height = 6, .radius = 3, .background = thumb_color }),
            } },
            .on_pointer = ui.pointerAction(v, Viewer.onHBar),
        }));
    }
}

fn toolbar(v: *Viewer, options: ViewOptions, theme: tokens.Theme) ui.Node {
    const arena = ui.frameAllocator();
    const labels = options.labels;
    const count = v.pageCount();
    const page_text = if (v.page_input_page != null and v.page_input_page.? == v.current_page)
        arena.dupe(u8, v.page_input.items) catch ""
    else
        std.fmt.allocPrint(arena, "{d}", .{v.current_page + 1}) catch "";
    const zoom_text = std.fmt.allocPrint(arena, "{d}%", .{@as(u32, @intFromFloat(@round(v.zoom * 100)))}) catch "";
    const match_text = if (v.query.items.len == 0)
        ""
    else if (v.matches.items.len == 0)
        (if (v.search_done) labels.no_results else "…")
    else
        std.fmt.allocPrint(arena, "{d}/{d}", .{ (v.active_match orelse 0) + 1, v.matches.items.len }) catch "";

    const children = arena.dupe(ui.Node, &.{
        ui.iconButton(.{ .key = frameKey("{s}-prev", .{options.key}), .icon = .chevron_up, .size = 16, .box = 28, .disabled = v.current_page == 0, .tooltip = labels.previous_page, .on_click = ui.action(v, Viewer.tbPrev) }),
        ui.iconButton(.{ .key = frameKey("{s}-next", .{options.key}), .icon = .chevron_down, .size = 16, .box = 28, .disabled = v.current_page + 1 >= count, .tooltip = labels.next_page, .on_click = ui.action(v, Viewer.tbNext) }),
        ui.input(.{ .key = frameKey("{s}-page", .{options.key}), .value = page_text, .width = 44, .height = 24, .font_size = 12, .on_change = ui.textAction(v, Viewer.tbPageInput), .on_submit = ui.action(v, Viewer.tbPageSubmit) }),
        ui.text(std.fmt.allocPrint(arena, "/ {d}", .{count}) catch "", .{ .size = 12, .color = theme.color_text_secondary }),
        ui.space(.{ .width = 8 }),
        ui.iconButton(.{ .key = frameKey("{s}-zout", .{options.key}), .icon = .minus, .size = 16, .box = 28, .disabled = v.zoom <= min_zoom + 0.001, .tooltip = labels.zoom_out, .on_click = ui.action(v, Viewer.tbZoomOut) }),
        ui.tooltip(.{ .text = if (v.fit == .width) labels.fit_page else labels.fit_width, .child = ui.button(.{ .key = frameKey("{s}-zoom", .{options.key}), .text = zoom_text, .variant = .text, .size = .small, .width = 56, .font_size = 12, .focusable = false, .on_click = ui.action(v, Viewer.tbFit) }) }),
        ui.iconButton(.{ .key = frameKey("{s}-zin", .{options.key}), .icon = .plus, .size = 16, .box = 28, .disabled = v.zoom >= max_zoom - 0.001, .tooltip = labels.zoom_in, .on_click = ui.action(v, Viewer.tbZoomIn) }),
        if (options.annotation_tools) toolSwitch(v, options) else ui.styled(ui.space(.{}), .{ .width = 0 }),
        ui.styled(ui.space(.{}), .{ .grow = 1 }),
        ui.input(.{ .key = frameKey("{s}-search", .{options.key}), .value = v.query.items, .placeholder = labels.search, .width = 160, .height = 24, .font_size = 12, .on_change = ui.textAction(v, Viewer.tbSearch), .on_key = ui.inputKeyAction(v, Viewer.tbSearchKey) }),
        ui.styled(ui.text(match_text, .{ .size = 12, .color = theme.color_text_secondary, .max_lines = 1 }), .{ .min_width = 40 }),
        ui.iconButton(.{ .key = frameKey("{s}-mprev", .{options.key}), .icon = .chevron_up, .size = 16, .box = 28, .disabled = v.matches.items.len == 0, .tooltip = labels.previous_match, .on_click = ui.action(v, Viewer.tbSearchPrev) }),
        ui.iconButton(.{ .key = frameKey("{s}-mnext", .{options.key}), .icon = .chevron_down, .size = 16, .box = 28, .disabled = v.matches.items.len == 0, .tooltip = labels.next_match, .on_click = ui.action(v, Viewer.tbSearchNext) }),
    }) catch &.{};
    return ui.row(.{
        .key = frameKey("{s}-toolbar", .{options.key}),
        .width_percent = 100,
        .height = 40,
        .gap = 4,
        .align_items = .center,
        .padding = .{ .left = 8, .right = 8 },
        .background = theme.color_bg_container,
        .children = children,
    });
}

fn annotationLayer(v: *Viewer, a: *const annots.Annotation, theme: tokens.Theme, options: ViewOptions, out: *std.ArrayListUnmanaged(ui.Node)) !void {
    const arena = ui.frameAllocator();
    const z = v.zoom;
    switch (a.kind) {
        .highlight => for (a.rects) |r| try out.append(arena, box(r, z, a.color, 1)),
        .underline => for (a.rects) |r| {
            const t = @max(1.0 / z, r.height * 0.07);
            try out.append(arena, box(.{ .x = r.x, .y = r.bottom() - t * 1.5, .width = r.width, .height = t }, z, a.color, 0));
        },
        .strikeout => for (a.rects) |r| {
            const t = @max(1.0 / z, r.height * 0.07);
            try out.append(arena, box(.{ .x = r.x, .y = r.y + r.height * 0.52 - t / 2, .width = r.width, .height = t }, z, a.color, 0));
        },
        .squiggly => for (a.rects) |r| try out.append(arena, squiggle(arena, r, z, a.color)),
        .ink => {
            const size = v.pageSizePx(a.page);
            try out.append(arena, inkNode(arena, a.strokes, a.color, a.width, z, size, null));
        },
        .note => if (a.rects.len > 0) {
            const r = a.rects[0];
            try out.append(arena, ui.styled(ui.row(.{
                .justify = .center,
                .align_items = .center,
                .children = &.{ui.icon(.{ .name = .file_text, .size = @max(10, r.width * z * 0.7), .color = 0xd9000000 })},
            }), .{ .absolute = true, .left = r.x * z, .top = r.y * z, .width = r.width * z, .height = r.height * z, .background = a.color | 0xff000000, .radius = 3 * z, .shadow_color = 0x33000000, .shadow_blur = 3, .shadow_offset_y = 1 }));
        },
    }
    // keyed box over the annotation (popovers place themselves with it);
    // outlined while selected
    const b = a.bounds();
    const selected = v.selected_annotation != null and v.selected_annotation.? == a.id;
    var key_buf: [96]u8 = undefined;
    try out.append(arena, ui.styled(ui.column(.{ .key = arena.dupe(u8, Viewer.annotationKey(&key_buf, options.key, a.id)) catch "" }), .{
        .absolute = true,
        .left = b.x * z - 2,
        .top = b.y * z - 2,
        .width = b.width * z + 4,
        .height = b.height * z + 4,
        .border_color = if (selected) theme.color_primary else null,
        .border_width = if (selected) 1.5 else 0,
        .radius = 3,
    }));
}

fn box(r: text_mod.Rect, z: f32, color: u32, radius: f32) ui.Node {
    return ui.styled(ui.column(.{}), .{ .absolute = true, .left = r.x * z, .top = r.y * z, .width = r.width * z, .height = r.height * z, .background = color, .radius = radius });
}

fn squiggle(arena: std.mem.Allocator, r: text_mod.Rect, z: f32, color: u32) ui.Node {
    const w = r.width * z;
    const amp = @max(1.0, r.height * z * 0.05);
    const step = @max(2.0, amp * 2);
    const n: usize = @intFromFloat(@max(2, @ceil(w / step) + 1));
    const pts = arena.alloc(vector.Point, n) catch return ui.column(.{});
    for (pts, 0..) |*p, i| {
        const x = @min(w, @as(f32, @floatFromInt(i)) * step);
        p.* = .{ x, if (i % 2 == 0) 0 else amp * 2 };
    }
    const cmds = arena.alloc(vector.Command, 1) catch return ui.column(.{});
    cmds[0] = .{ .path = .{ .points = pts, .color = color | 0xff000000, .stroke_width = @max(1, amp * 0.8) } };
    const d = arena.create(vector.Drawing) catch return ui.column(.{});
    d.* = .{ .width = w, .ascent = 0, .descent = amp * 2, .commands = cmds };
    return ui.styled(ui.vector(.{ .drawing = d }), .{ .absolute = true, .left = r.x * z, .top = r.bottom() * z - amp * 2 });
}

fn inkNode(arena: std.mem.Allocator, strokes: []const []const annots.Point, color: u32, width: f32, z: f32, size: [2]f32, key: ?[]const u8) ui.Node {
    const cmds = arena.alloc(vector.Command, strokes.len) catch return ui.column(.{});
    for (strokes, cmds) |stroke, *cmd| {
        const pts = arena.alloc(vector.Point, @max(stroke.len, 2)) catch return ui.column(.{});
        for (stroke, 0..) |p, i| pts[i] = .{ p[0] * z, p[1] * z };
        // a single tap draws a dot
        if (stroke.len == 1) pts[1] = .{ stroke[0][0] * z + 0.01, stroke[0][1] * z };
        cmd.* = .{ .path = .{ .points = pts, .color = color, .stroke_width = @max(1, width * z) } };
    }
    const d = arena.create(vector.Drawing) catch return ui.column(.{});
    d.* = .{ .width = size[0], .ascent = 0, .descent = size[1], .commands = cmds };
    return ui.styled(ui.vector(.{ .key = key, .drawing = d }), .{ .absolute = true, .left = 0, .top = 0 });
}

// selection menu actions
fn menuCopy(v: *Viewer) void {
    _ = v.copySelection();
    v.clearSelection();
}
fn menuHighlight(v: *Viewer) void {
    _ = v.annotateSelection(.highlight, null);
}
fn menuUnderline(v: *Viewer) void {
    _ = v.annotateSelection(.underline, null);
}
fn menuStrike(v: *Viewer) void {
    _ = v.annotateSelection(.strikeout, null);
}

/// Floating menu above the start of the text selection (EmbedPDF's
/// selection menu): copy and text markup.
fn selectionMenu(v: *Viewer, out: *std.ArrayListUnmanaged(ui.Node), theme: tokens.Theme, options: ViewOptions) !void {
    const doc = v.doc orelse return;
    const sel = v.selection() orelse return;
    const t = doc.pageText(sel.start.page) orelse return;
    if (t.count() == 0) return;
    const to = if (sel.end.page == sel.start.page) sel.end.index else t.count() - 1;
    const arena = ui.frameAllocator();
    const rects = try t.rects(arena, sel.start.index, @min(to, t.count() - 1));
    if (rects.len == 0) return;
    const origin = v.pageOrigin(sel.start.page);
    const first = rects[0];
    const labels = options.labels;
    const menu_w: f32 = 268;
    const menu_h: f32 = 36;
    var left = origin[0] + first.x * v.zoom;
    var top = origin[1] + first.y * v.zoom - menu_h - 8;
    if (top < 4) top = origin[1] + (first.bottom()) * v.zoom + 8;
    if (top < 4 or top > v.view_h - menu_h) return; // selection start scrolled away
    left = std.math.clamp(left, 8, @max(8, v.view_w - menu_w - 8));
    const item = struct {
        fn make(key: []const u8, label: []const u8, act: actions.Action) ui.Node {
            return ui.button(.{ .key = key, .text = label, .variant = .text, .size = .small, .font_size = 13, .focusable = false, .on_click = act });
        }
    }.make;
    const children = try arena.dupe(ui.Node, &.{
        item(frameKey("{s}-sel-copy", .{options.key}), labels.copy, ui.action(v, menuCopy)),
        item(frameKey("{s}-sel-hl", .{options.key}), labels.highlight, ui.action(v, menuHighlight)),
        item(frameKey("{s}-sel-ul", .{options.key}), labels.underline, ui.action(v, menuUnderline)),
        item(frameKey("{s}-sel-st", .{options.key}), labels.strikeout, ui.action(v, menuStrike)),
    });
    try out.append(arena, ui.styled(ui.row(.{
        .key = frameKey("{s}-sel-menu", .{options.key}),
        .gap = 4,
        .align_items = .center,
        .padding = ui.inset(4),
        .block_pointer = true,
        .children = children,
    }), .{
        .absolute = true,
        .left = left,
        .top = top,
        .height = menu_h,
        .background = theme.color_bg_elevated,
        .radius = 8,
        .shadow_color = 0x26000000,
        .shadow_blur = 12,
        .shadow_offset_y = 3,
        .motion = .zoom_big_fast,
    }));
}

fn tbTool(v: *Viewer, index: usize) void {
    const tools = [_]Tool{ .select, .highlight, .underline, .ink, .note };
    v.setTool(tools[@min(index, tools.len - 1)]);
    v.changed();
}
fn tbUndo(v: *Viewer) void {
    _ = v.undo();
}
fn tbRedo(v: *Viewer) void {
    _ = v.redo();
}

fn toolSwitch(v: *Viewer, options: ViewOptions) ui.Node {
    const labels = options.labels;
    const selected: usize = switch (v.tool) {
        .select => 0,
        .highlight => 1,
        .underline, .strikeout, .squiggly => 2,
        .ink => 3,
        .note => 4,
    };
    const opts = ui.frameAllocator().dupe(ui.SegmentedOption, &.{
        .{ .label = labels.select },
        .{ .label = labels.highlight },
        .{ .label = labels.underline },
        .{ .label = labels.pen, .icon = .pencil },
        .{ .label = labels.note, .icon = .file_text },
    }) catch &.{};
    const children = ui.frameAllocator().dupe(ui.Node, &.{
        ui.styled(ui.space(.{}), .{ .width = 8 }),
        ui.segmented(.{ .key = frameKey("{s}-tools", .{options.key}), .options = opts, .selected = selected, .height = 24, .font_size = 12, .on_change = ui.indexAction(v, tbTool) }),
        ui.iconButton(.{ .key = frameKey("{s}-undo", .{options.key}), .icon = .undo, .size = 16, .box = 28, .disabled = !v.annotations.canUndo(), .on_click = ui.action(v, tbUndo) }),
        ui.iconButton(.{ .key = frameKey("{s}-redo", .{options.key}), .icon = .redo, .size = 16, .box = 28, .disabled = !v.annotations.canRedo(), .on_click = ui.action(v, tbRedo) }),
    }) catch &.{};
    return ui.row(.{ .gap = 4, .align_items = .center, .children = children });
}
