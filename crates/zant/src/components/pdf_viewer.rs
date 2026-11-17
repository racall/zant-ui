//! The PDF reader widget: [`PdfViewerOptions`] / [`PdfViewerState`] /
//! [`pdf_viewer`]. Its slot belongs to the current runtime and is destroyed
//! on unmount, with its document and worker thread (`pdf` feature).

use std::cell::RefCell;
use std::rc::Rc;

use crate::Str;
use crate::core::node::Icon;
use crate::flex;
use crate::i18n::builtin;
use crate::pdf::{self, ViewOptions, Viewer};
use crate::ui;
use crate::ui::actions::{Action, TextAction};
use crate::ui::element::Node;
use crate::ui::{BoxOptions, IconOptions, TextOptions};
/// Full reader options, with document source and flex sizing added.
pub struct PdfViewerOptions {
    pub src: Str,
    pub password: Option<Str>,
    pub key: Option<Str>,
    pub toolbar: bool,
    pub page_gap: f32,
    pub padding: f32,
    pub background: Option<u32>,
    pub labels: pdf::Labels,
    pub on_link: Option<TextAction>,
    pub on_change: Option<Action>,
    pub annotation_tools: bool,
    pub selection_menu: bool,
    pub on_annotation: Option<Action>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Defaults to 1, or 0 with an explicit height.
    pub grow: Option<f32>,
    pub empty_text: Str,
    pub loading_text: Str,
    pub error_text: Str,
}

impl Default for PdfViewerOptions {
    fn default() -> PdfViewerOptions {
        PdfViewerOptions {
            src: Str::default(),
            password: None,
            key: None,
            toolbar: true,
            page_gap: 12.0,
            padding: 16.0,
            background: None,
            labels: pdf::Labels::default(),
            on_link: None,
            on_change: None,
            annotation_tools: false,
            selection_menu: true,
            on_annotation: None,
            width: None,
            height: None,
            grow: None,
            empty_text: builtin::PDF_EMPTY.current(),
            loading_text: builtin::PDF_LOADING.current(),
            error_text: builtin::PDF_ERROR.current(),
        }
    }
}

/// The slot payload, including owned input strings (TSX
/// props may be frame memory).
#[derive(Default)]
pub struct PdfViewerState {
    pub viewer: Viewer,
    pub src: String,
    pub password: Option<String>,
    pub prefix: String,
    pub pending: bool,
    /// The last open failed (missing file, wrong password, not a PDF).
    pub failed: bool,
}

impl PdfViewerState {
    /// Remembers a changed source / password and opens it after
    /// reconciliation (file I/O must not run in the build itself). Failed paths
    /// are remembered too (no retry every frame).
    fn update(state: &Rc<RefCell<PdfViewerState>>, options: &PdfViewerOptions, fresh: bool) {
        let mut slot = state.borrow_mut();
        if !fresh
            && slot.src == options.src.as_str()
            && same_password(slot.password.as_deref(), options.password.as_deref())
        {
            return;
        }
        slot.src = options.src.to_string();
        slot.password = options.password.as_ref().map(|value| value.to_string());
        slot.pending = true;
        let effect_state = Rc::clone(state);
        ui::effect(Action::new(move |_cx| PdfViewerState::open(&effect_state)));
    }

    /// Closes the document and opens `src` (empty clears it).
    fn open(state: &Rc<RefCell<PdfViewerState>>) {
        let (src, password) = {
            let slot = state.borrow();
            (slot.src.clone(), slot.password.clone())
        };
        let mut slot = state.borrow_mut();
        slot.viewer.close();
        slot.failed = false;
        if !src.is_empty()
            && slot
                .viewer
                .open_with_password(&src, password.as_deref())
                .is_err()
        {
            slot.failed = true;
        }
        slot.pending = false;
        ui::request_rebuild();
    }
}

fn same_password(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// A complete PDF reader that reopens only when src/password
/// change. Empty, loading and failed sources keep the same size and themed
/// surface.
pub fn pdf_viewer(id: u64, options: PdfViewerOptions) -> Node {
    let slot = ui::local_slot::<PdfViewerState>(id, PdfViewerState::default);
    let state = Rc::clone(&slot.value);
    if slot.fresh {
        let mut slot_state = state.borrow_mut();
        slot_state.viewer = Viewer::new();
        slot_state.prefix = format!("pdf-{id:x}");
    }
    PdfViewerState::update(&state, &options, slot.fresh);
    let theme = ui::current_theme();
    let (pending, failed, prefix) = {
        let slot_state = state.borrow();
        (
            slot_state.pending,
            slot_state.failed,
            slot_state.prefix.clone(),
        )
    };
    let message: Option<Str> = if options.src.is_empty() {
        Some(options.empty_text.clone())
    } else if pending {
        Some(options.loading_text.clone())
    } else if failed {
        Some(options.error_text.clone())
    } else {
        None
    };
    let content = if let Some(text) = message {
        ui::column(BoxOptions {
            grow: 1.0,
            justify: flex::Justify::Center,
            align_items: flex::Align::Center,
            gap: 8.0,
            padding: ui::inset(theme.padding),
            background: Some(options.background.unwrap_or(theme.color_bg_layout)),
            children: vec![
                ui::icon(IconOptions {
                    name: Icon::FileText,
                    size: 32.0,
                    color: Some(if failed {
                        theme.color_error
                    } else {
                        theme.color_text_disabled
                    }),
                    ..IconOptions::default()
                }),
                ui::text(
                    text,
                    TextOptions {
                        color: Some(if failed {
                            theme.color_error_text
                        } else {
                            theme.color_text_secondary
                        }),
                        size: theme.font_size,
                        ..TextOptions::default()
                    },
                ),
            ],
            ..BoxOptions::default()
        })
    } else {
        let viewer = state.borrow().viewer.clone();
        pdf::view(
            &viewer,
            ViewOptions {
                key: Str::copy(&prefix),
                toolbar: options.toolbar,
                page_gap: options.page_gap,
                padding: options.padding,
                background: options.background,
                labels: options.labels.clone(),
                on_link: options.on_link.clone(),
                on_change: options.on_change.clone(),
                annotation_tools: options.annotation_tools,
                selection_menu: options.selection_menu,
                on_annotation: options.on_annotation.clone(),
            },
        )
    };
    ui::column(BoxOptions {
        key: Some(options.key.clone().unwrap_or_else(|| Str::copy(&prefix))),
        width: options.width,
        height: options.height,
        grow: options
            .grow
            .unwrap_or(if options.height.is_some() { 0.0 } else { 1.0 }),
        shrink: 0.0,
        align_items: flex::Align::Stretch,
        clip: true,
        children: vec![content],
        ..BoxOptions::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_identity_includes_presence_and_content() {
        assert!(same_password(None, None));
        assert!(!same_password(None, Some("")));
        assert!(same_password(Some("secret"), Some("secret")));
        assert!(!same_password(Some("secret"), Some("other")));
    }
}
