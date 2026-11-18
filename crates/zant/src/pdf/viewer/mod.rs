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

mod inner;
mod model;

use self::inner::*;
pub use self::model::*;

/// Zoom steps of the zoom buttons and keyboard shortcuts (EmbedPDF presets).
pub const ZOOM_STEPS: [f32; 17] = [
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0,
];

pub const MIN_ZOOM: f32 = 0.25;

pub const MAX_ZOOM: f32 = 5.0;

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
