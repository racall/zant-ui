//! Annotations of the PDF viewer — the annotation section of
//! `src/pdf/viewer.zig` (WP-3.9).
//!
//! The lists, undo / redo history, hit testing and the annotated export live
//! in `pdf::annotations` (WP-3.8); this module is the viewer side: turning a
//! text selection or an ink drag into an annotation, selecting / editing /
//! removing one, marshalling the list for `save_annotated`, and drawing the
//! annotation layers (markup boxes, squiggles, ink strokes, note icons, the
//! selection outline) and the annotation tool switch of the toolbar.

use std::rc::Rc;

use crate::Str;
use crate::components::segmented::{SegmentedOption, SegmentedOptions};
use crate::core::node::Icon;
use crate::core::vector::{Command, Drawing, Path};
use crate::layout::flex;
use crate::pdf::annotations::{self, Annotation, Kind, Point};
use crate::pdf::native;
use crate::pdf::text::Rect;
use crate::theme::tokens::Theme;
use crate::ui::actions::{Action, IndexAction};
use crate::ui::builders::{icon_button, row, space};
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, IconButtonOptions, IconOptions, SpaceOptions};
use crate::ui::{self, Style, styled};

use super::{Inner, SaveError, Tool, ViewOptions, Viewer};

impl Inner {
    /// Zig `Viewer.setTool`: switches the pointer tool (clears the selection).
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

    /// Finishes the ink stroke being drawn (Zig `finishInk`).
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
                        justify: crate::layout::flex::Justify::Center,
                        align_items: crate::layout::flex::Align::Center,
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

/// One markup / underline / strikethrough box in viewport px.
fn markup_box(r: Rect, z: f32, color: u32, radius: f32) -> Node {
    ui::styled(
        ui::column(BoxOptions::default()),
        Style {
            absolute: Some(true),
            left: Some(r.x * z),
            top: Some(r.y * z),
            width: Some(r.width * z),
            height: Some(r.height * z),
            background: Some(color),
            radius: Some(radius),
            ..Default::default()
        },
    )
}

/// Zig `squiggle`: a zigzag polyline under the text baseline.
fn squiggle(r: Rect, z: f32, color: u32) -> Node {
    let w = r.width * z;
    let amp = (r.height * z * 0.05).max(1.0);
    let step = (amp * 2.0).max(2.0);
    let n = ((w / step).ceil() + 1.0).max(2.0) as usize;
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let x = w.min(i as f32 * step);
        points.push([x, if i % 2 == 0 { 0.0 } else { amp * 2.0 }]);
    }
    let drawing = Drawing {
        width: w,
        ascent: 0.0,
        descent: amp * 2.0,
        commands: vec![Command::Path(Path {
            points,
            color: color | 0xff00_0000,
            stroke_width: (amp * 0.8).max(1.0),
            closed: false,
        })],
        hash: 0,
    };
    ui::styled(
        ui::vector(ui::VectorOptions {
            drawing: Rc::new(drawing),
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(r.x * z),
            top: Some(r.bottom() * z - amp * 2.0),
            ..Default::default()
        },
    )
}

/// Zig `inkNode`: freehand strokes as one vector path per stroke.
pub(crate) fn ink_node(
    strokes: &[Vec<Point>],
    color: u32,
    width: f32,
    z: f32,
    size: [f32; 2],
    key: Option<Str>,
) -> Node {
    let mut commands = Vec::with_capacity(strokes.len());
    for stroke in strokes {
        let mut points = Vec::with_capacity(stroke.len().max(2));
        for point in stroke {
            points.push([point[0] * z, point[1] * z]);
        }
        // a single tap draws a dot
        if stroke.len() == 1 {
            points.push([stroke[0][0] * z + 0.01, stroke[0][1] * z]);
        }
        commands.push(Command::Path(Path {
            points,
            color,
            stroke_width: (width * z).max(1.0),
            closed: false,
        }));
    }
    let drawing = Drawing {
        width: size[0],
        ascent: 0.0,
        descent: size[1],
        commands,
        hash: 0,
    };
    ui::styled(
        ui::vector(ui::VectorOptions {
            key,
            drawing: Rc::new(drawing),
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            ..Default::default()
        },
    )
}

// `annotations::colors` names (Zig `annots.colors`).
const BLUE: u32 = 0xff16_77ff;

impl Viewer {
    /// Default colour of an annotation kind (Zig `Viewer.defaultColor`).
    pub fn default_color(kind: Kind) -> u32 {
        match kind {
            Kind::Highlight => annotations::colors::YELLOW,
            Kind::Underline | Kind::Squiggly => BLUE,
            Kind::Strikeout | Kind::Ink => annotations::colors::RED,
            Kind::Note => annotations::colors::NOTE,
        }
    }

    /// Node key of an annotation's box (for `ui::key_bounds`, e.g. to place
    /// a comment editor next to the selected annotation).
    pub fn annotation_key(key_prefix: &str, id: u64) -> String {
        format!("{key_prefix}-a{id}")
    }

    /// Switches the pointer tool (clears the text selection).
    pub fn set_tool(&self, tool: Tool) {
        self.borrow_mut().set_tool(tool);
    }

    /// The current pointer tool.
    pub fn tool(&self) -> Tool {
        self.borrow().tool
    }

    /// Colour for new annotations of the current tool (`None`: default).
    pub fn set_tool_color(&self, color: Option<u32>) {
        self.borrow_mut().tool_color = color;
    }

    /// The selected annotation.
    pub fn selected_annotation(&self) -> Option<Annotation> {
        self.borrow().selected_annotation().cloned()
    }

    /// Id of the selected annotation.
    pub fn selected_annotation_id(&self) -> Option<u64> {
        self.borrow().selected_annotation
    }

    /// Selects an annotation (`None` clears the selection).
    pub fn select_annotation(&self, id: Option<u64>) {
        self.borrow_mut().select_annotation(id);
    }

    /// The annotation list (read access; it is the viewer's undo history).
    pub fn annotations(&self) -> std::cell::Ref<'_, annotations::Store> {
        std::cell::Ref::map(self.borrow(), |inner| &inner.annotations)
    }

    /// The annotation list (write access, e.g. `annotations_mut().load(list)`
    /// to restore a saved list; Zig exposed the field directly).
    pub fn annotations_mut(&self) -> std::cell::RefMut<'_, annotations::Store> {
        std::cell::RefMut::map(self.borrow_mut(), |inner| &mut inner.annotations)
    }

    /// Turns the text selection into markup annotations (one per page);
    /// returns the id of the last one.
    pub fn annotate_selection(&self, kind: Kind, color: Option<u32>) -> Option<u64> {
        self.borrow_mut().annotate_selection(kind, color)
    }

    pub fn remove_annotation(&self, id: u64) {
        self.borrow_mut().remove_annotation(id);
    }

    pub fn set_annotation_contents(&self, id: u64, contents: &str) {
        self.borrow_mut().set_annotation_contents(id, contents);
    }

    pub fn set_annotation_color(&self, id: u64, color: u32) {
        self.borrow_mut().set_annotation_color(id, color);
    }

    pub fn undo(&self) -> bool {
        self.borrow_mut().undo()
    }

    pub fn redo(&self) -> bool {
        self.borrow_mut().redo()
    }

    /// Writes a copy of the open PDF with the annotations to `out_path`
    /// (standard PDF annotations other viewers show and edit).
    pub fn save_annotated(&self, out_path: &str) -> Result<(), SaveError> {
        self.borrow().save_annotated(out_path)
    }
}

/// Annotation tool switch of the toolbar (Zig `toolSwitch`): a segmented
/// control with the select / highlight / underline / pen / note tools, then
/// undo and redo.
pub(crate) fn tool_switch(inner: &Inner, viewer: &Viewer, options: &ViewOptions) -> Node {
    let labels = &options.labels;
    let selected: usize = match inner.tool {
        Tool::Select => 0,
        Tool::Highlight => 1,
        Tool::Underline | Tool::Strikeout | Tool::Squiggly => 2,
        Tool::Ink => 3,
        Tool::Note => 4,
    };
    let mut children: Vec<Node> = Vec::new();
    children.push(styled(
        space(SpaceOptions {
            width: Some(8.0),
            ..Default::default()
        }),
        Style::default(),
    ));
    let segmented_viewer = viewer.clone();
    children.push(ui::segmented(SegmentedOptions {
        key: Some(Str::from(format!("{}-tools", options.key))),
        options: vec![
            SegmentedOption {
                label: labels.select.clone(),
                ..Default::default()
            },
            SegmentedOption {
                label: labels.highlight.clone(),
                ..Default::default()
            },
            SegmentedOption {
                label: labels.underline.clone(),
                ..Default::default()
            },
            SegmentedOption {
                label: labels.pen.clone(),
                icon: Icon::Pencil,
                ..Default::default()
            },
            SegmentedOption {
                label: labels.note.clone(),
                icon: Icon::FileText,
                ..Default::default()
            },
        ],
        selected,
        on_change: Some(IndexAction::new(move |_, index| {
            tb_tool(&segmented_viewer, index)
        })),
        height: 24.0,
        font_size: 12.0,
        ..Default::default()
    }));
    let undo_viewer = viewer.clone();
    children.push(icon_button(IconButtonOptions {
        key: Some(Str::from(format!("{}-undo", options.key))),
        icon: Icon::Undo,
        size: 16.0,
        r#box: 28.0,
        disabled: !inner.annotations.can_undo(),
        on_click: Some(Action::new(move |_| tb_undo(&undo_viewer))),
        ..Default::default()
    }));
    let redo_viewer = viewer.clone();
    children.push(icon_button(IconButtonOptions {
        key: Some(Str::from(format!("{}-redo", options.key))),
        icon: Icon::Redo,
        size: 16.0,
        r#box: 28.0,
        disabled: !inner.annotations.can_redo(),
        on_click: Some(Action::new(move |_| tb_redo(&redo_viewer))),
        ..Default::default()
    }));
    row(BoxOptions {
        gap: 4.0,
        align_items: flex::Align::Center,
        children,
        ..Default::default()
    })
}

fn tb_tool(viewer: &Viewer, index: usize) {
    let tools = [
        Tool::Select,
        Tool::Highlight,
        Tool::Underline,
        Tool::Ink,
        Tool::Note,
    ];
    let mut inner = viewer.borrow_mut();
    inner.set_tool(tools[index.min(tools.len() - 1)]);
    inner.changed();
}

fn tb_undo(viewer: &Viewer) {
    let _ = viewer.undo();
}

fn tb_redo(viewer: &Viewer) {
    let _ = viewer.redo();
}
