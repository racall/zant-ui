//! Rendering options (Streamdown's component props).
//!
//! `ImageLoader` and `BlockRenderer` are `Rc<dyn Fn>`, so `Options` is fully
//! self-contained and `'static` (needed by the memoised block builders).
//! `BlockInfo` owns its strings (`Str`); callbacks capture `Rc` contexts.

use std::rc::Rc;

use crate::Str;
use crate::core::hash::Wyhash;
use crate::image::Image;
use crate::markdown::safety;
use crate::markdown::theme as theme_mod;
use crate::ui::actions::TextAction;
use crate::ui::element::Node;

// `markdown::safety::Policy` keeps borrowed slices and has no `Clone`;
// `Options` is cloned for every memoised block, so the renderer adds the impl.
impl<'a> Clone for safety::Policy<'a> {
    fn clone(&self) -> Self {
        safety::Policy {
            allowed_link_prefixes: self.allowed_link_prefixes,
            allowed_image_prefixes: self.allowed_image_prefixes,
            allow_data_images: self.allow_data_images,
            allow_local_images: self.allow_local_images,
            disabled_autolink_protocols: self.disabled_autolink_protocols,
        }
    }
}

/// Loads images by URL (`Options.image_loader`).
///
/// The decoded image for `url`, or `None` while it is not available. Keep it
/// cheap (a cache lookup): start a download on a worker thread and call
/// `ui::post_wake()` when it is decoded; the next render shows it.
///
/// The options hash uses the closure's `Rc` address: create the loader **once** (keep the `Rc` in the
/// app state) and clone it per frame, otherwise every frame looks like a
/// changed loader and every block rebuilds.
pub type ImageLoader = Rc<dyn Fn(&str) -> Option<Image>>;

/// The callback of a custom block renderer.
pub type BlockRender = Rc<dyn Fn(&BlockInfo) -> Option<Node>>;

/// What a custom block renderer receives (`Options.block_renderers`).
#[derive(Clone, Debug, Default)]
pub struct BlockInfo {
    /// Language of the fence (as written).
    pub lang: Str,
    /// Rest of the info string (e.g. `{"id": 3}` in ```widget {"id": 3}).
    pub meta: Str,
    /// Block content.
    pub code: Str,
    /// False while the fence is still streaming in.
    pub complete: bool,
    /// Unique key for the block's root node.
    pub key: Str,
}

/// Renders fenced code blocks of one language as custom UI (Streamdown's
/// custom `components`): charts, widgets, cards embedded in a message.
///
/// Like [`ImageLoader`], the callback is hashed by its `Rc` address: keep the
/// `Rc` in app state and clone it per frame instead of creating a new closure.
#[derive(Clone)]
pub struct BlockRenderer {
    /// Fence language that selects this renderer (case-insensitive).
    pub lang: Str,
    /// The block's UI, or `None` to fall back to a normal code block.
    pub func: BlockRender,
}

impl BlockRenderer {
    pub fn new(lang: &str, render: impl Fn(&BlockInfo) -> Option<Node> + 'static) -> BlockRenderer {
        BlockRenderer {
            lang: Str::copy(lang),
            func: Rc::new(render),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Caret {
    #[default]
    None,
    /// ▋
    Block,
    /// ●
    Circle,
}

impl Caret {
    pub fn glyph(self) -> &'static str {
        match self {
            Caret::None => "",
            Caret::Block => "▋",
            Caret::Circle => "●",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CodeControls {
    pub copy: bool,
    pub download: bool,
}

impl Default for CodeControls {
    fn default() -> Self {
        CodeControls {
            copy: true,
            download: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TableControls {
    pub copy: bool,
    pub download: bool,
}

impl Default for TableControls {
    fn default() -> Self {
        TableControls {
            copy: true,
            download: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MermaidControls {
    pub copy: bool,
    pub download: bool,
}

impl Default for MermaidControls {
    fn default() -> Self {
        MermaidControls {
            copy: true,
            download: true,
        }
    }
}

/// Streamdown `controls`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub code: CodeControls,
    pub table: TableControls,
    pub mermaid: MermaidControls,
}

impl Controls {
    /// Every control off (Streamdown `controls: { code: false, … }`).
    pub const NONE: Controls = Controls {
        code: CodeControls {
            copy: false,
            download: false,
        },
        table: TableControls {
            copy: false,
            download: false,
        },
        mermaid: MermaidControls {
            copy: false,
            download: false,
        },
    };

    pub fn none() -> Controls {
        Controls::NONE
    }
}

/// Streamdown `translations`.
#[derive(Clone, Debug)]
pub struct Labels {
    pub copy: Str,
    pub copy_markdown: Str,
    pub copy_csv: Str,
    pub download: Str,
    pub footnotes: Str,
    pub image: Str,
    pub loading_image: Str,
    pub open_link: Str,
    pub copy_link: Str,
    pub cancel: Str,
    pub link_title: Str,
    pub link_message: Str,
    pub unsupported_diagram: Str,
}

impl Default for Labels {
    fn default() -> Self {
        Labels {
            copy: "Copy".into(),
            copy_markdown: "Copy Markdown".into(),
            copy_csv: "Copy CSV".into(),
            download: "Download".into(),
            footnotes: "Footnotes".into(),
            image: "Image".into(),
            loading_image: "Loading image…".into(),
            open_link: "Open link".into(),
            copy_link: "Copy link".into(),
            cancel: "Cancel".into(),
            link_title: "Open external link?".into(),
            link_message: "You're about to visit an external website.".into(),
            unsupported_diagram: "Diagram type not supported".into(),
        }
    }
}

impl Labels {
    /// Chinese labels.
    pub fn zh() -> Labels {
        Labels {
            copy: "复制".into(),
            copy_markdown: "复制 Markdown".into(),
            copy_csv: "复制 CSV".into(),
            download: "下载".into(),
            footnotes: "脚注".into(),
            image: "图片".into(),
            loading_image: "图片加载中…".into(),
            open_link: "打开链接".into(),
            copy_link: "复制链接".into(),
            cancel: "取消".into(),
            link_title: "打开外部链接？".into(),
            link_message: "即将访问外部网站，请确认链接可信。".into(),
            unsupported_diagram: "暂不支持此图表类型".into(),
        }
    }
}

/// Every renderer option (Streamdown's props).
#[derive(Clone)]
pub struct Options {
    pub theme: theme_mod::Theme,
    /// Body font size (px).
    pub font_size: f32,
    /// Content column width; display math and diagrams shrink to fit.
    pub content_width: f32,
    /// Streamdown `isAnimating`: the response is still streaming. Shows the
    /// caret and disables copy/download on the incomplete tail.
    pub is_animating: bool,
    pub caret: Caret,
    pub controls: Controls,
    /// Line numbers in code blocks (Streamdown default: on).
    pub line_numbers: bool,
    /// Code blocks taller than this scroll (0 = no limit). Default 400px.
    pub code_max_height: f32,
    /// Link clicks. With Streamdown's link safety, the app shows
    /// `link_safety_modal` for the URL; `None` opens links directly.
    pub on_link: Option<TextAction>,
    pub policy: safety::Policy<'static>,
    /// Directory relative image paths resolve against.
    pub base_dir: Str,
    /// Remote images (`http(s)://…`, app schemes). Without it they show a
    /// placeholder card. `data:image/…;base64` URIs are always decoded.
    pub image_loader: Option<ImageLoader>,
    /// Custom renderers for fenced blocks by language.
    pub block_renderers: Vec<BlockRenderer>,
    pub labels: Labels,
    /// Prefix for node keys (several documents in one window).
    pub key_prefix: Str,
    /// Text can be selected with the pointer and copied (⌘C / Ctrl+C);
    /// formulas copy as their LaTeX source.
    pub selectable: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            theme: theme_mod::light(),
            font_size: 16.0,
            content_width: 680.0,
            is_animating: false,
            caret: Caret::None,
            controls: Controls::default(),
            line_numbers: true,
            code_max_height: 400.0,
            on_link: None,
            policy: safety::Policy::default(),
            base_dir: Str::default(),
            image_loader: None,
            block_renderers: Vec::new(),
            labels: Labels::default(),
            key_prefix: "md".into(),
            selectable: true,
        }
    }
}

fn hash_str(h: &mut Wyhash, value: &str) {
    h.update(value.as_bytes());
}

fn hash_f32(h: &mut Wyhash, value: f32) {
    h.update(&value.to_bits().to_le_bytes());
}

fn hash_policy(h: &mut Wyhash, policy: &safety::Policy<'_>) {
    h.update(&(policy.allowed_link_prefixes.len() as u64).to_le_bytes());
    for prefix in policy.allowed_link_prefixes {
        hash_str(h, prefix);
    }
    h.update(&(policy.allowed_image_prefixes.len() as u64).to_le_bytes());
    for prefix in policy.allowed_image_prefixes {
        hash_str(h, prefix);
    }
    h.update(&[
        policy.allow_data_images as u8,
        policy.allow_local_images as u8,
    ]);
    h.update(&(policy.disabled_autolink_protocols.len() as u64).to_le_bytes());
    for protocol in policy.disabled_autolink_protocols {
        hash_str(h, protocol);
    }
}

fn hash_labels(h: &mut Wyhash, labels: &Labels) {
    for value in [
        &labels.copy,
        &labels.copy_markdown,
        &labels.copy_csv,
        &labels.download,
        &labels.footnotes,
        &labels.image,
        &labels.loading_image,
        &labels.open_link,
        &labels.copy_link,
        &labels.cancel,
        &labels.link_title,
        &labels.link_message,
        &labels.unsupported_diagram,
    ] {
        hash_str(h, value);
    }
}

impl Options {
    /// Field-by-field hash: covers every option (policy, all labels) and never
    /// reads struct padding. Callbacks and loaders hash by their `Rc`
    /// address (`TextAction` is opaque outside `ui::actions`, so `on_link`
    /// hashes by presence only).
    pub fn hash(&self) -> u64 {
        let mut h = Wyhash::init(0x6f707473);
        self.theme.hash_into(&mut h);
        hash_f32(&mut h, self.font_size);
        hash_f32(&mut h, self.content_width);
        h.update(&[self.is_animating as u8]);
        h.update(&[match self.caret {
            Caret::None => 0,
            Caret::Block => 1,
            Caret::Circle => 2,
        }]);
        for control in [
            self.controls.code.copy,
            self.controls.code.download,
            self.controls.table.copy,
            self.controls.table.download,
            self.controls.mermaid.copy,
            self.controls.mermaid.download,
        ] {
            h.update(&[control as u8]);
        }
        h.update(&[self.line_numbers as u8]);
        hash_f32(&mut h, self.code_max_height);
        h.update(&[self.on_link.is_some() as u8]);
        hash_policy(&mut h, &self.policy);
        hash_str(&mut h, &self.base_dir);
        match &self.image_loader {
            Some(loader) => {
                h.update(&[1u8]);
                h.update(&(Rc::as_ptr(loader) as *const () as usize).to_le_bytes());
            }
            None => h.update(&[0u8]),
        }
        h.update(&(self.block_renderers.len() as u64).to_le_bytes());
        for renderer in &self.block_renderers {
            hash_str(&mut h, &renderer.lang);
            h.update(&(Rc::as_ptr(&renderer.func) as *const () as usize).to_le_bytes());
        }
        hash_labels(&mut h, &self.labels);
        hash_str(&mut h, &self.key_prefix);
        h.update(&[self.selectable as u8]);
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_option_values() {
        let options = Options::default();
        assert_eq!(options.theme, theme_mod::light());
        assert_eq!(options.font_size, 16.0);
        assert_eq!(options.content_width, 680.0);
        assert!(!options.is_animating);
        assert_eq!(options.caret, Caret::None);
        assert!(options.controls.code.copy);
        assert!(options.controls.table.download);
        assert!(options.controls.mermaid.copy);
        assert!(options.line_numbers);
        assert_eq!(options.code_max_height, 400.0);
        assert!(options.on_link.is_none());
        assert!(options.image_loader.is_none());
        assert!(options.block_renderers.is_empty());
        assert_eq!(options.key_prefix, "md");
        assert!(options.selectable);

        assert_eq!(Caret::Block.glyph(), "▋");
        assert_eq!(Caret::Circle.glyph(), "●");
        assert_eq!(Caret::None.glyph(), "");
        let none = Controls::none();
        assert!(!none.code.copy && !none.table.download && !none.mermaid.copy);
    }

    #[test]
    fn hash_changes_with_every_group_of_options() {
        let base = Options::default();
        let hash = base.hash();
        assert_eq!(hash, Options::default().hash(), "stable for equal options");

        let theme_changed = Options {
            theme: theme_mod::dark(),
            ..Options::default()
        };
        assert_ne!(hash, theme_changed.hash());

        let labels_changed = Options {
            labels: Labels::zh(),
            ..Options::default()
        };
        assert_ne!(hash, labels_changed.hash());

        let caret_changed = Options {
            caret: Caret::Block,
            ..Options::default()
        };
        assert_ne!(hash, caret_changed.hash());

        let policy = safety::Policy {
            allowed_link_prefixes: &["https://zant.dev/"],
            ..safety::Policy::default()
        };
        let policy_changed = Options {
            policy,
            ..Options::default()
        };
        assert_ne!(hash, policy_changed.hash());

        let renderer_changed = Options {
            block_renderers: vec![BlockRenderer::new("chart", |_| None)],
            ..Options::default()
        };
        assert_ne!(hash, renderer_changed.hash());

        let loader: ImageLoader = Rc::new(|_| None);
        let loader_changed = Options {
            image_loader: Some(loader),
            ..Options::default()
        };
        assert_ne!(hash, loader_changed.hash());
    }
}
