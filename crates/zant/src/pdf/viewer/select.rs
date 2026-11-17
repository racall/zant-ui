//! Text selection and pointer handling of the PDF viewer: the selection API
//! and the pointer state machine.
//!
//! Glyph geometry and the EmbedPDF hit-testing / word / line algorithms live
//! in `pdf::text`; this module maps viewport points to glyph positions, keeps
//! the anchor / focus range, extends it (char / word / line, Shift click) and
//! extracts the selected text. Pointer events that are not about selection
//! (ink strokes, notes, links) are part of the same state machine and
//! forwarded to `Inner::page_point` / `Inner::annotation_at`.
//!
//! `selection_menu` is the floating copy / markup menu above the selection
//! (EmbedPDF's selection menu), placed in viewport coordinates.

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonSize, ButtonType};
use crate::flex;
use crate::motion::motion::Presence;
use crate::pdf::annotations;
use crate::pdf::document::Link;
use crate::theme::tokens::Theme;
use crate::ui::actions::Action;
use crate::ui::builders::{button, row};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, ButtonOptions};
use crate::ui::{Style, styled};

use super::{DragMode, Fit, Hover, Inner, PagePoint, Pos, Range, Tool, ViewOptions, Viewer};

impl Inner {
    /// The selected glyph range, ordered (`None` when nothing is selected).
    pub(crate) fn selection(&self) -> Option<Range> {
        let a = self.anchor?;
        let f = self.focus?;
        Some(if Pos::less(f, a) {
            Range { start: f, end: a }
        } else {
            Range { start: a, end: f }
        })
    }

    pub(crate) fn set_selection(&mut self, start: Pos, end: Pos) {
        self.anchor = Some(start);
        self.focus = Some(end);
        self.anchor_slice = None;
    }

    pub(crate) fn clear_selection(&mut self) {
        self.anchor = None;
        self.focus = None;
        self.anchor_slice = None;
    }

    /// Selects every page's text.
    pub(crate) fn select_all(&mut self) {
        let Some(doc) = self.doc.clone() else {
            return;
        };
        let n = doc.page_count();
        if n == 0 {
            return;
        }
        let mut last = n;
        while last > 0 {
            last -= 1;
            let Some(text) = doc.page_text(last) else {
                continue;
            };
            if text.count() == 0 {
                continue;
            }
            self.anchor = Some(Pos { page: 0, index: 0 });
            self.focus = Some(Pos {
                page: last,
                index: text.count() - 1,
            });
            self.anchor_slice = None;
            return;
        }
    }

    /// UTF-8 text of the selection (pages separated by a line break).
    pub(crate) fn selected_text(&self) -> String {
        let mut out = String::new();
        let Some(doc) = self.doc.clone() else {
            return out;
        };
        let Some(range) = self.selection() else {
            return out;
        };
        let mut page = range.start.page;
        while page <= range.end.page {
            if let Some(text) = doc.page_text(page)
                && text.count() > 0
            {
                let from = if page == range.start.page {
                    range.start.index
                } else {
                    0
                };
                let to = if page == range.end.page {
                    range.end.index
                } else {
                    text.count() - 1
                };
                if page != range.start.page && !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                text.append_text(&mut out, from, to.min(text.count() - 1));
            }
            page += 1;
        }
        out
    }

    /// Copies the selection to the clipboard; false when nothing is selected.
    pub(crate) fn copy_selection(&self) -> bool {
        let value = self.selected_text();
        if value.is_empty() {
            return false;
        }
        crate::platform::platform::set_clipboard(&value)
    }

    /// Glyph position under a viewport point: exact / tolerant hit on the page
    /// under the point, else the nearest glyph of the nearest page.
    pub(crate) fn pos_at(&self, vx: f32, vy: f32, exact: bool) -> Option<Pos> {
        let doc = self.doc.clone()?;
        if self.tops.is_empty() {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        let px = (vx - origin[0]) / self.zoom;
        let py = (vy - origin[1]) / self.zoom;
        let text = doc.page_text(page)?;
        if let Some(index) = text.glyph_at(px, py, 1.5) {
            return Some(Pos { page, index });
        }
        if exact {
            return None;
        }
        text.nearest_glyph(px, py).map(|index| Pos { page, index })
    }

    pub(crate) fn link_at(&self, vx: f32, vy: f32) -> Option<Link> {
        let doc = self.doc.clone()?;
        if self.tops.is_empty() {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        let px = (vx - origin[0]) / self.zoom;
        let py = (vy - origin[1]) / self.zoom;
        doc.page_links(page)
            .iter()
            .find(|link| link.rect.contains(px, py))
            .cloned()
    }

    fn slice_for(&self, pos: Pos, mode: DragMode) -> [Pos; 2] {
        let Some(text) = self.doc.as_ref().and_then(|doc| doc.page_text(pos.page)) else {
            return [pos, pos];
        };
        let slice = match mode {
            DragMode::Char => return [pos, pos],
            DragMode::Word => text.word_at(pos.index),
            DragMode::Line => text.line_at(pos.index),
        };
        let Some(slice) = slice else {
            return [pos, pos];
        };
        [
            Pos {
                page: pos.page,
                index: slice.from,
            },
            Pos {
                page: pos.page,
                index: slice.to,
            },
        ]
    }

    fn extend_to(&mut self, pos: Pos) {
        let mode = self.drag.unwrap_or(DragMode::Char);
        if mode == DragMode::Char || self.anchor_slice.is_none() {
            self.focus = Some(pos);
            return;
        }
        // word / line granularity: grow from the anchored word or line
        let a = self.anchor_slice.expect("anchor slice");
        let b = self.slice_for(pos, mode);
        if Pos::less(b[0], a[0]) {
            self.anchor = Some(a[1]);
            self.focus = Some(b[0]);
        } else {
            self.anchor = Some(a[0]);
            self.focus = Some(if Pos::less(b[1], a[1]) { a[1] } else { b[1] });
        }
    }

    /// Page and page point under a viewport point (nearest page by y).
    pub(crate) fn page_point(&self, vx: f32, vy: f32) -> Option<PagePoint> {
        if self.tops.is_empty() {
            return None;
        }
        let page = self.page_near_y(vy);
        let origin = self.page_origin(page);
        Some(PagePoint {
            page,
            x: (vx - origin[0]) / self.zoom,
            y: (vy - origin[1]) / self.zoom,
        })
    }

    /// The drag / scroll / hover state machine.
    ///
    /// Returns whether the UI must rebuild, plus (for an activated external
    /// link) the callback and URL — the caller runs it after releasing the
    /// state borrow, because the app's `on_link` action may call back into the
    /// viewer.
    pub(crate) fn on_pointer(
        &mut self,
        info: crate::ui::actions::PointerInfo,
    ) -> (
        bool,
        Option<(Option<crate::ui::actions::TextAction>, String)>,
    ) {
        use crate::ui::actions::PointerPhase as Phase;
        if self.doc.is_none() {
            return (false, None);
        }
        match info.phase {
            Phase::Scroll => {
                if info.modifiers.control || info.modifiers.meta {
                    // ⌘ / Ctrl + wheel and pinch (Control + wheel, 250 per
                    // unit of magnification) zoom around the pointer; one
                    // wheel notch (~60) is about 1.27×
                    let factor = (info.dy.clamp(-120.0, 120.0) * 0.004).exp();
                    self.fit = Fit::None;
                    self.zoom_at(self.zoom * factor, info.x, info.y);
                } else {
                    self.scroll_by(-info.dx, -info.dy);
                }
                self.changed();
                (true, None)
            }
            Phase::Hover => {
                if self.tool == Tool::Ink || self.tool == Tool::Note {
                    return (false, None);
                }
                let next = if self.tool == Tool::Select
                    && (self.annotation_at(info.x, info.y).is_some()
                        || self.link_at(info.x, info.y).is_some())
                {
                    Hover::Link
                } else if self.pos_at(info.x, info.y, true).is_some() {
                    Hover::Text
                } else {
                    Hover::None
                };
                if next == self.hover {
                    return (false, None);
                }
                self.hover = next;
                (true, None)
            }
            Phase::Leave => {
                if self.hover == Hover::None {
                    return (false, None);
                }
                self.hover = Hover::None;
                (true, None)
            }
            Phase::Down => {
                self.down_x = info.x;
                self.down_y = info.y;
                self.drag_moved = false;
                self.pressed_link = None;
                match self.tool {
                    Tool::Ink => {
                        let Some(point) = self.page_point(info.x, info.y) else {
                            return (false, None);
                        };
                        self.ink_page = point.page;
                        self.ink_points.clear();
                        self.ink_points.push([point.x, point.y]);
                        return (true, None);
                    }
                    Tool::Note => {
                        let Some(point) = self.page_point(info.x, info.y) else {
                            return (false, None);
                        };
                        let size: f32 = 18.0;
                        let rect = crate::pdf::text::Rect {
                            x: point.x - size / 2.0,
                            y: point.y - size / 2.0,
                            width: size,
                            height: size,
                        };
                        let annotation = crate::pdf::annotations::Annotation {
                            page: point.page,
                            kind: crate::pdf::annotations::Kind::Note,
                            color: self.tool_color.unwrap_or_else(|| {
                                Viewer::default_color(crate::pdf::annotations::Kind::Note)
                            }),
                            rects: vec![rect],
                            ..Default::default()
                        };
                        let id = self.annotations.add(annotation);
                        self.selected_annotation = Some(id);
                        self.annotation_changed();
                        return (true, None);
                    }
                    Tool::Select => {
                        if !info.modifiers.shift && info.click_count == 1 {
                            if let Some(id) = self.annotation_at(info.x, info.y) {
                                self.clear_selection();
                                self.drag = None;
                                self.select_annotation(Some(id));
                                return (true, None);
                            }
                            self.select_annotation(None);
                        }
                    }
                    _ => {}
                }
                if !info.modifiers.shift
                    && info.click_count == 1
                    && let Some(link) = self.link_at(info.x, info.y)
                {
                    self.pressed_link = Some(link);
                    return (false, None);
                }
                let mode = match info.click_count {
                    1 => DragMode::Char,
                    2 => DragMode::Word,
                    _ => DragMode::Line,
                };
                self.drag = Some(mode);
                let pos = self.pos_at(info.x, info.y, false);
                if info.modifiers.shift && self.anchor.is_some() {
                    if let Some(p) = pos {
                        self.focus = Some(p);
                    }
                    self.drag_moved = true;
                    return (true, None);
                }
                if let Some(p) = pos {
                    if mode == DragMode::Char {
                        self.anchor = Some(p);
                        self.focus = Some(p);
                        self.anchor_slice = None;
                    } else {
                        let slice = self.slice_for(p, mode);
                        self.anchor = Some(slice[0]);
                        self.focus = Some(slice[1]);
                        self.anchor_slice = Some(slice);
                        self.drag_moved = true;
                    }
                } else {
                    self.clear_selection();
                }
                (true, None)
            }
            Phase::Drag => {
                if self.tool == Tool::Ink {
                    if self.ink_points.is_empty() {
                        return (false, None);
                    }
                    let origin = self.page_origin(self.ink_page);
                    let size = self
                        .doc
                        .as_ref()
                        .expect("open document")
                        .page_size(self.ink_page);
                    let x = ((info.x - origin[0]) / self.zoom).clamp(0.0, size.width);
                    let y = ((info.y - origin[1]) / self.zoom).clamp(0.0, size.height);
                    let last = self.ink_points[self.ink_points.len() - 1];
                    // skip sub-pixel moves (keeps strokes light)
                    if (last[0] - x).abs() + (last[1] - y).abs() < 0.75 / self.zoom {
                        return (false, None);
                    }
                    self.ink_points.push([x, y]);
                    return (true, None);
                }
                if self.pressed_link.is_some() {
                    return (false, None);
                }
                if self.drag.is_none() {
                    return (false, None);
                }
                if (info.x - self.down_x).abs() + (info.y - self.down_y).abs() > 3.0 {
                    self.drag_moved = true;
                }
                // auto-scroll when dragging past the viewport edges
                if info.y < 0.0 {
                    self.scroll_by(0.0, info.y * 0.5);
                } else if info.y > info.height {
                    self.scroll_by(0.0, (info.y - info.height) * 0.5);
                }
                if let Some(p) = self.pos_at(
                    info.x.clamp(0.0, info.width),
                    info.y.clamp(0.0, info.height),
                    false,
                ) {
                    self.extend_to(p);
                }
                (true, None)
            }
            Phase::Up => {
                if let Some(link) = self.pressed_link.take() {
                    let external =
                        if (info.x - self.down_x).abs() + (info.y - self.down_y).abs() <= 4.0 {
                            self.activate_link(link)
                        } else {
                            None
                        };
                    return (true, external);
                }
                if self.tool == Tool::Ink {
                    self.finish_ink();
                    return (true, None);
                }
                let was_dragging = self.drag.is_some();
                self.drag = None;
                // a click without a drag clears the selection (and selects nothing)
                if was_dragging && !self.drag_moved {
                    self.clear_selection();
                }
                // markup tools annotate what was dragged over
                if let Some(kind) = self.tool.markup_kind()
                    && self.selection().is_some()
                    && self.drag_moved
                {
                    let _ = self.annotate_selection(kind, self.tool_color);
                }
                self.changed();
                (true, None)
            }
        }
    }

    /// Activates a link: internal links scroll, external ones go to the app
    /// (or the browser).
    fn activate_link(
        &mut self,
        link: Link,
    ) -> Option<(Option<crate::ui::actions::TextAction>, String)> {
        if let Some(page) = link.dest_page {
            self.go_to_page(page, link.dest_y.unwrap_or(0.0));
            self.changed();
            return None;
        }
        if link.uri.is_empty() {
            return None;
        }
        Some((self.options.on_link.clone(), link.uri.clone()))
    }
}

impl Viewer {
    /// The selected glyph range, ordered (null when nothing is selected).
    pub fn selection(&self) -> Option<Range> {
        self.borrow().selection()
    }

    /// Selects glyphs `start..end` (inclusive; any order).
    pub fn set_selection(&self, start: Pos, end: Pos) {
        self.borrow_mut().set_selection(start, end);
    }

    pub fn clear_selection(&self) {
        self.borrow_mut().clear_selection();
    }

    /// Selects every page's text.
    pub fn select_all(&self) {
        self.borrow_mut().select_all();
    }

    /// UTF-8 text of the selection (pages separated by a line break).
    pub fn selected_text(&self) -> String {
        self.borrow().selected_text()
    }

    /// Copies the selection to the clipboard; false when nothing is selected.
    pub fn copy_selection(&self) -> bool {
        self.borrow().copy_selection()
    }
}

pub(crate) fn selection_menu(
    inner: &Inner,
    viewer: &Viewer,
    out: &mut Vec<Node>,
    theme: Theme,
    options: &ViewOptions,
) {
    let Some(doc) = inner.doc.clone() else {
        return;
    };
    let Some(sel) = inner.selection() else {
        return;
    };
    let Some(t) = doc.page_text(sel.start.page) else {
        return;
    };
    if t.count() == 0 {
        return;
    }
    let to = if sel.end.page == sel.start.page {
        sel.end.index
    } else {
        t.count() - 1
    };
    let rects = t.rects(sel.start.index, to.min(t.count() - 1));
    if rects.is_empty() {
        return;
    }
    let origin = inner.page_origin(sel.start.page);
    let first = rects[0];
    let labels = &options.labels;
    let menu_w: f32 = 268.0;
    let menu_h: f32 = 36.0;
    let mut left = origin[0] + first.x * inner.zoom;
    let mut top = origin[1] + first.y * inner.zoom - menu_h - 8.0;
    if top < 4.0 {
        top = origin[1] + first.bottom() * inner.zoom + 8.0;
    }
    if top < 4.0 || top > inner.view_h - menu_h {
        // selection start scrolled away
        return;
    }
    left = left.clamp(8.0, (inner.view_w - menu_w - 8.0).max(8.0));
    let item = |key: String, label: Str, viewer: &Viewer, callback: fn(&Viewer)| {
        let v = viewer.clone();
        button(ButtonOptions {
            key: Some(Str::from(key)),
            text: label,
            variant: ButtonType::Text,
            size: ButtonSize::Small,
            font_size: Some(13.0),
            focusable: false,
            on_click: Some(Action::new(move |cx| {
                callback(&v);
                v.flush_callbacks(cx);
            })),
            ..Default::default()
        })
    };
    let children = vec![
        item(
            format!("{}-sel-copy", options.key),
            labels.copy.clone(),
            viewer,
            menu_copy,
        ),
        item(
            format!("{}-sel-hl", options.key),
            labels.highlight.clone(),
            viewer,
            menu_highlight,
        ),
        item(
            format!("{}-sel-ul", options.key),
            labels.underline.clone(),
            viewer,
            menu_underline,
        ),
        item(
            format!("{}-sel-st", options.key),
            labels.strikeout.clone(),
            viewer,
            menu_strike,
        ),
    ];
    out.push(styled(
        row(BoxOptions {
            key: Some(Str::from(format!("{}-sel-menu", options.key))),
            gap: 4.0,
            align_items: flex::Align::Center,
            padding: Edges::all(4.0),
            block_pointer: true,
            children,
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(left),
            top: Some(top),
            height: Some(menu_h),
            background: Some(theme.color_bg_elevated),
            radius: Some(8.0),
            shadow_color: Some(0x2600_0000),
            shadow_blur: Some(12.0),
            shadow_offset_y: Some(3.0),
            motion: Some(Presence::ZoomBigFast),
            ..Default::default()
        },
    ));
}

// ---- selection menu actions ---------------------------------------------

fn menu_copy(viewer: &Viewer) {
    let _ = viewer.copy_selection();
    viewer.borrow_mut().clear_selection();
}

fn menu_highlight(viewer: &Viewer) {
    let _ = viewer.annotate_selection(annotations::Kind::Highlight, None);
}

fn menu_underline(viewer: &Viewer) {
    let _ = viewer.annotate_selection(annotations::Kind::Underline, None);
}

fn menu_strike(viewer: &Viewer) {
    let _ = viewer.annotate_selection(annotations::Kind::Strikeout, None);
}
