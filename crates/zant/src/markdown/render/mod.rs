//! Block and inline rendering of a parsed document (Streamdown's
//! `lib/components.tsx`): the whole-document renderer, the memoised
//! `streamdown` component, code blocks / tables / Mermaid diagrams with their
//! controls, inline math and links, the link-safety dialog.
//!
//! The names re-exported here are the ones `markdown/mod.rs` exposes at the
//! module root.

pub mod actions;
pub mod blocks;
pub mod code_block;
pub mod context;
pub mod controls;
pub mod data_images;
pub mod diagram;
pub mod labels;
pub mod link_modal;
pub mod options;
pub mod spans;
pub mod table;
pub mod view;

pub use actions::{Copy, Download, Link};
pub use blocks::load_image;
pub use context::Ctx;
pub use controls::GAP;
pub use link_modal::{Props as LinkModalProps, link_safety_modal};
pub use options::{
    BlockInfo, BlockRenderer, Caret, CodeControls, Controls, ImageLoader, Labels, MermaidControls,
    Options, TableControls,
};
pub use view::{
    Props, block_key, block_key_at_offset, document as render_document, heading_key, streamdown,
};
