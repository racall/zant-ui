//! Port of `src/math/mod.zig` — WP-3.5 / WP-3.6; UI layer moved in by WP-M3.
//!
//! LaTeX math (the `zant.math` component): the TeX subset KaTeX supports is
//! parsed ([`parser`]) and laid out with KaTeX's algorithms and font metrics
//! ([`layout`]), then drawn with STIX Two Math as a retained vector drawing.
//! [`block`] / [`render`] are the display-formula UI wrappers of the Zig
//! module root.

pub mod fonts;
pub mod layout;
pub mod mhchem;
pub mod parser;
pub mod symbols;

mod block;

pub use block::{block, render};
