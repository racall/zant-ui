//! Rendering: the Skia surface, the canvas and text layouts.

pub mod canvas;
// The module keeps the name of its directory (`renderer::renderer`).
#[allow(clippy::module_inception)]
pub mod renderer;
pub mod rich_text;
pub mod text;
