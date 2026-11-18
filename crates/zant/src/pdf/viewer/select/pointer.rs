//! Pointer events of the page area: selecting, dragging, clicking links.

use super::*;

impl Inner {
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
    pub(super) fn activate_link(
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
