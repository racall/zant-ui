//! The hand-held Streamdown cache of `examples/tsx/bench_markdown.zig`: the
//! control group of the `markdown-stream` scenario. The TSX side goes through
//! the generated `<Markdown>` component (`ui::markdown` with its own cache);
//! this side keeps the cache in the app like the Zig `Manual` struct did. The
//! source assignment is outside the timed render on both sides.
#![forbid(unsafe_code)]

use zant::markdown;
use zant::markdown::cache::{Cache, Mode};
use zant::markdown::render::options::{Caret, Options as RenderOptions};
use zant::markdown::render::view::Props as ViewProps;
use zant::ui::{self, App};

pub struct Manual {
    pub source: String,
    cache: Cache,
    /// `md-<id in hex>`, the key prefix the TSX `<Markdown>` facade uses
    /// (`ui::markdown` formats `md-{instance id:x}`); the Zig control hard-coded
    /// the same value.
    key_prefix: zant::Str,
}

impl Manual {
    pub fn new() -> Manual {
        let id = zant::tsx::site_id(0, "Markdown", 1);
        Manual {
            source: String::new(),
            cache: Cache::new(),
            key_prefix: zant::Str::copy(&format!("md-{id:x}")),
        }
    }
}

impl App for Manual {
    /// Zig `bench_markdown.Manual.render`.
    fn render(&mut self) -> ui::Node {
        markdown::streamdown(
            &mut self.cache,
            &self.source,
            ViewProps {
                mode: Mode::Streaming,
                render: RenderOptions {
                    key_prefix: self.key_prefix.clone(),
                    font_size: 15.0,
                    content_width: 600.0,
                    is_animating: true,
                    caret: Caret::Block,
                    ..RenderOptions::default()
                },
                ..ViewProps::default()
            },
        )
    }
}
