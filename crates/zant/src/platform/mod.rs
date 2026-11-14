//! Port of `src/platform/` — WP-1.1d.

pub mod dialog;
pub mod pdf;
// The Zig module keeps its own name (`renderer.zig` inside `renderer/`).
#[allow(clippy::module_inception)]
pub mod platform;
