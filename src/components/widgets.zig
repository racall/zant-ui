//! Stateful document builders. Slots belong to the current runtime and are
//! destroyed on unmount, including caches, documents and PDF worker threads.
const std = @import("std");
const ui = @import("../ui.zig");
const md = @import("../markdown/mod.zig");
const pdf = @import("../pdf/viewer.zig");
const actions = @import("../core/actions.zig");

/// Markdown content, typography and Streamdown controls. Complex renderer
/// extensions remain available to Zig callers; scalar fields reflect into TSX.
pub const MarkdownOptions = struct {
    source: []const u8,
    key: ?[]const u8 = null,
    streaming: bool = false,
    parse_incomplete: bool = true,
    single_dollar_math: bool = true,
    latex_delimiters: bool = false,
    font_size: f32 = 16,
    content_width: f32 = 680,
    caret: md.Caret = .none,
    line_numbers: bool = true,
    code_max_height: f32 = 400,
    code_copy: bool = true,
    code_download: bool = true,
    table_copy: bool = true,
    table_download: bool = true,
    mermaid_copy: bool = true,
    mermaid_download: bool = true,
    on_link: ?actions.TextAction = null,
    base_dir: []const u8 = "",
    selectable: bool = true,
    theme: ?md.Theme = null,
    policy: md.safety.Policy = .{},
    labels: md.Labels = .{},
    image_loader: ?md.ImageLoader = null,
    block_renderers: []const md.BlockRenderer = &.{},
};

/// Slot payload (public for diagnostics); applications normally use markdown.
pub const MarkdownState = struct {
    cache: md.Cache = md.Cache.init(std.heap.c_allocator),
    prefix: [32]u8 = undefined,
    prefix_len: usize = 0,

    /// Releases all parsed blocks and their memo arenas.
    pub fn deinit(self: *MarkdownState) void {
        self.cache.deinit();
    }
};

/// Renders Markdown with a persistent block cache. Give every simultaneous
/// instance a distinct stable id; stop calling it to release the cache.
pub fn markdown(id: u64, options: MarkdownOptions) ui.Node {
    const slot = ui.localSlot(MarkdownState, id);
    const state = slot.value;
    if (slot.fresh) {
        state.* = .{ .cache = md.Cache.init(std.heap.c_allocator) };
        state.prefix_len = (std.fmt.bufPrint(&state.prefix, "md-{x}", .{id}) catch unreachable).len;
    }
    const dark = @import("../theme/tokens.zig").isDark(ui.currentTheme());
    const node = md.streamdown(&state.cache, options.source, .{
        .mode = if (options.streaming) .streaming else .static,
        .parse_incomplete = options.parse_incomplete,
        .parse = .{
            .single_dollar_math = options.single_dollar_math,
            .latex_delimiters = options.latex_delimiters,
        },
        .render = .{
            .theme = options.theme orelse if (dark) md.theme.dark() else md.theme.light(),
            .key_prefix = state.prefix[0..state.prefix_len],
            .font_size = options.font_size,
            .content_width = options.content_width,
            .is_animating = options.streaming,
            .caret = options.caret,
            .line_numbers = options.line_numbers,
            .code_max_height = options.code_max_height,
            .controls = .{
                .code = .{ .copy = options.code_copy, .download = options.code_download },
                .table = .{ .copy = options.table_copy, .download = options.table_download },
                .mermaid = .{ .copy = options.mermaid_copy, .download = options.mermaid_download },
            },
            .on_link = options.on_link,
            .base_dir = options.base_dir,
            .selectable = options.selectable,
            .policy = options.policy,
            .labels = options.labels,
            .image_loader = options.image_loader,
            .block_renderers = options.block_renderers,
        },
    });
    // Streamdown's root is 100% wide. Let the parent stretch this facade so
    // ui.styled(..., .{ .width = ... }) can override its size normally.
    const root = ui.frameAllocator().create(ui.Element) catch @panic("Zant UI: out of memory");
    root.* = node.element.*;
    root.layout.width_percent = null;
    root.key = options.key orelse root.key;
    return .{ .element = root };
}

/// Full reader options, with document source and flex sizing added.
pub const PdfViewerOptions = struct {
    src: []const u8,
    password: ?[]const u8 = null,
    key: ?[]const u8 = null,
    toolbar: bool = true,
    page_gap: f32 = 12,
    padding: f32 = 16,
    background: ?u32 = null,
    labels: pdf.Labels = .{},
    on_link: ?actions.TextAction = null,
    on_change: ?actions.Action = null,
    annotation_tools: bool = false,
    selection_menu: bool = true,
    on_annotation: ?actions.Action = null,
    width: ?f32 = null,
    height: ?f32 = null,
    /// Defaults to 1, or 0 with an explicit height.
    grow: ?f32 = null,
    empty_text: []const u8 = "Choose a PDF to read",
    loading_text: []const u8 = "Opening PDF…",
    error_text: []const u8 = "This PDF can't be opened",
};

/// Slot payload, including owned input strings (TSX props may be frame memory).
pub const PdfViewerState = struct {
    viewer: pdf.Viewer = pdf.Viewer.init(std.heap.c_allocator),
    src: []const u8 = "",
    password: ?[]const u8 = null,
    prefix: [32]u8 = undefined,
    prefix_len: usize = 0,
    pending: bool = false,
    /// The last open failed (missing file, wrong password, not a PDF).
    failed: bool = false,

    /// Closes the document, joins the renderer and frees owned strings.
    pub fn deinit(self: *PdfViewerState) void {
        self.viewer.deinit();
        self.viewer.allocator.free(self.src);
        if (self.password) |password| self.viewer.allocator.free(password);
    }

    fn update(self: *PdfViewerState, options: PdfViewerOptions, fresh: bool) void {
        if (!fresh and std.mem.eql(u8, self.src, options.src) and samePassword(self.password, options.password)) return;
        const a = self.viewer.allocator;
        const src = a.dupe(u8, options.src) catch @panic("Zant UI: out of memory");
        const password = if (options.password) |p| a.dupe(u8, p) catch @panic("Zant UI: out of memory") else null;
        a.free(self.src);
        if (self.password) |p| a.free(p);
        self.src = src;
        self.password = password;
        self.pending = true;
        // File I/O happens after reconciliation, once per changed source or
        // password. Failed paths are remembered too (no retry every frame).
        ui.effect(ui.action(self, open));
    }

    fn open(self: *PdfViewerState) void {
        self.viewer.close();
        self.failed = false;
        if (self.src.len > 0) self.viewer.openWithPassword(self.src, self.password) catch {
            self.failed = true;
        };
        self.pending = false;
        ui.requestRebuild();
    }
};

fn samePassword(a: ?[]const u8, b: ?[]const u8) bool {
    if (a == null or b == null) return a == null and b == null;
    return std.mem.eql(u8, a.?, b.?);
}

/// Renders a complete PDF reader, reopening only when src/password change.
/// Empty, loading and failed sources keep the same size and themed surface.
pub fn pdfViewer(id: u64, options: PdfViewerOptions) ui.Node {
    const slot = ui.localSlot(PdfViewerState, id);
    const state = slot.value;
    if (slot.fresh) {
        state.* = .{ .viewer = pdf.Viewer.init(std.heap.c_allocator) };
        state.prefix_len = (std.fmt.bufPrint(&state.prefix, "pdf-{x}", .{id}) catch unreachable).len;
    }
    state.update(options, slot.fresh);
    const t = ui.currentTheme();
    const message: ?[]const u8 = if (options.src.len == 0)
        options.empty_text
    else if (state.pending)
        options.loading_text
    else if (state.failed)
        options.error_text
    else
        null;
    const content = if (message) |text|
        ui.column(.{
            .grow = 1,
            .justify = .center,
            .align_items = .center,
            .gap = 8,
            .padding = ui.inset(t.padding),
            .background = options.background orelse t.color_bg_layout,
            .children = &.{
                ui.icon(.{ .name = .file_text, .size = 32, .color = if (state.failed) t.color_error else t.color_text_disabled }),
                ui.text(text, .{ .color = if (state.failed) t.color_error_text else t.color_text_secondary, .size = t.font_size }),
            },
        })
    else
        pdf.view(&state.viewer, .{
            .key = state.prefix[0..state.prefix_len],
            .toolbar = options.toolbar,
            .page_gap = options.page_gap,
            .padding = options.padding,
            .background = options.background,
            .labels = options.labels,
            .on_link = options.on_link,
            .on_change = options.on_change,
            .annotation_tools = options.annotation_tools,
            .selection_menu = options.selection_menu,
            .on_annotation = options.on_annotation,
        });
    return ui.column(.{
        .key = options.key orelse state.prefix[0..state.prefix_len],
        .width = options.width,
        .height = options.height,
        .grow = options.grow orelse if (options.height != null) 0 else 1,
        .shrink = 0,
        .align_items = .stretch,
        .clip = true,
        .children = &.{content},
    });
}

test "password identity includes presence and content" {
    try std.testing.expect(samePassword(null, null));
    try std.testing.expect(!samePassword(null, ""));
    try std.testing.expect(samePassword("secret", "secret"));
    try std.testing.expect(!samePassword("secret", "other"));
}
