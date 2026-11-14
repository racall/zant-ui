//! Port of `src/renderer/` — WP-1.1c.

pub mod canvas;
// The Zig module keeps its own name (`renderer.zig` inside `renderer/`).
#[allow(clippy::module_inception)]
pub mod renderer;
pub mod rich_text;
pub mod text;
