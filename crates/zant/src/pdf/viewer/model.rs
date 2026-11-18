//! The viewer's value types: positions, ranges, matches, fit modes, options, tools, errors and drag / hover state.

use super::*;

/// A page and a glyph index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pos {
    pub page: usize,
    pub index: usize,
}

impl Pos {
    /// Orders by page first, then glyph index.
    pub fn less(a: Pos, b: Pos) -> bool {
        a.page < b.page || (a.page == b.page && a.index < b.index)
    }
}

/// Selected glyphs `start..end` (inclusive, ordered).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: Pos,
    pub end: Pos,
}

/// One search hit on a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub page: usize,
    pub from: usize,
    pub to: usize,
}

/// Zoom mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    /// Page width fills the viewport.
    #[default]
    Width,
    /// The whole page is visible.
    Page,
    /// Manual zoom (`set_zoom`, zoom buttons, ⌘/Ctrl + wheel).
    None,
}

/// `zant.pdf.view` options.
#[derive(Clone)]
pub struct ViewOptions {
    /// Prefix of node keys (several viewers in one window need different keys).
    pub key: Str,
    /// Show the toolbar (page navigation, zoom, search).
    pub toolbar: bool,
    /// Space between pages and around them (px).
    pub page_gap: f32,
    pub padding: f32,
    /// Viewport background (default: theme `color_bg_layout`).
    pub background: Option<u32>,
    pub labels: Labels,
    /// External link clicks; `None` opens the URL with the system browser.
    pub on_link: Option<TextAction>,
    /// Page, zoom, selection or search results changed.
    pub on_change: Option<Action>,
    /// Annotation tools in the toolbar (select / highlight / underline / pen /
    /// note).
    pub annotation_tools: bool,
    /// A small menu over selected text: copy, highlight, underline,
    /// strikethrough.
    pub selection_menu: bool,
    /// The annotation list or the selected annotation changed.
    pub on_annotation: Option<Action>,
}

impl Default for ViewOptions {
    fn default() -> ViewOptions {
        ViewOptions {
            key: Str::Static("pdf"),
            toolbar: true,
            page_gap: 12.0,
            padding: 16.0,
            background: None,
            labels: Labels::default(),
            on_link: None,
            on_change: None,
            annotation_tools: false,
            selection_menu: true,
            on_annotation: None,
        }
    }
}

/// What a drag on the page does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Select text (and click annotations to select them).
    #[default]
    Select,
    /// Drag over text to highlight / underline / strike it out.
    Highlight,
    Underline,
    Strikeout,
    Squiggly,
    /// Draw freehand strokes.
    Ink,
    /// Click to place a sticky note.
    Note,
}

impl Tool {
    pub(super) fn markup_kind(self) -> Option<annotations::Kind> {
        match self {
            Tool::Highlight => Some(annotations::Kind::Highlight),
            Tool::Underline => Some(annotations::Kind::Underline),
            Tool::Strikeout => Some(annotations::Kind::Strikeout),
            Tool::Squiggly => Some(annotations::Kind::Squiggly),
            _ => None,
        }
    }
}

/// Why the last [`Viewer::open`] failed (the variant of `OpenError`; the
/// document's error type is not `Clone`, so the viewer keeps the kind and
/// rebuilds the error for [`Viewer::open_error`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenErrorKind {
    FileNotFound,
    PasswordRequired,
}

/// Errors of [`Viewer::save_annotated`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SaveError {
    #[error("no open PDF document")]
    NoDocument,
    #[error("writing the annotated PDF failed")]
    SaveFailed,
}

/// How a selection drag extends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DragMode {
    #[default]
    Char,
    Word,
    Line,
}

/// Pointer hover state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hover {
    #[default]
    None,
    Text,
    Link,
}

/// An in-progress scrollbar drag.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BarDrag {
    pub start: f32,
    pub scroll: f32,
    /// Which bar is dragged (the drag handler gets it as a parameter).
    #[allow(dead_code)]
    pub horizontal: bool,
}

/// The page point under a viewport point kept fixed across a zoom change.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Anchor {
    pub page: usize,
    pub px: f32,
    pub py: f32,
    pub vx: f32,
    pub vy: f32,
}

/// Page and page point under a viewport point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PagePoint {
    pub page: usize,
    pub x: f32,
    pub y: f32,
}
