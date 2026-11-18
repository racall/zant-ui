//! The Markdown widget: [`MarkdownOptions`] / [`MarkdownState`] /
//! [`markdown`]. Its slot belongs to the current runtime and is destroyed on
//! unmount, with its caches (`markdown` feature).

use crate::Str;
use crate::core::node::ResizeStep;
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
use crate::ui::actions::{Action, TextAction};
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
    /// The widest a formula, image or diagram may be. 0 (the default)
    /// follows the width the Markdown is laid out at, like its text; a
    /// positive value caps them there (and still never exceeds the layout
    /// width, so a narrow window does not overflow).
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
            content_width: 0.0,
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

/// Before the first layout the width is unknown: Streamdown's column width.
const UNMEASURED_WIDTH: f32 = 680.0;

/// Block widths follow the layout in steps of this many points.
const BLOCK_STEP: f32 = 16.0;

/// The widest formulas / images / diagrams may be: `content_width` (0 =
/// unlimited) within the laid-out width, in 16 pt steps so resizing a window
/// re-lays the blocks out a few times rather than every frame.
fn block_width(content_width: f32, laid_out: Option<f32>) -> f32 {
    let available = laid_out
        .filter(|width| *width > 1.0)
        .map(|width| (width / BLOCK_STEP).floor() * BLOCK_STEP)
        .map(|width| width.max(BLOCK_STEP));
    match (content_width > 0.0, available) {
        (true, Some(available)) => content_width.min(available),
        (true, None) => content_width,
        (false, Some(available)) => available,
        (false, None) => UNMEASURED_WIDTH,
    }
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
    // the width this instance was laid out at last frame (its root reports
    // size changes, which rebuild it)
    let root_key = options
        .key
        .as_ref()
        .map(|key| key.to_string())
        // Streamdown keys its root with the instance prefix
        .unwrap_or_else(|| state.borrow().prefix.clone());
    let laid_out = ui::key_bounds(&root_key).map(|bounds| bounds.width);
    let content_width = block_width(options.content_width, laid_out);
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
                content_width,
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
    root.key = Some(Str::from(root_key));
    // a width in another `BLOCK_STEP` rebuilds with formulas /
    // images fitted to it; height changes (streaming) and moves within a step
    // do not rebuild the app
    root.on_resize = Some(Action::new(|_cx| ()));
    root.resize_step = Some(ResizeStep::width(BLOCK_STEP));
    node
}

/// Renders Markdown with a persistent block cache identified by `key`.
/// Give each instance in a list a distinct, stable key. The key also identifies
/// the root node unless `options.key` is set. [`markdown`] remains available
/// for existing callers with numeric slot IDs.
pub fn markdown_keyed(key: &str, mut options: MarkdownOptions) -> Node {
    let id = crate::tsx::child_id(0, "ui::markdown", key);
    options.key.get_or_insert_with(|| Str::copy(key));
    markdown(id, options)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{MarkdownOptions, block_width, markdown_keyed, ui};

    #[test]
    fn blocks_follow_the_laid_out_width_within_content_width() {
        // the default follows the layout, rounded down to 16 pt steps
        assert_eq!(block_width(0.0, Some(1000.0)), 992.0);
        assert_eq!(block_width(0.0, Some(320.0)), 320.0);
        // a cap still never overflows a narrow layout
        assert_eq!(block_width(680.0, Some(1200.0)), 680.0);
        assert_eq!(block_width(680.0, Some(400.0)), 400.0);
        // before the first layout
        assert_eq!(block_width(0.0, None), 680.0);
        assert_eq!(block_width(500.0, None), 500.0);
    }

    #[test]
    fn keyed_markdown_uses_stable_slots_and_defaults_the_root_key() {
        let store = Rc::new(RefCell::new(ui::LocalStateStore::new()));
        ui::set_build_states(Some(Rc::clone(&store)));
        store.borrow_mut().record();

        let doc = markdown_keyed("doc", MarkdownOptions::default());
        let _ = markdown_keyed("doc", MarkdownOptions::default());
        let _ = markdown_keyed("other", MarkdownOptions::default());
        let renamed = markdown_keyed(
            "doc",
            MarkdownOptions {
                key: Some("root".into()),
                ..MarkdownOptions::default()
            },
        );

        let slots = store.borrow_mut().end_record();
        ui::set_build_states(None);
        let [first, repeated, other, renamed_slot] = slots[..] else {
            panic!("expected one slot per Markdown call, got {slots:?}");
        };
        assert_eq!(first, repeated);
        assert_ne!(first, other);
        assert_eq!(first, renamed_slot);
        assert_eq!(doc.element().key.as_deref(), Some("doc"));
        assert_eq!(renamed.element().key.as_deref(), Some("root"));
    }
}
