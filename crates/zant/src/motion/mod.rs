//! Animation: easing curves and `Animated` (see docs/motion.md).
//! Port of `src/motion/` — WP-1.1a.

pub mod easing;
// The Zig module keeps its own name (`renderer.zig` inside `renderer/`).
#[allow(clippy::module_inception)]
pub mod motion;
