//! A thumbnail cache stand-in for tests/fixtures/media/App.tsx.

/// The runtime's in-memory image handle (`Image` in TSX).
pub type Image = std::rc::Rc<zant::image::Image>;

pub fn thumbnail(id: u64) -> Option<Image> {
    let _ = id;
    None
}

/// The width in a PNG's header (an imported asset reaches Rust as bytes).
#[allow(non_snake_case)]
pub fn pngWidth(png: &'static [u8]) -> u32 {
    if png.len() < 24 || &png[1..4] != b"PNG" {
        return 0;
    }
    u32::from_be_bytes([png[16], png[17], png[18], png[19]])
}
