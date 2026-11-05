//! The runnable Gallery application lives in src/main.zig and uses the
//! framework's built-in Gallery composition from src/gallery.zig. Run it with:
//!
//!     zig build run
//!
//! This file is kept as the conventional entry point for the example and
//! re-exports the public Gallery module for readers exploring the project.
const zant = @import("zant");

pub const GalleryState = zant.gallery.GalleryState;
