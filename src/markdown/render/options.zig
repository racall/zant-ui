//! Rendering options (Streamdown's component props).
const std = @import("std");
const actions = @import("../../core/actions.zig");
const theme_mod = @import("../theme.zig");
const safety = @import("../safety.zig");
const hash_mod = @import("../../core/hash.zig");

pub const Caret = enum {
    none,
    /// ▋
    block,
    /// ●
    circle,

    pub fn glyph(self: Caret) []const u8 {
        return switch (self) {
            .none => "",
            .block => "▋",
            .circle => "●",
        };
    }
};

pub const CodeControls = struct { copy: bool = true, download: bool = true };
pub const TableControls = struct { copy: bool = true, download: bool = true };
pub const MermaidControls = struct { copy: bool = true, download: bool = true };

/// Streamdown `controls`.
pub const Controls = struct {
    code: CodeControls = .{},
    table: TableControls = .{},
    mermaid: MermaidControls = .{},

    pub const none: Controls = .{
        .code = .{ .copy = false, .download = false },
        .table = .{ .copy = false, .download = false },
        .mermaid = .{ .copy = false, .download = false },
    };
};

/// Streamdown `translations`.
pub const Labels = struct {
    copy: []const u8 = "Copy",
    copy_markdown: []const u8 = "Copy Markdown",
    copy_csv: []const u8 = "Copy CSV",
    download: []const u8 = "Download",
    footnotes: []const u8 = "Footnotes",
    image: []const u8 = "Image",
    loading_image: []const u8 = "Loading image…",
    open_link: []const u8 = "Open link",
    copy_link: []const u8 = "Copy link",
    cancel: []const u8 = "Cancel",
    link_title: []const u8 = "Open external link?",
    link_message: []const u8 = "You're about to visit an external website.",
    unsupported_diagram: []const u8 = "Diagram type not supported",

    pub const zh: Labels = .{
        .copy = "复制",
        .copy_markdown = "复制 Markdown",
        .copy_csv = "复制 CSV",
        .download = "下载",
        .footnotes = "脚注",
        .image = "图片",
        .loading_image = "图片加载中…",
        .open_link = "打开链接",
        .copy_link = "复制链接",
        .cancel = "取消",
        .link_title = "打开外部链接？",
        .link_message = "即将访问外部网站，请确认链接可信。",
        .unsupported_diagram = "暂不支持此图表类型",
    };
};

pub const Options = struct {
    theme: theme_mod.Theme = theme_mod.light(),
    /// Body font size (px).
    font_size: f32 = 16,
    /// Content column width; display math and diagrams shrink to fit.
    content_width: f32 = 680,
    /// Streamdown `isAnimating`: the response is still streaming. Shows the
    /// caret and disables copy/download on the incomplete tail.
    is_animating: bool = false,
    caret: Caret = .none,
    controls: Controls = .{},
    /// Line numbers in code blocks (Streamdown default: on).
    line_numbers: bool = true,
    /// Code blocks taller than this scroll (0 = no limit). Default 400px.
    code_max_height: f32 = 400,
    /// Link clicks. With Streamdown's link safety, the app shows
    /// `linkSafetyModal` for the URL; null opens links directly.
    on_link: ?actions.TextAction = null,
    policy: safety.Policy = .{},
    /// Directory relative image paths resolve against.
    base_dir: []const u8 = "",
    labels: Labels = .{},
    /// Prefix for node keys (several documents in one window).
    key_prefix: []const u8 = "md",

    pub fn hash(self: *const Options) u64 {
        // field by field: covers every option (policy, all labels) and never
        // reads struct padding
        return hash_mod.hash(0x6f707473, self.*);
    }
};
