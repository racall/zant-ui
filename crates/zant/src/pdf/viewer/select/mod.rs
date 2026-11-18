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

mod hit;
mod pointer;
mod selection;

impl Inner {}

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
