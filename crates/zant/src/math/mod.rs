//! LaTeX math (the `zant.math` component): the TeX subset KaTeX supports is
//! parsed ([`parser`]) and laid out with KaTeX's algorithms and font metrics
//! ([`layout`]), then drawn with STIX Two Math as a retained vector drawing.
//! [`block`] / [`render`] are the display-formula UI wrappers,
//! [`inline_drawing`] is the inline (text-style) formula for rich text and
//! [`inline_text`] its plain-text rendering.

pub mod fonts;
pub mod layout;
pub mod mhchem;
pub mod parser;
pub mod symbols;

mod block;
mod text;

pub use block::{block, render};
pub use text::inline_text;

use std::rc::Rc;

use crate::core::vector::{Drawing, DrawingRef};

/// An inline (text-style) formula for embedding in rich text (Markdown's
/// `$…$`).
pub fn inline_drawing(source: &str, options: layout::Options) -> DrawingRef {
    let opts = layout::Options {
        display: false,
        ..options
    };
    let mut drawing: Drawing = layout::layout(source, opts);
    // keep line metrics stable: never shorter than the surrounding text
    drawing.ascent = drawing.ascent.max(0.75 * opts.size);
    drawing.descent = drawing.descent.max(0.22 * opts.size);
    Rc::new(drawing)
}
