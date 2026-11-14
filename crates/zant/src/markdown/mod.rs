//! Port of `src/markdown/mod.zig` — WP-3.1/3.2.

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

// Zig `markdown/mod.zig` re-exports: `zant.markdown.streamdown` etc.
pub use editor::{Props as EditorProps, editor};
pub use render::{
    BlockInfo, BlockRenderer, Caret, CodeControls, Controls, Ctx, ImageLoader, Labels,
    LinkModalProps, MermaidControls, Options as RenderOptions, Props as ViewProps, TableControls,
    block_key, block_key_at_offset, heading_key, link_safety_modal, load_image, render_document,
    streamdown,
};
