//! Creating, moving, recolouring and deleting annotations (`impl Inner`).

use super::*;

impl Inner {
    /// Switches the pointer tool (clears the selection).
    pub(crate) fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.clear_selection();
        self.ink_points.clear();
        if tool != Tool::Select {
            self.select_annotation(None);
        }
    }

    pub(crate) fn selected_annotation(&self) -> Option<&Annotation> {
        self.annotations.get(self.selected_annotation?)
    }

    pub(crate) fn select_annotation(&mut self, id: Option<u64>) {
        if self.selected_annotation == id {
            return;
        }
        self.selected_annotation = id;
        self.annotation_changed();
    }

    /// Turns the text selection into markup annotations (one per page);
    /// returns the id of the last one.
    pub(crate) fn annotate_selection(&mut self, kind: Kind, color: Option<u32>) -> Option<u64> {
        let doc = self.doc.clone()?;
        let range = self.selection()?;
        if !kind.is_markup() {
            return None;
        }
        let mut last: Option<u64> = None;
        let mut page = range.start.page;
        while page <= range.end.page {
            let Some(text) = doc.page_text(page) else {
                page += 1;
                continue;
            };
            if text.count() == 0 {
                page += 1;
                continue;
            }
            let from = if page == range.start.page {
                range.start.index
            } else {
                0
            };
            let to = if page == range.end.page {
                range.end.index
            } else {
                text.count() - 1
            }
            .min(text.count() - 1);
            if from <= to {
                let rects = text.rects(from, to);
                if !rects.is_empty() {
                    let mut text_buf = String::new();
                    text.append_text(&mut text_buf, from, to);
                    let annotation = Annotation {
                        id: 0,
                        page,
                        kind,
                        color: color.unwrap_or_else(|| Viewer::default_color(kind)),
                        rects,
                        text: text_buf,
                        ..Default::default()
                    };
                    last = Some(self.annotations.add(annotation));
                }
            }
            page += 1;
        }
        if last.is_some() {
            self.clear_selection();
            self.annotation_changed();
        }
        last
    }

    pub(crate) fn remove_annotation(&mut self, id: u64) {
        if self.annotations.remove(id) {
            if self.selected_annotation == Some(id) {
                self.selected_annotation = None;
            }
            self.annotation_changed();
        }
    }

    pub(crate) fn set_annotation_contents(&mut self, id: u64, contents: &str) {
        self.annotations.set_contents(id, contents);
        self.annotation_changed();
    }

    pub(crate) fn set_annotation_color(&mut self, id: u64, color: u32) {
        self.annotations.set_color(id, color);
        self.annotation_changed();
    }

    pub(crate) fn undo(&mut self) -> bool {
        if !self.annotations.undo() {
            return false;
        }
        if let Some(id) = self.selected_annotation
            && self.annotations.get(id).is_none()
        {
            self.selected_annotation = None;
        }
        self.annotation_changed();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        if !self.annotations.redo() {
            return false;
        }
        self.annotation_changed();
        true
    }

    /// Writes a copy of the open PDF with the annotations to `out_path`
    /// (standard PDF annotations other viewers show and edit).
    pub(crate) fn save_annotated(&self, out_path: &str) -> Result<(), SaveError> {
        if self.doc.is_none() || self.path.is_empty() {
            return Err(SaveError::NoDocument);
        }
        let list: Vec<native::Annotation> = self
            .annotations
            .all()
            .iter()
            .map(|annotation| {
                let mut rects = Vec::with_capacity(annotation.rects.len() * 4);
                for r in &annotation.rects {
                    rects.extend_from_slice(&[r.x, r.y, r.width, r.height]);
                }
                let mut points = Vec::new();
                let mut stroke_lengths = Vec::with_capacity(annotation.strokes.len());
                for stroke in &annotation.strokes {
                    stroke_lengths.push(stroke.len() as i32);
                    for point in stroke {
                        points.extend_from_slice(point);
                    }
                }
                native::Annotation {
                    page: annotation.page as i32,
                    kind: annotation.kind as i32,
                    color: annotation.color,
                    width: annotation.width,
                    rects,
                    points,
                    stroke_lengths,
                    contents: Some(annotation.contents.clone()),
                }
            })
            .collect();
        if native::save_annotated(&self.path, None, out_path, &list) {
            Ok(())
        } else {
            Err(SaveError::SaveFailed)
        }
    }

    /// Annotation id under a viewport point (topmost first).
    pub(crate) fn annotation_at(&self, vx: f32, vy: f32) -> Option<u64> {
        let point = self.page_point(vx, vy)?;
        let list = self.annotations.all();
        for a in list.iter().rev() {
            if a.page == point.page && a.hit(point.x, point.y) {
                return Some(a.id);
            }
        }
        None
    }

    /// Finishes the ink stroke being drawn.
    pub(crate) fn finish_ink(&mut self) {
        if self.ink_points.is_empty() {
            self.ink_points.clear();
            return;
        }
        let stroke = self.ink_points.clone();
        self.ink_points.clear();
        let annotation = Annotation {
            id: 0,
            page: self.ink_page,
            kind: Kind::Ink,
            color: self
                .tool_color
                .unwrap_or_else(|| Viewer::default_color(Kind::Ink)),
            strokes: vec![stroke],
            width: 2.0,
            ..Default::default()
        };
        self.annotations.add(annotation);
        self.annotation_changed();
    }

    /// Draws the annotation layers of `a` (markup, ink, note icon) and its
    /// keyed bounds box (popovers place themselves with that key; the box is
    /// outlined while selected).
    pub(crate) fn annotation_layer(
        &self,
        a: &Annotation,
        theme: Theme,
        options: &ViewOptions,
        out: &mut Vec<Node>,
    ) {
        let z = self.zoom;
        match a.kind {
            Kind::Highlight => {
                for r in &a.rects {
                    out.push(markup_box(*r, z, a.color, 1.0));
                }
            }
            Kind::Underline => {
                for r in &a.rects {
                    let thickness = (1.0 / z).max(r.height * 0.07);
                    out.push(markup_box(
                        Rect {
                            x: r.x,
                            y: r.bottom() - thickness * 1.5,
                            width: r.width,
                            height: thickness,
                        },
                        z,
                        a.color,
                        0.0,
                    ));
                }
            }
            Kind::Strikeout => {
                for r in &a.rects {
                    let thickness = (1.0 / z).max(r.height * 0.07);
                    out.push(markup_box(
                        Rect {
                            x: r.x,
                            y: r.y + r.height * 0.52 - thickness / 2.0,
                            width: r.width,
                            height: thickness,
                        },
                        z,
                        a.color,
                        0.0,
                    ));
                }
            }
            Kind::Squiggly => {
                for r in &a.rects {
                    out.push(squiggle(*r, z, a.color));
                }
            }
            Kind::Ink => {
                let size = self.page_size_px(a.page);
                out.push(ink_node(&a.strokes, a.color, a.width, z, size, None));
            }
            Kind::Note => {
                if let Some(r) = a.rects.first() {
                    let icon = ui::row(BoxOptions {
                        justify: crate::flex::Justify::Center,
                        align_items: crate::flex::Align::Center,
                        children: vec![ui::icon(IconOptions {
                            name: crate::core::node::Icon::FileText,
                            size: (r.width * z * 0.7).max(10.0),
                            color: Some(0xd900_0000),
                            ..Default::default()
                        })],
                        ..Default::default()
                    });
                    out.push(ui::styled(
                        icon,
                        Style {
                            absolute: Some(true),
                            left: Some(r.x * z),
                            top: Some(r.y * z),
                            width: Some(r.width * z),
                            height: Some(r.height * z),
                            background: Some(a.color | 0xff00_0000),
                            radius: Some(3.0 * z),
                            shadow_color: Some(0x3300_0000),
                            shadow_blur: Some(3.0),
                            shadow_offset_y: Some(1.0),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        // keyed box over the annotation (popovers place themselves with it);
        // outlined while selected
        let b = a.bounds();
        let selected = self.selected_annotation == Some(a.id);
        out.push(ui::styled(
            ui::column(BoxOptions {
                key: Some(Str::from(Viewer::annotation_key(&options.key, a.id))),
                ..Default::default()
            }),
            Style {
                absolute: Some(true),
                left: Some(b.x * z - 2.0),
                top: Some(b.y * z - 2.0),
                width: Some(b.width * z + 4.0),
                height: Some(b.height * z + 4.0),
                border_color: if selected {
                    Some(theme.color_primary)
                } else {
                    None
                },
                border_width: Some(if selected { 1.5 } else { 0.0 }),
                radius: Some(3.0),
                ..Default::default()
            },
        ));
    }
}
