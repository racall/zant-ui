//! Markdown document model produced by the parser and consumed by renderers.
//!
//! Every slice is owned by `Document.arena`; the tree is immutable after
//! parsing.
const std = @import("std");

pub const Style = struct {
    bold: bool = false,
    italic: bool = false,
    code: bool = false,
    strike: bool = false,
    underline: bool = false,
    highlight: bool = false,
    math: bool = false,
    link: bool = false,
    image: bool = false,
    superscript: bool = false,
    subscript: bool = false,
    footnote_ref: bool = false,
    /// Renderer-only marker text (list bullets, caret).
    marker: bool = false,

    pub fn eql(a: Style, b: Style) bool {
        return std.meta.eql(a, b);
    }
};

pub const Span = struct {
    text: []const u8,
    style: Style = .{},
    /// Link target / image source / footnote label. Empty when not linked.
    url: []const u8 = "",
};

pub const Heading = struct {
    level: u8,
    spans: []const Span,
    /// GitHub-style slug ("hello-world"), unique within the document.
    id: []const u8 = "",
};

pub const Code = struct {
    lang: []const u8 = "",
    /// Remainder of the info string after the language (e.g. `startLine=10`).
    meta: []const u8 = "",
    text: []const u8,
    /// False while a fence is still open (streaming).
    closed: bool = true,
    fenced: bool = true,
};

pub const ListItem = struct {
    /// Task list state; null for normal items.
    checked: ?bool = null,
    blocks: []const Block,
};

pub const List = struct {
    ordered: bool,
    start: usize = 1,
    /// Tight lists render their paragraphs without extra spacing.
    tight: bool = true,
    items: []const ListItem,
};

pub const Align = enum { none, left, center, right };

pub const Table = struct {
    headers: []const []const Span,
    rows: []const []const []const Span,
    aligns: []const Align,
    /// Plain-text cells (inline markup stripped) for copy/download.
    raw_headers: []const []const u8,
    raw_rows: []const []const []const u8,
};

pub const Image = struct {
    alt: []const u8,
    url: []const u8,
    title: []const u8 = "",
};

pub const CalloutKind = enum {
    note,
    tip,
    important,
    warning,
    caution,

    pub fn label(self: CalloutKind) []const u8 {
        return switch (self) {
            .note => "Note",
            .tip => "Tip",
            .important => "Important",
            .warning => "Warning",
            .caution => "Caution",
        };
    }
};

pub const Callout = struct {
    kind: CalloutKind,
    blocks: []const Block,
};

pub const Math = struct {
    source: []const u8,
    closed: bool = true,
};

pub const Block = union(enum) {
    heading: Heading,
    paragraph: []const Span,
    code: Code,
    quote: []const Block,
    callout: Callout,
    list: List,
    table: Table,
    math: Math,
    image: Image,
    html: []const u8,
    rule,
};

pub const Footnote = struct {
    label: []const u8,
    /// 1-based display number in order of first reference (0 if unreferenced).
    number: usize = 0,
    blocks: []const Block,
};

pub const Document = struct {
    /// Parser output without the owning arena.
    pub const Parts = struct {
        blocks: []const Block,
        footnotes: []const Footnote = &.{},
        front_matter: []const u8 = "",
    };

    arena: std.heap.ArenaAllocator,
    blocks: []const Block,
    footnotes: []const Footnote = &.{},
    /// Raw YAML front matter (without the `---` fences), if present.
    front_matter: []const u8 = "",

    pub fn deinit(self: *Document) void {
        self.arena.deinit();
    }

    /// True when the last block is a code fence or math block that is still
    /// open — i.e. the source is mid-stream (Streamdown's
    /// `useIsCodeFenceIncomplete`).
    pub fn hasIncompleteBlock(self: *const Document) bool {
        if (self.blocks.len == 0) return false;
        return switch (self.blocks[self.blocks.len - 1]) {
            .code => |code| !code.closed,
            .math => |m| !m.closed,
            else => false,
        };
    }
};

/// Concatenates the visible text of spans (inline markup removed).
pub fn plainText(allocator: std.mem.Allocator, spans: []const Span) ![]const u8 {
    var len: usize = 0;
    for (spans) |span| len += span.text.len;
    const out = try allocator.alloc(u8, len);
    var i: usize = 0;
    for (spans) |span| {
        @memcpy(out[i .. i + span.text.len], span.text);
        i += span.text.len;
    }
    return out;
}
