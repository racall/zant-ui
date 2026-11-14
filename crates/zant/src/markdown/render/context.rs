//! Port of `src/markdown/render/context.zig` — WP-3.3.
//!
//! Per-render state passed down the block tree. Zig carried an arena for
//! callback contexts and embed allocations; `Rc` closures own their contexts
//! in Rust, so `Ctx` only carries the options and the current style.

use std::rc::Rc;

use crate::markdown::render::options::Options;
use crate::platform::dialog;
use crate::renderer::text;
use crate::ui::actions::TextAction;

#[derive(Clone)]
pub struct Ctx {
    pub options: Rc<Options>,
    /// Current text colour (quotes and callouts mute it).
    pub fg: u32,
    /// Current body size (footnotes shrink it).
    pub size: f32,
    /// Append the streaming caret to the last text run.
    pub caret: bool,
    pub link_action: Option<TextAction>,
    /// Unique key prefix of the block being rendered.
    pub key: String,
}

impl Ctx {
    pub fn init(options: &Rc<Options>) -> Ctx {
        Ctx {
            options: Rc::clone(options),
            fg: options.theme.foreground,
            size: options.font_size,
            caret: false,
            link_action: Some(link_action(options.on_link.clone())),
            key: "md".to_owned(),
        }
    }

    pub fn with(&self, fg: u32, size: f32) -> Ctx {
        let mut c = self.clone();
        c.fg = fg;
        c.size = size;
        c
    }

    /// `"{key}-{suffix}"`: the key prefix of a child node.
    pub fn sub_key(&self, suffix: &str) -> String {
        format!("{}-{}", self.key, suffix)
    }

    pub fn family(&self) -> text::Family {
        self.options.theme.body_family
    }
}

/// Zig `actions.Link.action`: rich-text link activation forwards to the app
/// (link safety) or opens the URL directly.
pub fn link_action(forward: Option<TextAction>) -> TextAction {
    TextAction::new(move |cx, url| match &forward {
        Some(action) => action.call(cx, url),
        None => {
            let _ = dialog::open_url(url);
        }
    })
}
