//! Stateful document builders. Slots belong to the current runtime and are
//! destroyed on unmount, including caches, documents and PDF worker threads.
//!
//! Provides [`MarkdownOptions`] / [`MarkdownState`] / [`markdown`] and
//! [`PdfViewerOptions`] / [`PdfViewerState`] / [`pdf_viewer`].

use std::cell::RefCell;
use std::rc::Rc;

use crate::Str;
use crate::core::node::Icon;
use crate::layout::flex;
use crate::markdown::cache::{Cache, Mode};
use crate::markdown::parser;
use crate::markdown::render::options::{
    BlockRenderer, Caret, CodeControls, Controls, ImageLoader, Labels, MermaidControls, Options,
    TableControls,
};
use crate::markdown::render::view::{self, Props};
use crate::markdown::safety;
use crate::markdown::theme;
use crate::pdf::{self, ViewOptions, Viewer};
use crate::theme::tokens;
use crate::ui;
use crate::ui::actions::{Action, TextAction};
use crate::ui::element::Node;
use crate::ui::{BoxOptions, IconOptions, TextOptions};

/// Markdown content, typography and Streamdown controls. Complex renderer
/// extensions remain available to Rust callers; scalar fields reflect into TSX.
pub struct MarkdownOptions {
    pub source: Str,
    pub key: Option<Str>,
    pub streaming: bool,
    pub parse_incomplete: bool,
    pub single_dollar_math: bool,
    pub latex_delimiters: bool,
    pub font_size: f32,
    pub content_width: f32,
    pub caret: Caret,
    pub line_numbers: bool,
    pub code_max_height: f32,
    pub code_copy: bool,
    pub code_download: bool,
    pub table_copy: bool,
    pub table_download: bool,
    pub mermaid_copy: bool,
    pub mermaid_download: bool,
    pub on_link: Option<TextAction>,
    pub base_dir: Str,
    pub selectable: bool,
    pub theme: Option<theme::Theme>,
    pub policy: safety::Policy<'static>,
    pub labels: Labels,
    pub image_loader: Option<ImageLoader>,
    pub block_renderers: Vec<BlockRenderer>,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        MarkdownOptions {
            source: Str::default(),
            key: None,
            streaming: false,
            parse_incomplete: true,
            single_dollar_math: true,
            latex_delimiters: false,
            font_size: 16.0,
            content_width: 680.0,
            caret: Caret::None,
            line_numbers: true,
            code_max_height: 400.0,
            code_copy: true,
            code_download: true,
            table_copy: true,
            table_download: true,
            mermaid_copy: true,
            mermaid_download: true,
            on_link: None,
            base_dir: Str::default(),
            selectable: true,
            theme: None,
            policy: safety::Policy::default(),
            labels: Labels::default(),
            image_loader: None,
            block_renderers: Vec::new(),
        }
    }
}

/// Slot payload (public for diagnostics); applications normally use
/// [`markdown`].
#[derive(Default)]
pub struct MarkdownState {
    pub cache: Cache,
    /// Key prefix of every node of this instance (`md-<id in hex>`).
    pub prefix: String,
}

impl MarkdownState {
    /// The cache frees the parsed blocks on `Drop`, so this is a no-op kept
    /// for a symmetrical API.
    pub fn deinit(&mut self) {}
}

/// Renders Markdown with a persistent block cache. Give every simultaneous
/// instance a distinct stable id; stop calling it to release the cache.
pub fn markdown(id: u64, options: MarkdownOptions) -> Node {
    let slot = ui::local_slot::<MarkdownState>(id, MarkdownState::default);
    let state = slot.value;
    {
        let mut state = state.borrow_mut();
        if slot.fresh {
            state.cache = Cache::new();
            state.prefix = format!("md-{id:x}");
        }
    }
    let dark = tokens::is_dark(ui::current_theme());
    let node = {
        let mut state = state.borrow_mut();
        let render = Props {
            mode: if options.streaming {
                Mode::Streaming
            } else {
                Mode::Static
            },
            parse_incomplete: options.parse_incomplete,
            parse: parser::Options {
                single_dollar_math: options.single_dollar_math,
                latex_delimiters: options.latex_delimiters,
                ..Default::default()
            },
            remend: crate::markdown::remend::Options::default(),
            render: Options {
                theme: options
                    .theme
                    .unwrap_or(if dark { theme::dark() } else { theme::light() }),
                key_prefix: Str::copy(&state.prefix),
                font_size: options.font_size,
                content_width: options.content_width,
                is_animating: options.streaming,
                caret: options.caret,
                line_numbers: options.line_numbers,
                code_max_height: options.code_max_height,
                controls: Controls {
                    code: CodeControls {
                        copy: options.code_copy,
                        download: options.code_download,
                    },
                    table: TableControls {
                        copy: options.table_copy,
                        download: options.table_download,
                    },
                    mermaid: MermaidControls {
                        copy: options.mermaid_copy,
                        download: options.mermaid_download,
                    },
                },
                on_link: options.on_link.clone(),
                base_dir: options.base_dir.clone(),
                selectable: options.selectable,
                policy: options.policy,
                labels: options.labels.clone(),
                image_loader: options.image_loader.clone(),
                block_renderers: options.block_renderers.clone(),
            },
        };
        view::streamdown(&mut state.cache, &options.source, render)
    };
    // Streamdown's root is 100% wide. Let the parent stretch this facade so
    // `ui::styled(..., width = ..)` can override its size normally.
    let mut node = node;
    let root = node.make_mut();
    root.layout.width_percent = None;
    if let Some(key) = options.key {
        root.key = Some(key);
    }
    node
}

// ── PDF viewer widget ─────────────────────────────────────────────────────

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
            empty_text: Str::Static("Choose a PDF to read"),
            loading_text: Str::Static("Opening PDF…"),
            error_text: Str::Static("This PDF can't be opened"),
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
