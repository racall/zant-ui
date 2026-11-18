//! Options of images, icons, vectors and PDF pages.

use super::*;

/// `ui::image`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ImageOptions {
    pub key: Option<Str>,
    /// Image file path (PNG, JPEG, GIF, BMP, ICO).
    pub src: Str,
    /// In-memory image (`zant::image::Image`); used instead of `src`.
    pub image: Option<Rc<Image>>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fit: ImageFit,
    pub radius: f32,
    pub tooltip: Option<Str>,
}

impl Default for ImageOptions {
    fn default() -> Self {
        ImageOptions {
            key: None,
            src: Str::default(),
            image: None,
            width: None,
            height: None,
            fit: ImageFit::Cover,
            radius: 8.0,
            tooltip: None,
        }
    }
}

/// `ui::icon`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct IconOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub name: Icon,
    pub size: f32,
    pub color: Option<u32>,
    /// Stroke width in px (default scales with size: 1.5px at 18px).
    pub stroke: Option<f32>,
}

impl Default for IconOptions {
    fn default() -> Self {
        IconOptions {
            key: None,
            name: Icon::None,
            size: 18.0,
            color: None,
            stroke: None,
        }
    }
}

/// `ui::vector`: a retained vector drawing.
#[derive(Clone, zant_macros::TsxProps)]
pub struct VectorOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub drawing: DrawingRef,
    /// Fixed box width (default: the drawing's width). The drawing is
    /// left-aligned; wrap in a row to center it.
    pub width: Option<f32>,
    /// Text copied when the drawing is part of a text selection (e.g. a
    /// formula's LaTeX); empty keeps it out of selections.
    pub copy: Str,
}

impl Default for VectorOptions {
    fn default() -> Self {
        VectorOptions {
            key: None,
            drawing: Rc::new(Drawing::default()),
            width: None,
            copy: Str::default(),
        }
    }
}

/// `ui::pdf_preview`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct PdfOptions {
    pub key: Option<Str>,
    #[tsx(required)]
    pub src: Str,
    pub page: usize,
    pub width: f32,
    pub height: f32,
    pub show_toolbar: bool,
    pub background: Option<u32>,
    pub on_page_change: Option<IndexAction>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            key: None,
            src: Str::default(),
            page: 0,
            width: 320.0,
            height: 420.0,
            show_toolbar: true,
            background: None,
            on_page_change: None,
        }
    }
}
