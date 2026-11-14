//! Port of `src/markdown/ast.zig` — WP-3.2.
//!
//! Markdown document model produced by the parser and consumed by renderers.
//! The Zig version keeps every slice in the document's arena; the Rust port
//! owns `String` / `Vec` instead, so the tree is immutable after parsing and
//! needs no explicit `deinit`.

/// Inline styling of a [`Span`] (Zig `ast.Style`).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub underline: bool,
    pub highlight: bool,
    pub math: bool,
    pub link: bool,
    pub image: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub footnote_ref: bool,
    /// Renderer-only marker text (list bullets, caret).
    pub marker: bool,
}

/// A styled run of inline text (Zig `ast.Span`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Span {
    pub text: String,
    pub style: Style,
    /// Link target / image source / footnote label. Empty when not linked.
    pub url: String,
}

/// An ATX / setext heading (Zig `ast.Heading`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Heading {
    pub level: u8,
    pub spans: Vec<Span>,
    /// GitHub-style slug ("hello-world"), unique within the document.
    pub id: String,
}

/// A fenced or indented code block (Zig `ast.Code`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Code {
    pub lang: String,
    /// Remainder of the info string after the language (e.g. `startLine=10`).
    pub meta: String,
    pub text: String,
    /// False while a fence is still open (streaming).
    pub closed: bool,
    pub fenced: bool,
}

impl Default for Code {
    fn default() -> Self {
        Code {
            lang: String::new(),
            meta: String::new(),
            text: String::new(),
            closed: true,
            fenced: true,
        }
    }
}

/// One list item (Zig `ast.ListItem`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct ListItem {
    /// Task list state; `None` for normal items.
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A list block (Zig `ast.List`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct List {
    pub ordered: bool,
    pub start: usize,
    /// Tight lists render their paragraphs without extra spacing.
    pub tight: bool,
    pub items: Vec<ListItem>,
}

impl Default for List {
    fn default() -> Self {
        List {
            ordered: false,
            start: 1,
            tight: true,
            items: Vec::new(),
        }
    }
}

/// Table column alignment (Zig `ast.Align`).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Align {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// A GFM table (Zig `ast.Table`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Table {
    pub headers: Vec<Vec<Span>>,
    pub rows: Vec<Vec<Vec<Span>>>,
    pub aligns: Vec<Align>,
    /// Plain-text cells (inline markup stripped) for copy/download.
    pub raw_headers: Vec<String>,
    pub raw_rows: Vec<Vec<String>>,
}

/// An image block / `![]()` span payload (Zig `ast.Image`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Image {
    pub alt: String,
    pub url: String,
    pub title: String,
}

/// GFM alert kind (Zig `ast.CalloutKind`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    pub fn label(self) -> &'static str {
        match self {
            CalloutKind::Note => "Note",
            CalloutKind::Tip => "Tip",
            CalloutKind::Important => "Important",
            CalloutKind::Warning => "Warning",
            CalloutKind::Caution => "Caution",
        }
    }
}

/// A block quote that is a GFM alert (Zig `ast.Callout`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Callout {
    pub kind: CalloutKind,
    pub blocks: Vec<Block>,
}

/// A display-math block (Zig `ast.Math`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Math {
    pub source: String,
    pub closed: bool,
}

impl Default for Math {
    fn default() -> Self {
        Math {
            source: String::new(),
            closed: true,
        }
    }
}

/// A top-level block (Zig `ast.Block`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Block {
    Heading(Heading),
    Paragraph(Vec<Span>),
    Code(Code),
    Quote(Vec<Block>),
    Callout(Callout),
    List(List),
    Table(Table),
    Math(Math),
    Image(Image),
    Html(String),
    Rule,
}

/// A footnote definition with its display number (Zig `ast.Footnote`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Footnote {
    pub label: String,
    /// 1-based display number in order of first reference (0 if unreferenced).
    pub number: usize,
    pub blocks: Vec<Block>,
}

/// Parser output without the owning arena (Zig `ast.Document.Parts`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Parts {
    pub blocks: Vec<Block>,
    pub footnotes: Vec<Footnote>,
    pub front_matter: String,
}

/// A parsed markdown document (Zig `ast.Document`).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub footnotes: Vec<Footnote>,
    /// Raw YAML front matter (without the `---` fences), if present.
    pub front_matter: String,
}

impl From<Parts> for Document {
    fn from(parts: Parts) -> Document {
        Document {
            blocks: parts.blocks,
            footnotes: parts.footnotes,
            front_matter: parts.front_matter,
        }
    }
}

impl Document {
    /// True when the last block is a code fence or math block that is still
    /// open — i.e. the source is mid-stream (Streamdown's
    /// `useIsCodeFenceIncomplete`).
    pub fn has_incomplete_block(&self) -> bool {
        let Some(last) = self.blocks.last() else {
            return false;
        };
        match last {
            Block::Code(code) => !code.closed,
            Block::Math(math) => !math.closed,
            _ => false,
        }
    }
}

/// Concatenates the visible text of spans (inline markup removed).
pub fn plain_text(spans: &[Span]) -> String {
    let len = spans.iter().map(|span| span.text.len()).sum();
    let mut out = String::with_capacity(len);
    for span in spans {
        out.push_str(&span.text);
    }
    out
}
