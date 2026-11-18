//! `Inner`: the viewer state shared by the reader, its toolbar and its overlays.

use super::*;

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
    pub(super) fn new() -> Inner {
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
