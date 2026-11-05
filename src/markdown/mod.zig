//! Zant Markdown — a native port of Vercel's Streamdown.
//!
//! Pipeline (mirrors `packages/streamdown`):
//!
//!   remend/      heal incomplete Markdown while streaming (`remend`)
//!   blocks.zig   split into top-level blocks, reuse stable ones (`parse-blocks`)
//!   cache.zig    parse each changed block once (block memoisation)
//!   parser/      CommonMark + GFM + remark-math parser → ast.zig
//!   render/      components: code blocks, tables, Mermaid, math, links …
//!   editor.zig   edit / split / preview editor with formatting commands
//!
//! Quick use:
//!
//!     // stateless, whole document every frame
//!     zant.markdown.render(allocator, source)
//!     // Streamdown with block memoisation (keep `cache` in your state)
//!     zant.markdown.streamdown(&state.md_cache, source, .{ .mode = .streaming })
const std = @import("std");
const ui = @import("../ui.zig");

pub const ast = @import("ast.zig");
pub const parser = @import("parser/mod.zig");
pub const remend = @import("remend/mod.zig");
pub const blocks = @import("blocks.zig");
pub const cache = @import("cache.zig");
pub const theme = @import("theme.zig");
pub const highlight = @import("highlight.zig");
pub const mermaid = @import("mermaid.zig");
pub const table_utils = @import("table_utils.zig");
pub const safety = @import("safety.zig");
pub const edit = @import("edit.zig");
pub const stats = @import("stats.zig");
pub const entities = @import("entities.zig");
pub const scan = @import("scan.zig");
/// HTML export (`html.toHtml`).
pub const html = @import("html.zig");
pub const render_options = @import("render/options.zig");
const view = @import("render/view.zig");
const link_modal = @import("render/link_modal.zig");
const editor_mod = @import("editor.zig");

pub const Document = ast.Document;
pub const Block = ast.Block;
pub const Span = ast.Span;
pub const Style = ast.Style;
pub const Cache = cache.Cache;
pub const Mode = cache.Mode;
pub const Theme = theme.Theme;
pub const Options = render_options.Options;
pub const Labels = render_options.Labels;
pub const Controls = render_options.Controls;
pub const Caret = render_options.Caret;
pub const Props = view.Props;
pub const ParseOptions = parser.Options;
pub const LinkModalProps = link_modal.Props;
pub const EditorProps = editor_mod.Props;
pub const EditorMode = editor_mod.Mode;
pub const EditorLabels = editor_mod.Labels;

/// Parses a complete document (remark-math defaults: `$$…$$` math only).
pub fn parse(allocator: std.mem.Allocator, source: []const u8) !Document {
    return parser.parse(allocator, source, .{});
}

pub fn parseWith(allocator: std.mem.Allocator, source: []const u8, options: ParseOptions) !Document {
    return parser.parse(allocator, source, options);
}

/// Renders a parsed document (no caching).
pub fn renderDocument(doc: *const Document, options: Options) ui.Node {
    return view.document(doc, options);
}

/// Parses and renders `source` for this frame.
pub fn render(allocator: std.mem.Allocator, source: []const u8) !ui.Node {
    return renderWith(allocator, source, .{});
}

pub fn renderWith(allocator: std.mem.Allocator, source: []const u8, options: Options) !ui.Node {
    var doc = try parse(allocator, source);
    defer doc.deinit();
    return view.document(&doc, options);
}

/// The Streamdown component (block-memoised; `cache` lives in app state).
pub const streamdown = view.streamdown;
/// Key of the preview block containing a source offset (for `ui.scrollIntoView`).
pub const blockKeyAtOffset = view.blockKeyAtOffset;
/// Key of the n-th top-level heading in the preview (outline jumps).
pub const headingKey = view.headingKey;
/// Link-safety confirmation dialog (Streamdown `linkSafety`).
pub const linkSafetyModal = link_modal.linkSafetyModal;
/// Markdown editor with edit / split / preview modes.
pub const editor = editor_mod.editor;

test {
    _ = ast;
    _ = parser;
    _ = remend;
    _ = blocks;
    _ = cache;
    _ = @import("../core/hash.zig");
    _ = highlight;
    _ = mermaid;
    _ = table_utils;
    _ = safety;
    _ = edit;
    _ = stats;
    _ = entities;
    _ = scan;
    _ = html;
}
