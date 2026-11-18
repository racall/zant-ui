//! Display formulas as UI nodes.
//!
//! The pure layout engine lives in [`crate::math::layout`]; [`block`] and
//! [`render`] wrap it in a centred full-width row with one `ui::vector`
//! retained drawing.

use std::rc::Rc;

use crate::core::geometry::Edges;
use crate::flex;
use crate::math::layout::{self, Options};
use crate::ui::{self, BoxOptions, VectorOptions};

/// A centred display formula (KaTeX `.katex-display`: 1em vertical margin).
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

/// A display formula with default options.
pub fn render(source: &str) -> ui::Node {
    block(source, Options::default())
}
