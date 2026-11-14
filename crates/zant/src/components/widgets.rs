//! Port of `src/components/widgets.zig` — WP-3.3 (markdown half).
//!
//! Stateful document builders. Slots belong to the current runtime and are
//! destroyed on unmount, including caches, documents and PDF worker threads.
//!
//! Ported here: [`MarkdownOptions`] / [`MarkdownState`] / [`markdown`].
//! The PDF half of `widgets.zig` (`PdfViewerOptions` / `PdfViewerState` /
//! `pdfViewer`, plus its `password identity` test) depends on
//! `pdf/viewer.rs`, which WP-3.9 owns; it is reported as a leftover and
//! intentionally absent here.

use crate::Str;
use crate::markdown::cache::{Cache, Mode};
use crate::markdown::parser;
use crate::markdown::render::options::{
    BlockRenderer, Caret, CodeControls, Controls, ImageLoader, Labels, MermaidControls, Options,
    TableControls,
};
use crate::markdown::render::view::{self, Props};
use crate::markdown::safety;
use crate::markdown::theme;
use crate::theme::tokens;
use crate::ui;
use crate::ui::actions::TextAction;
use crate::ui::element::Node;

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
    /// Zig `deinit` released the parsed blocks and their arenas; the Rust cache
    /// frees them on `Drop`, so this is a no-op kept for a symmetrical API.
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
                ..Default::default()
            },
        };
        view::streamdown(&mut state.cache, &options.source, render)
    };
    // Streamdown's root is 100% wide. Let the parent stretch this facade so
    // `ui::styled(..., width = ..)` can override its size normally.
    let mut root = node.element_clone();
    root.layout.width_percent = None;
    if let Some(key) = options.key {
        root.key = Some(key);
    }
    Node::new(root)
}
