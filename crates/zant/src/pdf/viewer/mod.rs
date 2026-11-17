//! The PDF viewer UI (`zant.pdf.view`).
//!
//! Continuous vertical scrolling, zoom (fit width / fit page / manual,
//! ⌘/Ctrl + wheel or pinch anchored at the pointer), text selection (drag,
//! double click = word, triple click = line, Shift extends) with copy, search
//! with highlighted matches, links, page navigation, annotations (highlight /
//! underline / strikeout / squiggly / ink / note, undo / redo, annotated
//! export) and a toolbar.
//!
//! Modelled on EmbedPDF's plugins (viewport, scroll vertical strategy, zoom,
//! render + tiling, selection, search, annotation, history), drawn natively
//! with Zant nodes. The application owns the [`Viewer`] (like
//! `markdown::Cache`) and puts it in `render`:
//!
//! ```ignore
//! let viewer = zant::pdf::Viewer::new();
//! viewer.open("manual.pdf")?;
//! // in render:
//! zant::pdf::view(&viewer, zant::pdf::ViewOptions::default())
//! ```
//!
//! Ownership: [`Viewer`] is a cheap handle over `Rc<RefCell<Inner>>`, because
//! every toolbar button and pointer-area callback is a `'static` closure
//! (`ui::Action`) that must mutate the viewer. Methods take `&self` and borrow
//! the inner state (`viewer.open(..)`, `viewer.page_count()`).
//!
//! The submodules split the viewer by concern: `layout` (page tops, scroll,
//! navigation, toolbar), `zoom`, `select` (pointer input, text selection,
//! selection menu), `search`, `annotate` (annotation API, layers, tool switch)
//! and `cache` (page layers from the render worker's tile cache); this file
//! holds the shared state, the lifecycle and the keyboard shortcuts.
//!
//! Callbacks: `on_change` / `on_annotation` run inside the event that caused
//! the change ([`Viewer::flush_callbacks`]), so the rebuild that follows
//! already sees what they did to the app state. A change made through the API
//! outside an event (no `ActionCx` at hand) is reported by the next build
//! (see `cache.rs`). The annotation tool switch is a `ui::segmented` control.
//! `selected_text` / `search_results` return owned data.

pub mod annotate;
pub mod cache;
pub mod labels;
pub mod layout;
pub mod search;
pub mod select;
pub mod zoom;

pub use cache::view;
pub use labels::Labels;

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

use crate::Str;
use crate::core::events::{self, Key, KeyEvent};
use crate::core::keys;
use crate::pdf::annotations::{self, Point, Store};
use crate::pdf::document::{Document, Link, OpenError};
use crate::pdf::renderer::Renderer;
use crate::ui::actions::{Action, TextAction};

/// Zoom steps of the zoom buttons and keyboard shortcuts (EmbedPDF presets).
pub const ZOOM_STEPS: [f32; 17] = [
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0,
];
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 5.0;

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
    fn markup_kind(self) -> Option<annotations::Kind> {
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

/// The state of one viewer.
///
/// Split from the [`Viewer`] handle so callbacks can borrow it mutably; the
/// submodules of this directory implement its behaviour (zoom, layout,
/// selection, search, annotations).
pub(crate) struct Inner {
    pub doc: Option<Arc<Document>>,
    pub renderer: Option<Renderer>,
    pub open_error: Option<OpenErrorKind>,

    pub zoom: f32,
    pub fit: Fit,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub view_w: f32,
    pub view_h: f32,
    /// Page tops in content px at `layout_zoom`.
    pub tops: Vec<f32>,
    pub layout_zoom: f32,
    pub content_w: f32,
    pub content_h: f32,
    pub max_page_w: f32,
    pub current_page: usize,

    /// Selection (anchor = where the drag started).
    pub anchor: Option<Pos>,
    pub focus: Option<Pos>,
    pub drag: Option<DragMode>,
    pub drag_moved: bool,
    pub down_x: f32,
    pub down_y: f32,
    /// Word / line under the pointer when a multi-click drag started.
    pub anchor_slice: Option<[Pos; 2]>,
    pub pressed_link: Option<Link>,
    pub hover: Hover,

    pub query: String,
    pub matches: Vec<Match>,
    pub active_match: Option<usize>,
    /// Next page to search (incremental search runs a few pages per build).
    pub search_page: usize,
    pub search_done: bool,

    pub page_input: String,
    pub page_input_page: Option<usize>,
    /// Scroll to this page (and y in points) once the layout is known.
    pub pending_goto: Option<(usize, f32)>,
    #[allow(dead_code)]
    pub bar_drag: Option<BarDrag>,

    /// Annotations (with undo / redo).
    pub annotations: Store,
    pub tool: Tool,
    /// Colour for new annotations of the current tool (None: default).
    pub tool_color: Option<u32>,
    pub selected_annotation: Option<u64>,
    /// Ink stroke being drawn (page points).
    pub ink_points: Vec<Point>,
    pub ink_page: usize,
    /// Path of the open document (annotated export reads it again).
    pub path: String,

    pub options: ViewOptions,
    pub padding: f32,
    pub page_gap: f32,

    /// `options.on_change` is due: the event that set it runs the callback, or
    /// the next build when the change came through the API (see the module
    /// docs).
    pub pending_change: bool,
    /// `options.on_annotation` is due (like `pending_change`).
    pub pending_annotation: bool,
}

impl Inner {
    fn new() -> Inner {
        Inner {
            doc: None,
            renderer: None,
            open_error: None,
            zoom: 1.0,
            fit: Fit::Width,
            scroll_x: 0.0,
            scroll_y: 0.0,
            view_w: 0.0,
            view_h: 0.0,
            tops: Vec::new(),
            layout_zoom: 0.0,
            content_w: 0.0,
            content_h: 0.0,
            max_page_w: 0.0,
            current_page: 0,
            anchor: None,
            focus: None,
            drag: None,
            drag_moved: false,
            down_x: 0.0,
            down_y: 0.0,
            anchor_slice: None,
            pressed_link: None,
            hover: Hover::None,
            query: String::new(),
            matches: Vec::new(),
            active_match: None,
            search_page: 0,
            search_done: true,
            page_input: String::new(),
            page_input_page: None,
            pending_goto: None,
            bar_drag: None,
            annotations: Store::new(),
            tool: Tool::Select,
            tool_color: None,
            selected_annotation: None,
            ink_points: Vec::new(),
            ink_page: 0,
            path: String::new(),
            options: ViewOptions::default(),
            padding: 16.0,
            page_gap: 12.0,
            pending_change: false,
            pending_annotation: false,
        }
    }

    /// Marks `on_change` as due. The callback needs the viewer unborrowed, so
    /// the entry point that caused the mutation runs it right after
    /// ([`Viewer::flush_callbacks`]).
    pub(crate) fn changed(&mut self) {
        self.pending_change = true;
    }

    /// Marks `on_annotation` as due (see [`Inner::changed`]).
    pub(crate) fn annotation_changed(&mut self) {
        self.pending_annotation = true;
    }

    /// Takes the callbacks that are due and clears the flags.
    pub(crate) fn take_callbacks(&mut self) -> (Option<Action>, Option<Action>) {
        let change = if self.pending_change {
            self.options.on_change.clone()
        } else {
            None
        };
        let annotation = if self.pending_annotation {
            self.options.on_annotation.clone()
        } else {
            None
        };
        self.pending_change = false;
        self.pending_annotation = false;
        (change, annotation)
    }

    /// Viewport resize: the rebuild that follows reads the new size.
    pub(crate) fn on_resize(&mut self) {}

    /// Keyboard shortcut of the focused viewport.
    pub(crate) fn on_key(&mut self, k: KeyEvent) -> bool {
        let primary = keys::primary(k.modifiers);
        if primary {
            if events::character_equals(k.key, b'z') {
                let _ = if k.modifiers.shift {
                    self.redo()
                } else {
                    self.undo()
                };
                return true;
            }
            if events::character_equals(k.key, b'y') {
                let _ = self.redo();
                return true;
            }
            if events::character_equals(k.key, b'c') {
                let _ = self.copy_selection();
                return true;
            }
            if events::character_equals(k.key, b'a') {
                self.select_all();
                return true;
            }
            if events::character_equals(k.key, b'=') || events::character_equals(k.key, b'+') {
                self.zoom_in();
                self.changed();
                return true;
            }
            if events::character_equals(k.key, b'-') {
                self.zoom_out();
                self.changed();
                return true;
            }
            if events::character_equals(k.key, b'0') {
                self.set_fit_mode(Fit::Width);
                self.changed();
                return true;
            }
            if events::character_equals(k.key, b'f') && self.options.toolbar {
                crate::ui::focus(&format!("{}-search", self.options.key));
                return true;
            }
            return false;
        }
        match k.key {
            Key::Escape => {
                if self.selected_annotation.is_some() {
                    self.select_annotation(None);
                    return true;
                }
                if self.selection().is_some() {
                    self.clear_selection();
                    return true;
                }
                if self.tool != Tool::Select {
                    self.set_tool(Tool::Select);
                    self.changed();
                    return true;
                }
                return false;
            }
            Key::Backspace | Key::Delete => {
                let Some(id) = self.selected_annotation else {
                    return false;
                };
                self.remove_annotation(id);
                return true;
            }
            Key::Up => self.scroll_by(0.0, -48.0),
            Key::Down => self.scroll_by(0.0, 48.0),
            Key::Left => self.scroll_by(-48.0, 0.0),
            Key::Right => self.scroll_by(48.0, 0.0),
            Key::Home => {
                self.scroll_y = 0.0;
                self.update_current_page();
            }
            Key::End => {
                self.scroll_y = self.max_scroll_y();
                self.update_current_page();
            }
            Key::Character(' ') => {
                self.scroll_by(
                    0.0,
                    if k.modifiers.shift {
                        -self.view_h * 0.9
                    } else {
                        self.view_h * 0.9
                    },
                );
            }
            _ => return false,
        }
        self.changed();
        true
    }
}

/// The PDF viewer. The application owns it and passes it to [`view`]:
///
/// ```ignore
/// let viewer = zant::pdf::Viewer::new();
/// viewer.open("manual.pdf")?;
/// // in render:
/// zant::pdf::view(&viewer, zant::pdf::ViewOptions::default())
/// ```
///
/// Cheap to clone: every clone shares the same state (the callbacks need
/// that). Opening the first document starts the render worker; dropping the
/// last clone stops it and closes the document.
#[derive(Clone)]
pub struct Viewer {
    inner: Rc<RefCell<Inner>>,
}

impl Default for Viewer {
    fn default() -> Viewer {
        Viewer::new()
    }
}

impl Viewer {
    /// An empty viewer.
    pub fn new() -> Viewer {
        Viewer {
            inner: Rc::new(RefCell::new(Inner::new())),
        }
    }

    pub(crate) fn borrow(&self) -> Ref<'_, Inner> {
        self.inner.borrow()
    }

    pub(crate) fn borrow_mut(&self) -> RefMut<'_, Inner> {
        self.inner.borrow_mut()
    }

    /// Opens a PDF (closing the current one). On failure the viewer shows an
    /// error state and the error is returned.
    pub fn open(&self, path: &str) -> Result<(), OpenError> {
        self.open_with_password(path, None)
    }

    pub fn open_with_password(&self, path: &str, password: Option<&str>) -> Result<(), OpenError> {
        self.close();
        let doc = match Document::open(path, password) {
            Ok(doc) => doc,
            Err(err) => {
                let kind = match err {
                    OpenError::FileNotFound => OpenErrorKind::FileNotFound,
                    OpenError::PasswordRequired => OpenErrorKind::PasswordRequired,
                };
                self.borrow_mut().open_error = Some(kind);
                return Err(err);
            }
        };
        let doc = Arc::new(doc);
        let mut inner = self.borrow_mut();
        inner.renderer = Some(Renderer::create(Arc::clone(&doc), crate::ui::post_wake));
        inner.doc = Some(doc);
        inner.path.clear();
        inner.path.push_str(path);
        inner.open_error = None;
        inner.layout_zoom = 0.0;
        Ok(())
    }

    /// Closes the open document, its render worker and the viewer state.
    pub fn close(&self) {
        let mut inner = self.borrow_mut();
        inner.renderer = None;
        inner.doc = None;
        inner.anchor = None;
        inner.focus = None;
        inner.matches.clear();
        inner.active_match = None;
        inner.scroll_x = 0.0;
        inner.scroll_y = 0.0;
        inner.current_page = 0;
        inner.tops.clear();
        inner.open_error = None;
        inner.annotations.load(&[]);
        inner.selected_annotation = None;
        inner.ink_points.clear();
    }

    /// The open document.
    pub fn document(&self) -> Option<Arc<Document>> {
        self.borrow().doc.clone()
    }

    /// The error of the last failed [`Viewer::open`].
    pub fn open_error(&self) -> Option<OpenError> {
        match self.borrow().open_error {
            Some(OpenErrorKind::PasswordRequired) => Some(OpenError::PasswordRequired),
            Some(OpenErrorKind::FileNotFound) => Some(OpenError::FileNotFound),
            None => None,
        }
    }

    /// Runs one viewer pointer event; returns whether the UI must rebuild.
    pub(crate) fn pointer(
        &self,
        cx: &mut crate::ui::actions::ActionCx<'_>,
        info: crate::ui::actions::PointerInfo,
    ) -> bool {
        let (changed, external) = self.borrow_mut().on_pointer(info);
        if let Some((callback, uri)) = external {
            match callback {
                Some(callback) => callback.call(cx, &uri),
                None => {
                    let _ = crate::platform::dialog::open_url(&uri);
                }
            }
        }
        self.flush_callbacks(cx);
        changed
    }

    /// Runs one viewer key event; returns whether it was consumed.
    pub(crate) fn key(&self, cx: &mut crate::ui::actions::ActionCx<'_>, event: KeyEvent) -> bool {
        let consumed = self.borrow_mut().on_key(event);
        self.flush_callbacks(cx);
        consumed
    }

    /// Runs `on_change` / `on_annotation` when the event that just ran changed
    /// the viewer, inside that same event: the rebuild that follows the event
    /// already sees whatever the callbacks did to the app state. The viewer is
    /// not borrowed while they run, so they may call back into it.
    pub(crate) fn flush_callbacks(&self, cx: &mut crate::ui::actions::ActionCx<'_>) {
        let (change, annotation) = self.borrow_mut().take_callbacks();
        if let Some(callback) = change {
            callback.call(cx);
        }
        if let Some(callback) = annotation {
            callback.call(cx);
        }
    }
}
