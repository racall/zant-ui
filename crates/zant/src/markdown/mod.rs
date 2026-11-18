//! Zant Markdown — a native implementation of Vercel's Streamdown.
//!
//! Pipeline (mirrors `packages/streamdown`):
//!
//! ```text
//! remend/      heal incomplete Markdown while streaming (`remend`)
//! blocks.rs    split into top-level blocks, reuse stable ones (`parse-blocks`)
//! cache.rs     parse each changed block once (block memoisation)
//! parser/      CommonMark + GFM + remark-math parser → ast.rs
//! render/      components: code blocks, tables, Mermaid, math, links …
//! editor.rs    edit / split / preview editor with formatting commands
//! ```
//!
//! Quick use:
//!
//! ```ignore
//! // stateless, whole document every frame
//! zant::markdown::render(source)
//! // Streamdown with block memoisation (keep `cache` in your state)
//! zant::markdown::streamdown(&mut state.md_cache, source, ViewProps {
//!     mode: CacheMode::Streaming,
//!     ..Default::default()
//! })
//! ```

pub mod ast;
pub mod blocks;
pub mod cache;
pub mod edit;
pub mod editor;
pub mod entities;
pub mod highlight;
pub mod html;
pub mod mermaid;
pub mod parser;
pub mod remend;
pub mod render;
pub mod safety;
pub mod scan;
pub mod stats;
pub mod table_utils;
pub mod theme;
pub mod timing;

pub use ast::{Block, Document, Span, Style};
pub use cache::{Cache, Mode as CacheMode, Options as CacheOptions};
pub use editor::{Labels as EditorLabels, Mode as EditorMode, Props as EditorProps, editor};
pub use parser::Options as ParseOptions;
pub use render::{
    BlockInfo, BlockRenderer, Caret, CodeControls, Controls, Ctx, ImageLoader, Labels,
    LinkModalProps, MermaidControls, Options as RenderOptions, Props as ViewProps, TableControls,
    block_key, block_key_at_offset, heading_key, link_safety_modal, load_image, render_document,
    streamdown,
};
pub use theme::Theme;

use crate::ui;

/// Parses a complete document with the default parse options.
pub fn parse(source: &str) -> Document {
    parser::parse(source, ParseOptions::default())
}

/// Parses a complete document.
pub fn parse_with(source: &str, options: ParseOptions) -> Document {
    parser::parse(source, options)
}

/// Parses and renders `source` for this frame (no caching).
pub fn render(source: &str) -> ui::Node {
    render_with(source, RenderOptions::default())
}

/// Parses and renders `source` for this frame with `options` (no caching).
pub fn render_with(source: &str, options: RenderOptions) -> ui::Node {
    let doc = parse(source);
    render_document(&doc, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uses_the_default_options_and_parse_with_the_given_ones() {
        let doc = parse("# Title\n\ntext $x$");
        assert_eq!(doc.blocks.len(), 2);
        assert!(matches!(doc.blocks[0], Block::Heading(_)));
        let same = parse_with("# Title\n\ntext $x$", ParseOptions::default());
        assert_eq!(same.blocks.len(), doc.blocks.len());

        // `\[..\]` is display math only with `latex_delimiters`
        let source = "\\[\nx\n\\]";
        assert!(!matches!(parse(source).blocks[0], Block::Math(_)));
        let options = ParseOptions {
            latex_delimiters: true,
            ..ParseOptions::default()
        };
        assert!(matches!(
            parse_with(source, options).blocks[0],
            Block::Math(_)
        ));
    }

    #[test]
    fn render_builds_one_node_per_block() {
        // the selectable root wraps the column of blocks
        fn blocks(node: &ui::Node) -> usize {
            node.element().children[0].element().children.len()
        }
        ui::begin_frame();
        let node = render("# Title\n\nparagraph");
        let sized = render_with(
            "# Title\n\nparagraph\n\nmore",
            RenderOptions {
                font_size: 20.0,
                ..RenderOptions::default()
            },
        );
        ui::end_frame();
        assert_eq!(blocks(&node), 2);
        assert_eq!(blocks(&sized), 3);
    }
}
