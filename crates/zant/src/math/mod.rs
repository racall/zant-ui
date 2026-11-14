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

pub use crate::markdown::render::spans::inline_drawing;
