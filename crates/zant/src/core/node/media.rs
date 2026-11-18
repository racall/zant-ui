//! Retained data of images and vectors.

use super::*;

/// `ImageData`: an image source and how it fills its box.
pub struct ImageData {
    pub path: Str,
    /// In-memory source; the tree shares the loaded `Image` through `Rc`.
    pub handle: Option<std::rc::Rc<crate::image::Image>>,
    pub source_id: u64,
    pub intrinsic_width: i32,
    pub intrinsic_height: i32,
    pub fit: ImageFit,
    pub radius: f32,
}

impl Default for ImageData {
    fn default() -> Self {
        ImageData {
            path: Str::default(),
            handle: None,
            source_id: 0,
            intrinsic_width: 0,
            intrinsic_height: 0,
            fit: ImageFit::Cover,
            radius: 0.0,
        }
    }
}

/// `VectorData`: a retained vector drawing (math blocks, diagrams).
pub struct VectorData {
    pub drawing: DrawingRef,
    pub hash: u64,
    /// Text copied for the drawing in a text selection ("" = not selectable),
    /// e.g. a formula's LaTeX.
    pub copy: Str,
}
