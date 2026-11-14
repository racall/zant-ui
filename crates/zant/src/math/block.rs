//! UI layer of `src/math/mod.zig` — moved into the framework by WP-M3.
//!
//! The pure layout engine lives in [`crate::math::layout`]; [`block`] and
//! [`render`] are the `ui`-dependent half of the Zig module (a display formula
//! as a centred full-width row with one `ui.vector` retained drawing).
//!
//! `inlineDrawing` was ported earlier as
//! `crate::markdown::render::spans::inline_drawing` (WP-3.3), and
//! `inlineText` has no caller in the Zig tree, so neither is duplicated here.

use std::rc::Rc;

use crate::core::geometry::Edges;
use crate::layout::flex;
use crate::math::layout::{self, Options};
use crate::ui::{self, BoxOptions, VectorOptions};

/// Zig `math/mod.zig: block`: a centred display formula (KaTeX
/// `.katex-display`: 1em vertical margin).
pub fn block(source: &str, options: Options) -> ui::Node {
    let options = Options {
        display: true,
        ..options
    };
    let drawing = layout::layout(source, options);
    ui::row(BoxOptions {
        width_percent: Some(100.0),
        justify: flex::Justify::Center,
        margin: Edges {
            top: 0.5 * options.size,
            bottom: 0.5 * options.size,
            ..Edges::default()
        },
        children: vec![ui::vector(VectorOptions {
            drawing: Rc::new(drawing),
            ..VectorOptions::default()
        })],
        ..BoxOptions::default()
    })
}

/// Zig `math/mod.zig: render`: the backwards-compatible display formula with
/// default options (the Zig signature took an allocator, which Rust does not
/// need).
pub fn render(source: &str) -> ui::Node {
    block(source, Options::default())
}
