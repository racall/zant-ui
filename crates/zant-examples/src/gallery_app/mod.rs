//! `zant-gallery`: the imperative gallery application.
//!
//! The gallery is the framework's retained-mode demo and does not use the
//! declarative `ui::App` runtime: [`GalleryApp`] owns a hand-built
//! `core::tree::Tree`, paints it and handles the native events itself through
//! [`app::GalleryHost`]. See the module docs of [`app`] and [`gallery`].

pub mod app;
pub mod gallery;

pub use app::{GalleryApp, GalleryHost, run};
pub use gallery::{GalleryIds, GalleryState};
